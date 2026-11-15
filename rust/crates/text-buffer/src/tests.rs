//! Tests for the builder and the read-only piece tree, ported from
//! `src/vs/editor/test/common/model/pieceTreeTextBuffer/pieceTreeTextBuffer.test.ts`.
//!
//! The model is a `String` with the same text. Every check compares the tree with the model:
//! raw text, line count, line contents, and the position of every valid offset. The randomized
//! tests are seeded, so they are reproducible; the upstream versions used `Math.random`.

use vscode_api::editor::EndOfLineSequence;

use crate::{PieceTree, TextBufferBuilder};

/// Builds a tree from `chunks` the way `createTextBuffer` does in the upstream tests.
fn build(chunks: &[&str], normalize: bool) -> PieceTree {
    let mut builder = TextBufferBuilder::new();
    for chunk in chunks {
        builder.accept_chunk(chunk);
    }
    builder.finish(normalize).create(EndOfLineSequence::Lf)
}

/// Lines of `text`, split at `\r\n`, `\r` and `\n` (`splitLines`). Always at least one line.
fn model_lines(text: &str) -> Vec<String> {
    let mut lines = Vec::new();
    let mut current = String::new();
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '\r' => {
                if chars.peek() == Some(&'\n') {
                    chars.next();
                }
                lines.push(std::mem::take(&mut current));
            }
            '\n' => lines.push(std::mem::take(&mut current)),
            other => current.push(other),
        }
    }
    lines.push(current);
    lines
}

/// Offsets where each line starts: `0` and the offset after every break.
fn model_line_starts(text: &str) -> Vec<usize> {
    let bytes = text.as_bytes();
    let mut starts = vec![0];
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'\r' if bytes.get(i + 1) == Some(&b'\n') => {
                i += 2;
                starts.push(i);
            }
            b'\r' | b'\n' => {
                i += 1;
                starts.push(i);
            }
            _ => i += 1,
        }
    }
    starts
}

/// The 1-based position of `offset` in `text`. Requires ASCII text, so bytes are UTF-16 units.
fn model_position(text: &str, offset: usize) -> (usize, usize) {
    let starts = model_line_starts(text);
    let index = starts.partition_point(|&s| s <= offset) - 1;
    (index + 1, offset - starts[index] + 1)
}

/// Whether `offset` can be a cursor position: it may not fall between the `\r` and `\n` of a
/// `\r\n`, which is a single line break.
fn is_valid_offset(text: &str, offset: usize) -> bool {
    let bytes = text.as_bytes();
    !(offset > 0 && bytes[offset - 1] == b'\r' && bytes.get(offset) == Some(&b'\n'))
}

/// Checks the tree against `model`: raw text, lines, the position of every valid offset, and
/// the internal invariants. The model must be ASCII.
fn check_against_model(tree: &PieceTree, model: &str) {
    assert_eq!(tree.raw_content(), model);
    assert_eq!(tree.length(), model.len());
    tree.assert_valid();

    let lines = model_lines(model);
    assert_eq!(tree.line_count(), lines.len(), "line count of {model:?}");
    assert_eq!(tree.lines_content(), lines, "lines of {model:?}");

    for offset in 0..=model.len() {
        if !is_valid_offset(model, offset) {
            continue;
        }
        let (line, column) = model_position(model, offset);
        assert_eq!(
            tree.position_at(offset),
            (line, column),
            "position of {offset}"
        );
        assert_eq!(
            tree.offset_at(line, column),
            offset,
            "offset of {line}:{column}"
        );
    }
}

/// `suite('buffer api')`: text read back from one chunk.
#[test]
fn single_chunk_reads_back() {
    let tree = build(&["This is a document with some text."], false);
    check_against_model(&tree, "This is a document with some text.");
}

/// `suite('CRLF')` and the builder: a `\r\n` split across two chunks is one line break.
#[test]
fn crlf_split_across_chunks_is_one_break() {
    let tree = build(&["ab\r", "\ncd\r", "\r\nef"], false);
    check_against_model(&tree, "ab\r\ncd\r\r\nef");
    assert_eq!(tree.line_count(), 4);
    assert_eq!(tree.lines_content(), vec!["ab", "cd", "", "ef"]);
}

/// The builder strips a leading BOM and keeps it out of the text.
#[test]
fn leading_bom_is_stripped() {
    let mut builder = TextBufferBuilder::new();
    builder.accept_chunk("\u{feff}first\nsecond");
    let factory = builder.finish(false);
    assert_eq!(factory.bom(), "\u{feff}");
    let tree = factory.create(EndOfLineSequence::Lf);
    assert_eq!(tree.raw_content(), "first\nsecond");
}

