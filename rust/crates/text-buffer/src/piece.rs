//! Pieces and the append-only string buffers they point into.
//!
//! Ported from `src/vs/editor/common/model/pieceTreeTextBuffer/pieceTreeBase.ts` (`Piece`,
//! `StringBuffer`, `createLineStarts`, `createLineStartsFast`).
//!
//! Positions inside a buffer are UTF-16 code units, as in VS Code (ADR 0003). Buffers hold UTF-8
//! text, so every line start is kept twice: in UTF-16 units, which all cursor arithmetic uses, and
//! in bytes, which is used to slice text. For ASCII-only buffers the two are equal and no scan is
//! needed.

/// A line/column pair inside one buffer (`BufferCursor`). Both fields are 0-based.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Cursor {
    pub(crate) line: usize,
    pub(crate) column: usize,
}

/// A slice of one buffer, with cached line-feed count and UTF-16 length (`Piece`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Piece {
    pub(crate) buffer_index: usize,
    pub(crate) start: Cursor,
    pub(crate) end: Cursor,
    pub(crate) line_feed_cnt: usize,
    pub(crate) length: usize,
}

impl Piece {
    pub(crate) fn new(
        buffer_index: usize,
        start: Cursor,
        end: Cursor,
        line_feed_cnt: usize,
        length: usize,
    ) -> Self {
        Self {
            buffer_index,
            start,
            end,
            line_feed_cnt,
            length,
        }
    }
}

/// Length of `s` in UTF-16 code units.
pub(crate) fn utf16_len(s: &str) -> usize {
    if s.is_ascii() {
        s.len()
    } else {
        s.encode_utf16().count()
    }
}

/// Byte index of the code point that contains UTF-16 unit `units` of `s`. A position inside a
/// surrogate pair rounds down to the start of the pair. Returns `s.len()` past the end.
pub(crate) fn utf16_to_byte(s: &str, units: usize) -> usize {
    let mut acc = 0;
    for (i, ch) in s.char_indices() {
        let width = ch.len_utf16();
        if acc + width > units {
            return i;
        }
        acc += width;
    }
    s.len()
}

/// Line starts of `text` in UTF-16 units and in bytes, each beginning with 0 (`createLineStartsFast`).
/// `\r\n`, `\r` and `\n` each end a line; `\r\n` counts once.
pub(crate) fn line_starts(text: &str) -> (Vec<usize>, Vec<usize>) {
    let mut utf16 = vec![0];
    let mut bytes = vec![0];
    let mut units = 0;
    let mut chars = text.char_indices().peekable();
    while let Some((i, ch)) = chars.next() {
        match ch {
            '\r' => {
                if let Some(&(_, '\n')) = chars.peek() {
                    chars.next();
                    units += 2;
                    utf16.push(units);
                    bytes.push(i + 2);
                } else {
                    units += 1;
                    utf16.push(units);
                    bytes.push(i + 1);
                }
            }
            '\n' => {
                units += 1;
                utf16.push(units);
                bytes.push(i + 1);
            }
            other => units += other.len_utf16(),
        }
    }
    (utf16, bytes)
}

/// Replaces every `\r\n`, `\r` and `\n` in `text` with `eol` (`replace(/\r\n|\r|\n/g, eol)`).
pub(crate) fn normalize_breaks(text: &str, eol: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '\r' => {
                if chars.peek() == Some(&'\n') {
                    chars.next();
                }
                out.push_str(eol);
            }
            '\n' => out.push_str(eol),
            other => out.push(other),
        }
    }
    out
}

/// Append-only text with its line starts (`StringBuffer`). Buffer 0 is the change buffer.
#[derive(Clone, Debug)]
pub(crate) struct StringBuffer {
    text: String,
    utf16_len: usize,
    ascii: bool,
    line_starts: Vec<usize>,
    byte_line_starts: Vec<usize>,
}

impl StringBuffer {
    /// Wraps `text`, computing its line starts.
    pub(crate) fn new(text: String) -> Self {
        let (line_starts, byte_line_starts) = line_starts(&text);
        Self {
            utf16_len: utf16_len(&text),
            ascii: text.is_ascii(),
            text,
            line_starts,
            byte_line_starts,
        }
    }

    pub(crate) fn text(&self) -> &str {
        &self.text
    }

    /// Length in UTF-16 code units.
    pub(crate) fn utf16_len(&self) -> usize {
        self.utf16_len
    }

