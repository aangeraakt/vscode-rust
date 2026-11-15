//! The piece tree: text as a red-black tree of pieces over append-only buffers.
//!
//! Ported from `src/vs/editor/common/model/pieceTreeTextBuffer/pieceTreeBase.ts`. This layer has
//! the construction and the read-only queries: line contents, offsets and positions, and the
//! value of a range. Edits (`insert`, `delete`) follow in the next PR of the stack.
//!
//! Positions here are `(line, column)` pairs, both 1-based, with columns in UTF-16 code units.
//! Offsets are UTF-16 code units from the start of the text.

use vscode_api::editor::EndOfLineSequence;

use crate::piece::{Cursor, Piece, StringBuffer, normalize_breaks};
use crate::rbtree::{NIL, RbTree};

/// Text stored as pieces over buffers, with cached line feed counts.
///
/// Every edit keeps the invariants checked by the upstream tests: piece lengths and line feed
/// counts match their text, and each subtree's metadata matches its contents.
#[derive(Debug)]
pub struct PieceTree {
    rb: RbTree,
    buffers: Vec<StringBuffer>,
    line_count: usize,
    length: usize,
    eol: EndOfLineSequence,
    eol_normalized: bool,
    last_change_buffer_pos: Cursor,
}

impl PieceTree {
    /// Builds a tree over `chunks` (`create`). Empty chunks are skipped.
    pub(crate) fn new(
        chunks: Vec<StringBuffer>,
        eol: EndOfLineSequence,
        eol_normalized: bool,
    ) -> Self {
        let mut tree = Self {
            rb: RbTree::new(),
            buffers: vec![StringBuffer::new(String::new())],
            line_count: 1,
            length: 0,
            eol,
            eol_normalized,
            last_change_buffer_pos: Cursor::default(),
        };
        let mut last_node = NIL;
        for chunk in chunks {
            if chunk.text().is_empty() {
                continue;
            }
            let starts = chunk.line_starts();
            let last = starts.len() - 1;
            let end = Cursor {
                line: last,
                column: chunk.utf16_len() - starts[last],
            };
            let piece = Piece::new(
                tree.buffers.len(),
                Cursor::default(),
                end,
                last,
                chunk.utf16_len(),
            );
            tree.buffers.push(chunk);
            last_node = tree.rb.insert_right(last_node, piece);
        }
        tree.compute_buffer_metadata();
        tree
    }

    /// The terminator the tree was built with (`getEOL`).
    pub fn eol(&self) -> EndOfLineSequence {
        self.eol
    }

    /// Whether every line break in the tree already uses [`Self::eol`] (`_EOLNormalized`).
    pub fn is_eol_normalized(&self) -> bool {
        self.eol_normalized
    }

    /// Length of the whole text in UTF-16 code units (`getLength`).
    pub fn length(&self) -> usize {
        self.length
    }

    /// Number of lines; at least 1 (`getLineCount`).
    pub fn line_count(&self) -> usize {
        self.line_count
    }

    /// The text with its line terminators as stored (`getLinesRawContent`).
    pub fn raw_content(&self) -> String {
        let mut out = String::new();
        let mut x = if self.rb.root == NIL {
            NIL
        } else {
            self.rb.leftest(self.rb.root)
        };
        while x != NIL {
            out.push_str(self.piece_text(x, 0, self.rb.piece(x).length));
            x = self.rb.next(x);
        }
        out
    }

    /// The content of a line without its terminator (`getLineContent`).
    pub fn line_content(&self, line: usize) -> String {
        if line == self.line_count {
            self.line_raw_content(line, 0)
        } else if self.eol_normalized {
            self.line_raw_content(line, self.eol.as_str().len())
        } else {
            strip_trailing_break(&self.line_raw_content(line, 0))
        }
    }

    /// Content of every line without terminators (`getLinesContent`).
    pub fn lines_content(&self) -> Vec<String> {
        (1..=self.line_count)
            .map(|l| self.line_content(l))
            .collect()
    }