/// The builder picks the majority terminator; with no breaks it uses the default.
#[test]
fn eol_is_the_majority_terminator() {
    let eol = |text: &str| {
        let mut builder = TextBufferBuilder::new();
        builder.accept_chunk(text);
        builder.finish(false).create(EndOfLineSequence::Lf).eol()
    };
    assert_eq!(eol("a\r\nb\r\nc\nd"), EndOfLineSequence::CrLf);
    assert_eq!(eol("a\nb\r\nc\nd"), EndOfLineSequence::Lf);
    assert_eq!(eol("no breaks"), EndOfLineSequence::Lf);
}

/// Normalizing rewrites every break to the chosen terminator and marks the tree normalized.
#[test]
fn normalize_rewrites_breaks_to_the_eol() {
    let mut builder = TextBufferBuilder::new();
    builder.accept_chunk("a\r\nb\rc\nd\r\n");
    let tree = builder.finish(true).create(EndOfLineSequence::Lf);
    assert_eq!(tree.eol(), EndOfLineSequence::CrLf);
    assert!(tree.is_eol_normalized());
    check_against_model(&tree, "a\r\nb\r\nc\r\nd\r\n");
    assert_eq!(tree.line_content(2), "b");
}

/// `suite('get text in range')`: `getContentInRange` and `get line content`.
#[test]
fn get_line_content_basic() {
    let tree = build(&["Hello\r\nWorld\nLast"], true);
    assert_eq!(tree.line_count(), 3);
    assert_eq!(tree.line_content(1), "Hello");
    assert_eq!(tree.line_content(2), "World");
    assert_eq!(tree.line_content(3), "Last");
    assert_eq!(tree.value_in_range((1, 3), (3, 3), "\n"), "llo\nWorld\nLa");
    assert_eq!(tree.value_in_range((1, 1), (1, 1), "\n"), "");
}

/// Line lengths on normalized text leave out the terminator (`getLineLength`).
#[test]
fn line_length_on_normalized_text() {
    let tree = build(&["ab\r\nc\r\n\r\ndef"], true);
    assert!(tree.is_eol_normalized());
    let lengths: Vec<usize> = (1..=tree.line_count())
        .map(|l| tree.line_length(l))
        .collect();
    assert_eq!(lengths, vec![2, 1, 0, 3]);
}

/// Columns and offsets count UTF-16 units: an astral character is two columns wide.
#[test]
fn non_ascii_columns_count_utf16_units() {
    let tree = build(&["a\u{1F600}b\n\u{e7}"], false);
    assert_eq!(tree.line_count(), 2);
    assert_eq!(tree.line_content(1), "a\u{1F600}b");
    assert_eq!(tree.line_content(2), "\u{e7}");
    assert_eq!(tree.position_at(3), (1, 4));
    assert_eq!(tree.offset_at(1, 4), 3);
    assert_eq!(tree.value_in_range((1, 2), (1, 4), "\n"), "\u{1F600}");
    assert_eq!(tree.length(), 6);
}

/// Seeded xorshift generator: reproducible stand-in for `Math.random` in the upstream fuzzers.
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        Self(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1)
    }

    fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }

    /// A value in `0..bound`. `bound` must be non-zero.
    fn below(&mut self, bound: usize) -> usize {
        (self.next_u64() % bound as u64) as usize
    }
}

/// Random text with breaks, split into random chunks, reads back as the same text. The chunks
/// split `\r\n` pairs and `\r` often, which the builder must handle.
#[test]
fn random_chunks_read_back_as_the_same_text() {
    const ALPHABET: &[u8] = b"abcdefghijklmnopqrstuvwxyz\r\n";
    for seed in 1..=300 {
        let mut rng = Rng::new(seed);
        let length = rng.below(120);
        let text: String = (0..length)
            .map(|_| ALPHABET[rng.below(ALPHABET.len())] as char)
            .collect();

        let mut chunks = Vec::new();
        let mut rest = text.as_str();
        while !rest.is_empty() {
            let cut = 1 + rng.below(rest.len());
            let (head, tail) = rest.split_at(cut);
            chunks.push(head);
            rest = tail;
        }

        let tree = build(&chunks, false);
        check_against_model(&tree, &text);
    }
}