    /// UTF-16 line starts, one per line.
    pub(crate) fn line_starts(&self) -> &[usize] {
        &self.line_starts
    }

    /// Length of the UTF-16 offset of `cursor` from the start of the buffer.
    pub(crate) fn offset(&self, cursor: Cursor) -> usize {
        self.line_starts[cursor.line] + cursor.column
    }

    /// Byte index of `cursor`. Scans only inside the cursor's line.
    pub(crate) fn byte_index(&self, cursor: Cursor) -> usize {
        if self.ascii {
            return self.offset(cursor);
        }
        let base = self.byte_line_starts[cursor.line];
        base + utf16_to_byte(&self.text[base..], cursor.column)
    }

    /// Text between two cursors. `from` must not come after `to`.
    pub(crate) fn slice(&self, from: Cursor, to: Cursor) -> &str {
        &self.text[self.byte_index(from)..self.byte_index(to)]
    }

    /// The code unit at `cursor` if it is ASCII (`charCodeAt`). `None` past the end.
    pub(crate) fn ascii_at(&self, cursor: Cursor) -> Option<u8> {
        self.text.as_bytes().get(self.byte_index(cursor)).copied()
    }

    /// Appends `prefix` and `text`, adding the line starts of `text` after its first line.
    pub(crate) fn append(&mut self, prefix: &str, text: &str) {
        let (utf16, bytes) = line_starts(text);
        let utf16_shift = self.utf16_len + utf16_len(prefix);
        let byte_shift = self.text.len() + prefix.len();
        self.line_starts
            .extend(utf16.iter().skip(1).map(|s| s + utf16_shift));
        self.byte_line_starts
            .extend(bytes.iter().skip(1).map(|s| s + byte_shift));
        self.text.push_str(prefix);
        self.text.push_str(text);
        self.utf16_len += utf16_len(prefix) + utf16_len(text);
        self.ascii = self.ascii && prefix.is_ascii() && text.is_ascii();
    }

    /// Drops the last line start, used when a `\r` at the end of the buffer becomes part of
    /// a `\r\n` (`Vec.pop` on `lineStarts` in `appendToNode`).
    pub(crate) fn pop_line_start(&mut self) {
        self.line_starts.pop();
        self.byte_line_starts.pop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_starts_count_crlf_once() {
        // a \r\n, a lone \r, a \n, and a last line without a break
        let (utf16, bytes) = line_starts("a\r\nb\rc\nd");
        assert_eq!(utf16, vec![0, 3, 5, 7]);
        assert_eq!(bytes, vec![0, 3, 5, 7]);
    }

    #[test]
    fn line_starts_count_utf16_units_and_bytes_separately() {
        // U+1F600 is 4 bytes and 2 UTF-16 units
        let (utf16, bytes) = line_starts("\u{1F600}\n\u{e7}");
        assert_eq!(utf16, vec![0, 3]);
        assert_eq!(bytes, vec![0, 5]);
    }

    #[test]
    fn utf16_to_byte_rounds_down_inside_a_pair() {
        let s = "a\u{1F600}b";
        assert_eq!(utf16_to_byte(s, 0), 0);
        assert_eq!(utf16_to_byte(s, 1), 1);
        assert_eq!(utf16_to_byte(s, 2), 1, "inside the pair rounds down");
        assert_eq!(utf16_to_byte(s, 3), 5);
        assert_eq!(utf16_to_byte(s, 99), s.len());
    }

    #[test]
    fn normalize_breaks_replaces_each_terminator_once() {
        assert_eq!(normalize_breaks("a\r\nb\rc\nd", "\n"), "a\nb\nc\nd");
        assert_eq!(normalize_breaks("a\r\nb", "\r\n"), "a\r\nb");
        assert_eq!(normalize_breaks("\r\r\n\n", "\n"), "\n\n\n");
    }

    #[test]
    fn append_shifts_new_line_starts() {
        let mut buffer = StringBuffer::new("ab\n".to_string());
        buffer.append("", "c\nd");
        assert_eq!(buffer.line_starts(), &[0, 3, 5]);
        assert_eq!(buffer.utf16_len(), 6);
        assert_eq!(
            buffer.slice(Cursor { line: 1, column: 0 }, Cursor { line: 2, column: 1 }),
            "c\nd"
        );
    }
}
