//! Builds a [`PieceTree`] from text that arrives in chunks.
//!
//! Ported from `src/vs/editor/common/model/pieceTreeTextBuffer/pieceTreeTextBufferBuilder.ts`.
//! The builder strips a leading UTF-8 BOM, counts `\r`, `\n` and `\r\n` to choose the line
//! terminator, and optionally normalizes every chunk to that terminator. A trailing `\r` is
//! held back until the next chunk shows whether it starts a `\r\n`, so a CRLF split across two
//! chunks is counted once.

use vscode_api::editor::EndOfLineSequence;

use crate::piece::{StringBuffer, normalize_breaks};
use crate::piece_tree::PieceTree;
use crate::text_buffer::PieceTreeTextBuffer;

/// The byte order mark, as a `char` (`UTF8_BOM_CHARACTER`).
const BOM: char = '\u{feff}';

/// Accumulates chunks of text (`PieceTreeTextBufferBuilder`).
#[derive(Debug, Default)]
pub struct TextBufferBuilder {
    chunks: Vec<StringBuffer>,
    bom: String,
    has_previous_cr: bool,
    cr: usize,
    lf: usize,
    crlf: usize,
}

impl TextBufferBuilder {
    /// Creates an empty builder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds the next chunk of text (`acceptChunk`). A leading BOM is removed and remembered.
    pub fn accept_chunk(&mut self, chunk: &str) {
        if chunk.is_empty() {
            return;
        }
        let mut chunk = chunk;
        if self.chunks.is_empty() {
            if let Some(rest) = chunk.strip_prefix(BOM) {
                self.bom = BOM.to_string();
                chunk = rest;
            }
        }

        if let Some(head) = chunk.strip_suffix('\r') {
            self.accept_chunk1(head, false);
            self.has_previous_cr = true;
        } else {
            self.accept_chunk1(chunk, false);
            self.has_previous_cr = false;
        }
    }

    /// Finishes the input and returns a factory for the buffer (`finish`).
    /// `normalize_eol` says whether `create` should rewrite the line breaks to one terminator.
    pub fn finish(mut self, normalize_eol: bool) -> TextBufferFactory {
        self.finish_pending();
        TextBufferFactory {
            chunks: self.chunks,
            bom: self.bom,
            cr: self.cr,
            lf: self.lf,
            crlf: self.crlf,
            normalize_eol,
        }
    }

    /// Prepends the held-back `\r` to `chunk` and stores the result.
    fn accept_chunk1(&mut self, chunk: &str, allow_empty: bool) {
        if !allow_empty && chunk.is_empty() {
            return;
        }
        if self.has_previous_cr {
            let joined = format!("\r{chunk}");
            self.accept_chunk2(&joined);
        } else {
            self.accept_chunk2(chunk);
        }
    }

    fn accept_chunk2(&mut self, chunk: &str) {
        let (cr, lf, crlf) = count_breaks(chunk);
        self.chunks.push(StringBuffer::new(chunk.to_string()));
        self.cr += cr;
        self.lf += lf;
        self.crlf += crlf;
    }

    /// Stores the held-back `\r` at the end of the input (`_finish`).
    fn finish_pending(&mut self) {
        if self.chunks.is_empty() {
            // Empty input: an empty chunk gives the held `\r`, if any, somewhere to go.
            self.chunks.push(StringBuffer::new(String::new()));
        }
        if self.has_previous_cr {
            self.has_previous_cr = false;
            let last = self.chunks.last_mut().expect("a chunk exists");
            let mut text = last.text().to_string();
            text.push('\r');
            *last = StringBuffer::new(text);
            self.cr += 1;
        }
    }
}

/// Counts `(cr, lf, crlf)` line breaks in `text`. A `\r\n` counts once, as `crlf`.
fn count_breaks(text: &str) -> (usize, usize, usize) {
    let (mut cr, mut lf, mut crlf) = (0, 0, 0);
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '\r' => {
                if chars.peek() == Some(&'\n') {
                    chars.next();
                    crlf += 1;
                } else {
                    cr += 1;
                }
            }
            '\n' => lf += 1,
            _ => {}
        }
    }
    (cr, lf, crlf)
}

/// The finished input, ready to become a [`PieceTree`] (`PieceTreeTextBufferFactory`).
#[derive(Debug)]
pub struct TextBufferFactory {
    chunks: Vec<StringBuffer>,
    bom: String,
    cr: usize,
    lf: usize,
    crlf: usize,
    normalize_eol: bool,
}

impl TextBufferFactory {
    /// The BOM that was stripped from the input, or the empty string.
    pub fn bom(&self) -> &str {
        &self.bom
    }

    /// Builds the text buffer, keeping the BOM (`create`).
    pub fn create_text_buffer(self, default_eol: EndOfLineSequence) -> PieceTreeTextBuffer {
        let bom = self.bom.clone();
        PieceTreeTextBuffer::new(self.create(default_eol), bom)
    }

    /// Builds the tree. The terminator is the majority one in the input; when the input has no
    /// line breaks, `default_eol` is used. With `normalize_eol`, every break is rewritten to the
    /// chosen terminator and the tree is marked as normalized.
    pub fn create(self, default_eol: EndOfLineSequence) -> PieceTree {
        let eol = self.eol(default_eol);
        let mut chunks = self.chunks;
        let needs_rewrite = self.normalize_eol
            && ((eol == EndOfLineSequence::CrLf && (self.cr > 0 || self.lf > 0))
                || (eol == EndOfLineSequence::Lf && (self.cr > 0 || self.crlf > 0)));
        if needs_rewrite {
            for chunk in &mut chunks {
                let text = normalize_breaks(chunk.text(), eol.as_str());
                *chunk = StringBuffer::new(text);
            }
        }
        PieceTree::new(chunks, eol, self.normalize_eol)
    }

    fn eol(&self, default_eol: EndOfLineSequence) -> EndOfLineSequence {
        let total = self.cr + self.lf + self.crlf;
        let total_cr = self.cr + self.crlf;
        if total == 0 {
            // an empty file, or one line without a break
            return default_eol;
        }
        if 2 * total_cr > total {
            // more than half of the breaks are \r\n
            EndOfLineSequence::CrLf
        } else {
            EndOfLineSequence::Lf
        }
    }
}
