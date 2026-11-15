//! The piece tree: text as a red-black tree of pieces over append-only buffers.
//!
//! Ported from `src/vs/editor/common/model/pieceTreeTextBuffer/pieceTreeBase.ts`. Behavior follows
//! the TypeScript, including its CRLF handling across piece boundaries, the `_` placeholder that
//! keeps the change buffer's line starts aligned after a split `\r\n`, and the 65535-unit chunk
//! split. Not ported yet: the search cache (a pure lookup accelerator, so results do not change),
//! the snapshot and the search methods, which belong to the text model rows.
//!
//! Positions here are `(line, column)` pairs, both 1-based, with columns in UTF-16 code units.
//! Offsets are UTF-16 code units from the start of the text.

use vscode_api::editor::EndOfLineSequence;

use crate::piece::{Cursor, Piece, StringBuffer, normalize_breaks, utf16_len};
use crate::rbtree::{NIL, RbTree};

/// Chunks longer than this many UTF-16 units are split when inserted (`AverageBufferSize`).
const AVERAGE_BUFFER_SIZE: usize = 65535;

/// A location in a piece: the node, and how far into its piece (`NodePosition`).
#[derive(Clone, Copy, Debug)]
struct NodePosition {
    node: usize,
    remainder: usize,
    node_start_offset: usize,
}

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

    /// Inserts `value` at `offset` (`insert`). `eol_normalized` says whether `value` already
    /// uses the tree's terminator; the tree stays normalized only if both are.
    pub fn insert(&mut self, offset: usize, value: &str, eol_normalized: bool) {
        self.eol_normalized = self.eol_normalized && eol_normalized;
        let mut value = value.to_string();

        if self.rb.root != NIL {
            let pos = self.node_at(offset).expect("offset is inside the text");
            let node = pos.node;
            let remainder = pos.remainder;
            let piece = self.rb.piece(node);
            let insert_pos_in_buffer = self.position_in_buffer(node, remainder);

            if piece.buffer_index == 0
                && piece.end == self.last_change_buffer_pos
                && pos.node_start_offset + piece.length == offset
                && utf16_len(&value) < AVERAGE_BUFFER_SIZE
            {
                // extend the last piece of the change buffer
                self.append_to_node(node, &value);
                self.compute_buffer_metadata();
                return;
            }

            if pos.node_start_offset == offset {
                self.insert_content_to_node_left(value, node);
            } else if pos.node_start_offset + piece.length > offset {
                // inserting into the middle of a node
                let mut nodes_to_del = Vec::new();
                let mut new_right = Piece::new(
                    piece.buffer_index,
                    insert_pos_in_buffer,
                    piece.end,
                    self.line_feed_cnt(piece.buffer_index, insert_pos_in_buffer, piece.end),
                    self.offset_in_buffer(piece.buffer_index, piece.end)
                        - self.offset_in_buffer(piece.buffer_index, insert_pos_in_buffer),
                );

                if self.should_check_crlf() && ends_with_cr(&value) {
                    let head_of_right = self.node_char_code_at(node, remainder);
                    if head_of_right == Some(b'\n') {
                        let new_start = Cursor {
                            line: new_right.start.line + 1,
                            column: 0,
                        };
                        new_right = Piece::new(
                            new_right.buffer_index,
                            new_start,
                            new_right.end,
                            self.line_feed_cnt(new_right.buffer_index, new_start, new_right.end),
                            new_right.length - 1,
                        );
                        value.push('\n');
                    }
                }

                // reuse node for content before insertion point.
                if self.should_check_crlf() && starts_with_lf(&value) {
                    let tail_of_left = self.node_char_code_at(node, remainder - 1);
                    if tail_of_left == Some(b'\r') {
                        let previous_pos = self.position_in_buffer(node, remainder - 1);
                        self.delete_node_tail(node, previous_pos);
                        value.insert(0, '\r');

                        if self.rb.piece(node).length == 0 {
                            nodes_to_del.push(node);
                        }
                    } else {
                        self.delete_node_tail(node, insert_pos_in_buffer);
                    }
                } else {
                    self.delete_node_tail(node, insert_pos_in_buffer);
                }

                let new_pieces = self.create_new_pieces(&value);
                if new_right.length > 0 {
                    self.rb.insert_right(node, new_right);
                }

                let mut tmp = node;
                for piece in new_pieces {
                    tmp = self.rb.insert_right(tmp, piece);
                }
                self.delete_nodes(&nodes_to_del);
            } else {
                self.insert_content_to_node_right(&value, node);
            }
        } else {
            // empty tree: insert a new node
            let pieces = self.create_new_pieces(&value);
            let mut node = self.rb.insert_left(NIL, pieces[0]);
            for piece in pieces.into_iter().skip(1) {
                node = self.rb.insert_right(node, piece);
            }
        }

        self.compute_buffer_metadata();
    }

    /// Removes `cnt` UTF-16 units starting at `offset` (`delete`).
    pub fn delete(&mut self, offset: usize, cnt: usize) {
        if cnt == 0 || self.rb.root == NIL {
            return;
        }
        if self.eol_normalized && self.range_has_break(offset, cnt) {
            // Removing part of a \r\n leaves a bare break behind, so the tree is no longer
            // normalized. (The TypeScript keeps the flag here and then strips the wrong
            // characters in `getLineContent`.)
            self.eol_normalized = false;
        }

        let start_pos = self.node_at(offset).expect("offset is inside the text");
        let end_pos = self.node_at(offset + cnt).expect("end is inside the text");
        let start_node = start_pos.node;
        let end_node = end_pos.node;

        if start_node == end_node {
            let start_split = self.position_in_buffer(start_node, start_pos.remainder);
            let end_split = self.position_in_buffer(start_node, end_pos.remainder);
            let piece_length = self.rb.piece(start_node).length;

            if start_pos.node_start_offset == offset {
                if cnt == piece_length {
                    // delete the whole node
                    let next = self.rb.next(start_node);
                    self.rb.delete(start_node);
                    self.validate_crlf_with_prev_node(next);
                    self.compute_buffer_metadata();
                    return;
                }
                self.delete_node_head(start_node, end_split);
                self.validate_crlf_with_prev_node(start_node);
                self.compute_buffer_metadata();
                return;
            }

            if start_pos.node_start_offset + piece_length == offset + cnt {
                self.delete_node_tail(start_node, start_split);
                self.validate_crlf_with_next_node(start_node);
                self.compute_buffer_metadata();
                return;
            }

            // deleting from the middle: the node is split in two
            self.shrink_node(start_node, start_split, end_split);
            self.compute_buffer_metadata();
            return;
        }

        let mut nodes_to_del = Vec::new();

        let start_split = self.position_in_buffer(start_node, start_pos.remainder);
        self.delete_node_tail(start_node, start_split);
        if self.rb.piece(start_node).length == 0 {
            nodes_to_del.push(start_node);
        }

        let end_split = self.position_in_buffer(end_node, end_pos.remainder);
        self.delete_node_head(end_node, end_split);
        if self.rb.piece(end_node).length == 0 {
            nodes_to_del.push(end_node);
        }

        // delete the nodes in between
        let mut node = self.rb.next(start_node);
        while node != NIL && node != end_node {
            nodes_to_del.push(node);
            node = self.rb.next(node);
        }

        let prev = if self.rb.piece(start_node).length == 0 {
            self.rb.prev(start_node)
        } else {
            start_node
        };
        self.delete_nodes(&nodes_to_del);
        self.validate_crlf_with_next_node(prev);
        self.compute_buffer_metadata();
    }

    // --- internals: lookups -------------------------------------------------------------

    /// Whether the `count` units at `offset` contain a line break.
    fn range_has_break(&self, offset: usize, count: usize) -> bool {
        let start = self.position_at(offset);
        let end = self.position_at(offset + count);
        self.value_in_range(start, end, "\n").contains('\n')
    }

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

    /// The node containing UTF-16 `offset` (`nodeAt`). `None` past the end.
    fn node_at(&self, mut offset: usize) -> Option<NodePosition> {
        let mut x = self.rb.root;
        let mut node_start = 0;
        while x != NIL {
            let n = *self.rb.node(x);
            if n.size_left > offset {
                x = n.left;
            } else if n.size_left + n.piece.length >= offset {
                node_start += n.size_left;
                return Some(NodePosition {
                    node: x,
                    remainder: offset - n.size_left,
                    node_start_offset: node_start,
                });
            } else {
                offset -= n.size_left + n.piece.length;
                node_start += n.size_left + n.piece.length;
                x = n.right;
            }
        }
        None
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

    /// The UTF-16 length of `node`'s piece is `remainder`-based; this returns the code unit at
    /// `offset` into the piece, if the piece has line feeds and the unit is ASCII
    /// (`nodeCharCodeAt`).
    fn node_char_code_at(&self, node: usize, offset: usize) -> Option<u8> {
        let piece = self.rb.piece(node);
        if piece.line_feed_cnt < 1 {
            return None;
        }
        let cursor = self.position_in_buffer(node, offset);
        self.buffers[piece.buffer_index].ascii_at(cursor)
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

    // --- internals: node edits ------------------------------------------------------------

    fn should_check_crlf(&self) -> bool {
        !(self.eol_normalized && self.eol == EndOfLineSequence::Lf)
    }

    /// Whether the node's text begins with `\n` at a line start (`startWithLF` for nodes).
    fn node_starts_with_lf(&self, node: usize) -> bool {
        if node == NIL {
            return false;
        }
        let piece = self.rb.piece(node);
        if piece.line_feed_cnt == 0 {
            return false;
        }
        let buffer = &self.buffers[piece.buffer_index];
        let starts = buffer.line_starts();
        let line = piece.start.line;
        if line == starts.len() - 1 {
            // last line: there is no line feed at its end
            return false;
        }
        let start_offset = starts[line] + piece.start.column;
        if starts[line + 1] > start_offset + 1 {
            return false;
        }
        buffer.ascii_at(piece.start) == Some(b'\n')
    }

    /// Whether the node's text ends with `\r` (`endWithCR` for nodes).
    fn node_ends_with_cr(&self, node: usize) -> bool {
        if node == NIL {
            return false;
        }
        let piece = self.rb.piece(node);
        if piece.line_feed_cnt == 0 {
            return false;
        }
        self.node_char_code_at(node, piece.length - 1) == Some(b'\r')
    }

    /// Splits off `\r\n` at a node boundary by moving the `\n` into the previous node
    /// (`validateCRLFWithPrevNode`).
    fn validate_crlf_with_prev_node(&mut self, next: usize) {
        if self.should_check_crlf() && self.node_starts_with_lf(next) {
            let node = self.rb.prev(next);
            if self.node_ends_with_cr(node) {
                self.fix_crlf(node, next);
            }
        }
    }

    /// Joins a `\r` at the end of a node with a `\n` at the start of the next one
    /// (`validateCRLFWithNextNode`).
    fn validate_crlf_with_next_node(&mut self, node: usize) {
        if self.should_check_crlf() && self.node_ends_with_cr(node) {
            let next = self.rb.next(node);
            if self.node_starts_with_lf(next) {
                self.fix_crlf(node, next);
            }
        }
    }

    /// Moves the `\r` / `\n` pair across the boundary between `prev` and `next` into a `\r\n`
    /// piece of its own (`fixCRLF`).
    fn fix_crlf(&mut self, prev: usize, next: usize) {
        let mut nodes_to_del = Vec::new();

        // update prev: drop its trailing \r
        let prev_piece = self.rb.piece(prev);
        let new_end = if prev_piece.end.column == 0 {
            // the last line ended with \r, not \r\n
            let starts = self.buffers[prev_piece.buffer_index].line_starts();
            Cursor {
                line: prev_piece.end.line - 1,
                column: starts[prev_piece.end.line] - starts[prev_piece.end.line - 1] - 1,
            }
        } else {
            Cursor {
                line: prev_piece.end.line,
                column: prev_piece.end.column - 1,
            }
        };
        self.rb.set_piece(
            prev,
            Piece::new(
                prev_piece.buffer_index,
                prev_piece.start,
                new_end,
                prev_piece.line_feed_cnt - 1,
                prev_piece.length - 1,
            ),
        );
        self.rb.update_metadata(prev, -1, -1);
        if self.rb.piece(prev).length == 0 {
            nodes_to_del.push(prev);
        }

        // update next: drop its leading \n
        let next_piece = self.rb.piece(next);
        let new_start = Cursor {
            line: next_piece.start.line + 1,
            column: 0,
        };
        let new_lf = self.line_feed_cnt(next_piece.buffer_index, new_start, next_piece.end);
        self.rb.set_piece(
            next,
            Piece::new(
                next_piece.buffer_index,
                new_start,
                next_piece.end,
                new_lf,
                next_piece.length - 1,
            ),
        );
        self.rb.update_metadata(next, -1, -1);
        if self.rb.piece(next).length == 0 {
            nodes_to_del.push(next);
        }

        // the \r\n piece goes between them
        let pieces = self.create_new_pieces("\r\n");
        self.rb.insert_right(prev, pieces[0]);
        self.delete_nodes(&nodes_to_del);
    }

    /// If `value` ends with `\r` and the node after `node` starts with `\n`, moves that `\n`
    /// into `value`. Returns whether it did; the caller then appends `\n` to `value`
    /// (`adjustCarriageReturnFromNext`).
    fn adjust_cr_from_next(&mut self, value: &str, node: usize) -> bool {
        if self.should_check_crlf() && ends_with_cr(value) {
            let next = self.rb.next(node);
            if self.node_starts_with_lf(next) {
                if self.rb.piece(next).length == 1 {
                    self.rb.delete(next);
                } else {
                    let piece = self.rb.piece(next);
                    let new_start = Cursor {
                        line: piece.start.line + 1,
                        column: 0,
                    };
                    let new_lf = self.line_feed_cnt(piece.buffer_index, new_start, piece.end);
                    self.rb.set_piece(
                        next,
                        Piece::new(
                            piece.buffer_index,
                            new_start,
                            piece.end,
                            new_lf,
                            piece.length - 1,
                        ),
                    );
                    self.rb.update_metadata(next, -1, -1);
                }
                return true;
            }
        }
        false
    }

    /// Inserts `value` at the start of `node` (`insertContentToNodeLeft`).
    fn insert_content_to_node_left(&mut self, mut value: String, node: usize) {
        let mut nodes_to_del = Vec::new();
        if self.should_check_crlf() && ends_with_cr(&value) && self.node_starts_with_lf(node) {
            // move the \n into the new node
            let piece = self.rb.piece(node);
            let new_start = Cursor {
                line: piece.start.line + 1,
                column: 0,
            };
            let new_lf = self.line_feed_cnt(piece.buffer_index, new_start, piece.end);
            self.rb.set_piece(
                node,
                Piece::new(
                    piece.buffer_index,
                    new_start,
                    piece.end,
                    new_lf,
                    piece.length - 1,
                ),
            );
            value.push('\n');
            self.rb.update_metadata(node, -1, -1);
            if self.rb.piece(node).length == 0 {
                nodes_to_del.push(node);
            }
        }

        let new_pieces = self.create_new_pieces(&value);
        let (last, rest) = new_pieces.split_last().expect("at least one piece");
        let mut new_node = self.rb.insert_left(node, *last);
        for piece in rest.iter().rev() {
            new_node = self.rb.insert_left(new_node, *piece);
        }
        self.validate_crlf_with_prev_node(new_node);
        self.delete_nodes(&nodes_to_del);
    }

    /// Inserts `value` right after `node` (`insertContentToNodeRight`).
    fn insert_content_to_node_right(&mut self, value: &str, node: usize) {
        let mut value = value.to_string();
        if self.adjust_cr_from_next(&value, node) {
            // the \n moved into the new piece
            value.push('\n');
        }

        let new_pieces = self.create_new_pieces(&value);
        let new_node = self.rb.insert_right(node, new_pieces[0]);
        let mut tmp = new_node;
        for piece in new_pieces.into_iter().skip(1) {
            tmp = self.rb.insert_right(tmp, piece);
        }
        self.validate_crlf_with_prev_node(new_node);
    }

    /// Appends `value` to the change buffer and extends the last piece to cover it
    /// (`appendToNode`).
    fn append_to_node(&mut self, node: usize, value: &str) {
        let mut value = value.to_string();
        if self.adjust_cr_from_next(&value, node) {
            value.push('\n');
        }

        let hit_crlf =
            self.should_check_crlf() && starts_with_lf(&value) && self.node_ends_with_cr(node);
        let start_offset = self.buffers[0].utf16_len();
        if hit_crlf {
            // the \r at the end of the change buffer is now part of a \r\n
            let starts = self.buffers[0].line_starts();
            let prev_start = starts[starts.len() - 2];
            self.buffers[0].pop_line_start();
            self.last_change_buffer_pos = Cursor {
                line: self.last_change_buffer_pos.line - 1,
                column: start_offset - prev_start,
            };
        }
        self.buffers[0].append("", &value);

        let buffer = &self.buffers[0];
        let end_index = buffer.line_starts().len() - 1;
        let end = Cursor {
            line: end_index,
            column: buffer.utf16_len() - buffer.line_starts()[end_index],
        };
        let piece = self.rb.piece(node);
        let value_len = utf16_len(&value);
        let new_lf = self.line_feed_cnt(0, piece.start, end);
        let lf_delta = new_lf as isize - piece.line_feed_cnt as isize;
        self.rb.set_piece(
            node,
            Piece::new(
                piece.buffer_index,
                piece.start,
                end,
                new_lf,
                piece.length + value_len,
            ),
        );
        self.last_change_buffer_pos = end;
        self.rb.update_metadata(node, value_len as isize, lf_delta);
    }

    /// Shortens `node` to end at `start` and adds a node for the text from `end` to its
    /// old end (`shrinkNode`).
    fn shrink_node(&mut self, node: usize, start: Cursor, end: Cursor) {
        let piece = self.rb.piece(node);
        let old_length = piece.length;
        let old_lf = piece.line_feed_cnt;

        let new_lf = self.line_feed_cnt(piece.buffer_index, piece.start, start);
        let new_length = self.offset_in_buffer(piece.buffer_index, start)
            - self.offset_in_buffer(piece.buffer_index, piece.start);
        self.rb.set_piece(
            node,
            Piece::new(piece.buffer_index, piece.start, start, new_lf, new_length),
        );
        self.rb.update_metadata(
            node,
            new_length as isize - old_length as isize,
            new_lf as isize - old_lf as isize,
        );

        let right = Piece::new(
            piece.buffer_index,
            end,
            piece.end,
            self.line_feed_cnt(piece.buffer_index, end, piece.end),
            self.offset_in_buffer(piece.buffer_index, piece.end)
                - self.offset_in_buffer(piece.buffer_index, end),
        );
        let new_node = self.rb.insert_right(node, right);
        self.validate_crlf_with_prev_node(new_node);
    }

    /// Moves the end of `node` back to `pos` (`deleteNodeTail`).
    fn delete_node_tail(&mut self, node: usize, pos: Cursor) {
        let piece = self.rb.piece(node);
        let original_lf = piece.line_feed_cnt;
        let original_end_offset = self.offset_in_buffer(piece.buffer_index, piece.end);
        let new_end_offset = self.offset_in_buffer(piece.buffer_index, pos);
        let new_lf = self.line_feed_cnt(piece.buffer_index, piece.start, pos);
        let size_delta = new_end_offset as isize - original_end_offset as isize;
        let lf_delta = new_lf as isize - original_lf as isize;
        self.rb.set_piece(
            node,
            Piece::new(
                piece.buffer_index,
                piece.start,
                pos,
                new_lf,
                (piece.length as isize + size_delta) as usize,
            ),
        );
        self.rb.update_metadata(node, size_delta, lf_delta);
    }

    /// Moves the start of `node` forward to `pos` (`deleteNodeHead`).
    fn delete_node_head(&mut self, node: usize, pos: Cursor) {
        let piece = self.rb.piece(node);
        let original_lf = piece.line_feed_cnt;
        let original_start_offset = self.offset_in_buffer(piece.buffer_index, piece.start);
        let new_start_offset = self.offset_in_buffer(piece.buffer_index, pos);
        let new_lf = self.line_feed_cnt(piece.buffer_index, pos, piece.end);
        let size_delta = original_start_offset as isize - new_start_offset as isize;
        let lf_delta = new_lf as isize - original_lf as isize;
        self.rb.set_piece(
            node,
            Piece::new(
                piece.buffer_index,
                pos,
                piece.end,
                new_lf,
                (piece.length as isize + size_delta) as usize,
            ),
        );
        self.rb.update_metadata(node, size_delta, lf_delta);
    }

    fn delete_nodes(&mut self, nodes: &[usize]) {
        for &node in nodes {
            self.rb.delete(node);
        }
    }

    /// Stores `text` in the buffers and returns the pieces that cover it (`createNewPieces`).
    /// Text longer than [`AVERAGE_BUFFER_SIZE`] units goes to new buffers of that size; the
    /// rest is appended to the change buffer.
    fn create_new_pieces(&mut self, text: &str) -> Vec<Piece> {
        if utf16_len(text) > AVERAGE_BUFFER_SIZE {
            let mut pieces = Vec::new();
            let mut rest = text;
            while utf16_len(rest) > AVERAGE_BUFFER_SIZE {
                let cut = split_point(rest);
                let (head, tail) = rest.split_at(cut);
                pieces.push(self.push_buffer(head));
                rest = tail;
            }
            pieces.push(self.push_buffer(rest));
            return pieces;
        }

        let mut start_offset = self.buffers[0].utf16_len();
        let mut start = self.last_change_buffer_pos;
        let buffer = &self.buffers[0];
        let ends_with_break = buffer.line_starts().last() == Some(&start_offset);
        if ends_with_break
            && start_offset != 0
            && starts_with_lf(text)
            && buffer.text().ends_with('\r')
        {
            // The \r at the end of the change buffer and the \n of `text` form one \r\n.
            // The `_` placeholder keeps the line starts aligned; the \n is then not a line start.
            self.last_change_buffer_pos = Cursor {
                line: self.last_change_buffer_pos.line,
                column: self.last_change_buffer_pos.column + 1,
            };
            start = self.last_change_buffer_pos;
            self.buffers[0].append("_", text);
            start_offset += 1;
        } else {
            self.buffers[0].append("", text);
        }

        let buffer = &self.buffers[0];
        let end_offset = buffer.utf16_len();
        let end_index = buffer.line_starts().len() - 1;
        let end = Cursor {
            line: end_index,
            column: end_offset - buffer.line_starts()[end_index],
        };
        let line_feed_cnt = self.line_feed_cnt(0, start, end);
        self.last_change_buffer_pos = end;
        vec![Piece::new(
            0,
            start,
            end,
            line_feed_cnt,
            end_offset - start_offset,
        )]
    }

    /// Adds `text` as a new read-only buffer and returns the piece covering all of it.
    fn push_buffer(&mut self, text: &str) -> Piece {
        let buffer = StringBuffer::new(text.to_string());
        let index = self.buffers.len();
        let starts = buffer.line_starts();
        let last = starts.len() - 1;
        let end = Cursor {
            line: last,
            column: buffer.utf16_len() - starts[last],
        };
        let piece = Piece::new(index, Cursor::default(), end, last, buffer.utf16_len());
        self.buffers.push(buffer);
        piece
    }
}

fn starts_with_lf(s: &str) -> bool {
    s.as_bytes().first() == Some(&b'\n')
}

fn ends_with_cr(s: &str) -> bool {
    s.as_bytes().last() == Some(&b'\r')
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

/// Byte index at which to split `text` for a chunk of at most [`AVERAGE_BUFFER_SIZE`] units.
/// A trailing `\r` or a surrogate pair that would be cut is kept for the next chunk.
fn split_point(text: &str) -> usize {
    let target = AVERAGE_BUFFER_SIZE - 1;
    let mut units = 0;
    for (i, ch) in text.char_indices() {
        let width = ch.len_utf16();
        if units + width > target {
            // this code point contains unit `target`
            return if units == target && (ch == '\r' || width == 2) {
                i
            } else {
                i + ch.len_utf8()
            };
        }
        units += width;
    }
    text.len()
}

#[cfg(test)]
impl PieceTree {
    /// Checks the tree invariants (`assertTreeInvariants`). Panics on violation.
    pub(crate) fn assert_valid(&self) {
        self.rb.assert_valid();
    }
}