    /// Length of a line in UTF-16 code units, without its terminator (`getLineLength`).
    pub fn line_length(&self, line: usize) -> usize {
        if line == self.line_count {
            self.length - self.offset_at(line, 1)
        } else {
            self.offset_at(line + 1, 1) - self.offset_at(line, 1) - self.eol.as_str().len()
        }
    }

    /// UTF-16 offset of a 1-based position (`getOffsetAt`). Positions are trusted to be valid.
    pub fn offset_at(&self, line: usize, column: usize) -> usize {
        let mut left_len = 0;
        let mut x = self.rb.root;
        let mut line = line as isize;
        while x != NIL {
            let n = *self.rb.node(x);
            let lf_left = n.lf_left as isize;
            if n.left != NIL && lf_left + 1 >= line {
                x = n.left;
            } else if lf_left + n.piece.line_feed_cnt as isize + 1 >= line {
                left_len += n.size_left;
                let accumulated = self.accumulated(x, line - lf_left - 2);
                return left_len + accumulated + column - 1;
            } else {
                line -= lf_left + n.piece.line_feed_cnt as isize;
                left_len += n.size_left + n.piece.length;
                x = n.right;
            }
        }
        left_len
    }

    /// The 1-based position of a UTF-16 offset (`getPositionAt`). Offsets past the end are
    /// clamped to the end of the text.
    pub fn position_at(&self, offset: usize) -> (usize, usize) {
        let original = offset;
        let mut offset = offset;
        let mut x = self.rb.root;
        let mut lf_cnt = 0;

        while x != NIL {
            let n = *self.rb.node(x);
            if n.size_left != 0 && n.size_left >= offset {
                x = n.left;
            } else if n.size_left + n.piece.length >= offset {
                let (index, remainder) = self.index_of(x, offset - n.size_left);
                lf_cnt += n.lf_left + index;
                if index == 0 {
                    let line_start = self.offset_at(lf_cnt + 1, 1);
                    return (lf_cnt + 1, original - line_start + 1);
                }
                return (lf_cnt + 1, remainder + 1);
            } else {
                offset -= n.size_left + n.piece.length;
                lf_cnt += n.lf_left + n.piece.line_feed_cnt;
                if n.right == NIL {
                    // past the last node
                    let line_start = self.offset_at(lf_cnt + 1, 1);
                    let column = original - offset - line_start;
                    return (lf_cnt + 1, column + 1);
                }
                x = n.right;
            }
        }
        (1, 1)
    }

    /// The text between two positions, with `\r\n`, `\r` and `\n` replaced by `eol`
    /// (`getValueInRange`). Equal positions give an empty string.
    pub fn value_in_range(&self, start: (usize, usize), end: (usize, usize), eol: &str) -> String {
        if start == end {
            return String::new();
        }
        let start_pos = self
            .node_at2(start.0, start.1)
            .expect("start is inside the text");
        let end_pos = self.node_at2(end.0, end.1).expect("end is inside the text");
        let raw = self.value_between(start_pos, end_pos);
        normalize_breaks(&raw, eol)
    }

    // --- internals: lookups -------------------------------------------------------------

    fn compute_buffer_metadata(&mut self) {
        let mut x = self.rb.root;
        let mut lf_cnt = 1;
        let mut len = 0;
        while x != NIL {
            let n = self.rb.node(x);
            lf_cnt += n.lf_left + n.piece.line_feed_cnt;
            len += n.size_left + n.piece.length;
            x = n.right;
        }
        self.line_count = lf_cnt;
        self.length = len;
    }

    /// The node and remainder for a 1-based position (`nodeAt2`). `None` past the end.
    fn node_at2(&self, line: usize, column: usize) -> Option<(usize, usize)> {
        let mut x = self.rb.root;
        let mut line = line as isize;
        let mut column = column as isize;

        while x != NIL {
            let n = *self.rb.node(x);
            let lf_left = n.lf_left as isize;
            let lf = n.piece.line_feed_cnt as isize;
            if n.left != NIL && lf_left >= line - 1 {
                x = n.left;
            } else if lf_left + lf > line - 1 {
                let prev = self.accumulated(x, line - lf_left - 2) as isize;
                let acc = self.accumulated(x, line - lf_left - 1) as isize;
                return Some((x, (prev + column - 1).min(acc) as usize));
            } else if lf_left + lf == line - 1 {
                let prev = self.accumulated(x, line - lf_left - 2) as isize;
                if prev + column - 1 <= n.piece.length as isize {
                    return Some((x, (prev + column - 1) as usize));
                }
                column -= n.piece.length as isize - prev;
                break;
            } else {
                line -= lf_left + lf;
                x = n.right;
            }
        }

        // the position is in a later node that starts within the same line
        x = self.rb.next(x);
        while x != NIL {
            let n = *self.rb.node(x);
            if n.piece.line_feed_cnt > 0 {
                let acc = self.accumulated(x, 0) as isize;
                return Some((x, (column - 1).min(acc) as usize));
            }
            if n.piece.length as isize >= column - 1 {
                return Some((x, (column - 1) as usize));
            }
            column -= n.piece.length as isize;
            x = self.rb.next(x);
        }
        None
    }

    /// The UTF-16 offset, relative to the node's piece start, of the `index`-th line start
    /// in it (`getAccumulatedValue`).
    fn accumulated(&self, node: usize, index: isize) -> usize {
        if index < 0 {
            return 0;
        }
        let piece = self.rb.piece(node);
        let starts = self.buffers[piece.buffer_index].line_starts();
        let expected = piece.start.line + index as usize + 1;
        let start = starts[piece.start.line] + piece.start.column;
        if expected > piece.end.line {
            (starts[piece.end.line] + piece.end.column) - start
        } else {
            starts[expected] - start
        }
    }

    /// The text of a line, including its terminator up to `end_offset` units short of the
    /// next line start (`getLineRawContent`).
    fn line_raw_content(&self, line: usize, end_offset: usize) -> String {
        let mut x = self.rb.root;
        let mut ret = String::new();
        let mut line = line as isize;

        while x != NIL {
            let n = *self.rb.node(x);
            let lf_left = n.lf_left as isize;
            let lf = n.piece.line_feed_cnt as isize;
            if n.left != NIL && lf_left >= line - 1 {
                x = n.left;
            } else if lf_left + lf > line - 1 {
                let prev = self.accumulated(x, line - lf_left - 2);
                let acc = self.accumulated(x, line - lf_left - 1);
                return self.piece_text(x, prev, acc - end_offset).to_string();
            } else if lf_left + lf == line - 1 {
                let prev = self.accumulated(x, line - lf_left - 2);
                ret.push_str(self.piece_text(x, prev, n.piece.length));
                break;
            } else {
                line -= lf_left + lf;
                x = n.right;
            }
        }

        // the line continues in the following nodes until the next line feed
        x = self.rb.next(x);
        while x != NIL {
            let n = *self.rb.node(x);
            if n.piece.line_feed_cnt > 0 {
                let acc = self.accumulated(x, 0);
                ret.push_str(self.piece_text(x, 0, acc - end_offset));
                return ret;
            }
            ret.push_str(self.piece_text(x, 0, n.piece.length));
            x = self.rb.next(x);
        }
        ret
    }

    /// The text between two locations, in order (`getValueInRange2`).
    fn value_between(&self, start: (usize, usize), end: (usize, usize)) -> String {
        let (start_node, start_rem) = start;
        let (end_node, end_rem) = end;
        if start_node == end_node {
            return self.piece_text(start_node, start_rem, end_rem).to_string();
        }
        let mut ret = self
            .piece_text(start_node, start_rem, self.rb.piece(start_node).length)
            .to_string();
        let mut x = self.rb.next(start_node);
        while x != NIL {
            if x == end_node {
                ret.push_str(self.piece_text(x, 0, end_rem));
                break;
            }
            ret.push_str(self.piece_text(x, 0, self.rb.piece(x).length));
            x = self.rb.next(x);
        }
        ret
    }

    /// Text of `node` from `from` to `to` units into its piece.
    fn piece_text(&self, node: usize, from: usize, to: usize) -> &str {
        let piece = self.rb.piece(node);
        let buffer = &self.buffers[piece.buffer_index];
        if from == 0 && to == piece.length {
            return buffer.slice(piece.start, piece.end);
        }
        let a = self.position_in_buffer(node, from);
        let b = self.position_in_buffer(node, to);
        buffer.slice(a, b)
    }

    /// The line and column in the buffer of `remainder` units into `node`'s piece
    /// (`positionInBuffer`).
    fn position_in_buffer(&self, node: usize, remainder: usize) -> Cursor {
        let piece = self.rb.piece(node);
        let starts = self.buffers[piece.buffer_index].line_starts();
        let start_offset = starts[piece.start.line] + piece.start.column;
        let offset = start_offset + remainder;

        // binary search for the line that contains `offset`
        let mut low = piece.start.line as isize;
        let mut high = piece.end.line as isize;
        let mut mid: isize = 0;
        let mut mid_start = 0;
        while low <= high {
            mid = low + (high - low) / 2;
            mid_start = starts[mid as usize];
            if mid == high {
                break;
            }
            let mid_stop = starts[mid as usize + 1];
            if offset < mid_start {
                high = mid - 1;
            } else if offset >= mid_stop {
                low = mid + 1;
            } else {
                break;
            }
        }
        Cursor {
            line: mid as usize,
            column: offset - mid_start,
        }
    }

    /// Line feeds between two cursors in one buffer (`getLineFeedCnt`). A `\r` right before
    /// `end` followed by a `\n` at `end` counts as one more break.
    fn line_feed_cnt(&self, buffer_index: usize, start: Cursor, end: Cursor) -> usize {
        if end.column == 0 {
            return end.line - start.line;
        }
        let buffer = &self.buffers[buffer_index];
        let starts = buffer.line_starts();
        if end.line == starts.len() - 1 {
            // no line start after `end`, so there is no line feed after it
            return end.line - start.line;
        }
        let next_line_start = starts[end.line + 1];
        let end_offset = starts[end.line] + end.column;
        if next_line_start > end_offset + 1 {
            return end.line - start.line;
        }
        // end sits right before a line feed: check whether it is a \r\n pair
        let before = Cursor {
            line: end.line,
            column: end.column - 1,
        };
        if buffer.ascii_at(before) == Some(b'\r') {
            end.line - start.line + 1
        } else {
            end.line - start.line
        }
    }

    /// UTF-16 offset of a cursor in a buffer (`offsetInBuffer`).
    fn offset_in_buffer(&self, buffer_index: usize, cursor: Cursor) -> usize {
        self.buffers[buffer_index].offset(cursor)
    }

    /// The position of a UTF-16 offset in a node's piece, as the index of a line start
    /// (`getIndexOf`): `(line index in piece, column in line)`, adjusted for `\r\n`.
    fn index_of(&self, node: usize, accumulated: usize) -> (usize, usize) {
        let piece = self.rb.piece(node);
        let pos = self.position_in_buffer(node, accumulated);
        let line_cnt = pos.line - piece.start.line;

        if self.offset_in_buffer(piece.buffer_index, piece.end)
            - self.offset_in_buffer(piece.buffer_index, piece.start)
            == accumulated
        {
            // the end of this node: a CRLF check is needed
            let real_line_cnt = self.line_feed_cnt(piece.buffer_index, piece.start, pos);
            if real_line_cnt != line_cnt {
                return (real_line_cnt, 0);
            }
        }
        (line_cnt, pos.column)
    }
}

/// Removes one trailing `\r\n`, `\r` or `\n` (`replace(/(\r\n|\r|\n)$/, '')`).
fn strip_trailing_break(s: &str) -> String {
    if let Some(stripped) = s.strip_suffix("\r\n") {
        stripped.to_string()
    } else if let Some(stripped) = s.strip_suffix('\r').or_else(|| s.strip_suffix('\n')) {
        stripped.to_string()
    } else {
        s.to_string()
    }
}

#[cfg(test)]
impl PieceTree {
    /// Checks the tree invariants (`assertTreeInvariants`). Panics on violation.
    pub(crate) fn assert_valid(&self) {
        self.rb.assert_valid();
    }
}
