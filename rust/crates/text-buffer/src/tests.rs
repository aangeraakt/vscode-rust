//! Tests for the piece tree, ported from
//! `src/vs/editor/test/common/model/pieceTreeTextBuffer/pieceTreeTextBuffer.test.ts`.
//!
//! Each deterministic upstream test is a list of edits applied to both the tree and a `String`
//! model. After every edit the tree must match the model: raw text, line count, line contents,
//! and the invariants of the red-black tree. The randomized tests are seeded, so they are
//! reproducible; the upstream versions used `Math.random`.

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

/// One edit: `Insert(offset, text)` or `Delete(offset, count)`, in UTF-16 units (ASCII here).
#[derive(Clone, Copy)]
enum Op {
    Insert(usize, &'static str),
    Delete(usize, usize),
}

/// Applies `ops` to a tree built from `initial` and to a `String` model, checking the two agree
/// after every edit. Mirrors the `pieceTable.insert` / `str = ...` pairs of the upstream tests.
fn run_ops(initial: &str, ops: &[Op]) {
    let mut tree = build(&[initial], false);
    let mut model = initial.to_string();
    check_against_model(&tree, &model);
    for op in ops {
        match *op {
            Op::Insert(offset, text) => {
                tree.insert(offset, text, false);
                model.insert_str(offset, text);
            }
            Op::Delete(offset, count) => {
                tree.delete(offset, count);
                model.replace_range(offset..offset + count, "");
            }
        }
        check_against_model(&tree, &model);
    }
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

/// The 1-based position of `offset` in `text`.
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

/// Checks the tree against the model: the raw text, the lines, every valid position, and the
/// internal invariants.
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

/// `pieceTreeTextBuffer.test.ts`: `suite('inserts and deletes')`.
#[test]
fn basic_insert_delete() {
    run_ops(
        "This is a document with some text.",
        &[
            Op::Insert(34, "This is some more text to insert at offset 34."),
            Op::Delete(42, 5),
        ],
    );
}

#[test]
fn more_inserts() {
    run_ops(
        "",
        &[
            Op::Insert(0, "AAA"),
            Op::Insert(0, "BBB"),
            Op::Insert(6, "CCC"),
            Op::Insert(5, "DDD"),
        ],
    );
}

#[test]
fn more_deletes() {
    run_ops(
        "012345678",
        &[
            Op::Delete(8, 1),
            Op::Delete(0, 1),
            Op::Delete(5, 1),
            Op::Delete(5, 1),
            Op::Delete(0, 5),
        ],
    );
}

#[test]
fn random_test_1() {
    run_ops(
        "",
        &[
            Op::Insert(0, "ceLPHmFzvCtFeHkCBej "),
            Op::Insert(8, "gDCEfNYiBUNkSwtvB K "),
            Op::Insert(38, "cyNcHxjNPPoehBJldLS "),
            Op::Insert(59, "ejMx\nOTgWlbpeDExjOk "),
        ],
    );
}

#[test]
fn random_test_2() {
    run_ops(
        "",
        &[
            Op::Insert(0, "VgPG "),
            Op::Insert(2, "DdWF "),
            Op::Insert(0, "hUJc "),
            Op::Insert(8, "lQEq "),
            Op::Insert(10, "Gbtp "),
        ],
    );
}

#[test]
fn random_delete_1() {
    run_ops(
        "",
        &[
            Op::Insert(0, "vfb"),
            Op::Insert(0, "zRq"),
            Op::Delete(5, 1),
            Op::Insert(1, "UNw"),
            Op::Delete(4, 3),
            Op::Delete(1, 4),
            Op::Delete(0, 1),
        ],
    );
}

#[test]
fn random_delete_2() {
    run_ops(
        "",
        &[
            Op::Insert(0, "IDT"),
            Op::Insert(3, "wwA"),
            Op::Insert(3, "Gnr"),
            Op::Delete(6, 3),
            Op::Insert(4, "eHp"),
            Op::Insert(1, "UAi"),
            Op::Insert(2, "FrR"),
            Op::Delete(6, 7),
            Op::Delete(3, 5),
        ],
    );
}

#[test]
fn random_delete_3() {
    run_ops(
        "",
        &[
            Op::Insert(0, "PqM"),
            Op::Delete(1, 2),
            Op::Insert(1, "zLc"),
            Op::Insert(0, "MEX"),
            Op::Insert(0, "jZh"),
            Op::Insert(8, "GwQ"),
            Op::Delete(5, 6),
            Op::Insert(4, "ktw"),
            Op::Insert(5, "GVu"),
            Op::Insert(9, "jdm"),
            Op::Insert(15, "na\n"),
            Op::Delete(5, 8),
            Op::Delete(3, 4),
        ],
    );
}

/// `suite('inserts and deletes')`: `random insert/delete \\r bug 1`. Inserts that split and
/// join `\r\n` pairs across piece boundaries.
#[test]
fn random_insert_delete_cr_bug_1() {
    run_ops(
        "a",
        &[
            Op::Delete(0, 1),
            Op::Insert(0, "\r\r\n\n"),
            Op::Delete(3, 1),
            Op::Insert(2, "\n\n\ra"),
            Op::Delete(4, 3),
            Op::Insert(2, "\na\r\r"),
            Op::Insert(6, "\ra\n\n"),
            Op::Insert(0, "aa\n\n"),
            Op::Insert(5, "\n\na\r"),
        ],
    );
}

/// `suite('CRLF')`: `delete CR in CRLF 1` and `2`.
#[test]
fn delete_cr_in_crlf_1() {
    let mut tree = build(&[""], false);
    tree.insert(0, "a\r\nb", false);
    tree.delete(0, 2);
    assert_eq!(tree.line_count(), 2);
    tree.assert_valid();
}

#[test]
fn delete_cr_in_crlf_2() {
    let mut tree = build(&[""], false);
    tree.insert(0, "a\r\nb", false);
    tree.delete(2, 2);
    assert_eq!(tree.line_count(), 2);
    tree.assert_valid();
}

/// `suite('CRLF')`: `random bug 1`.
#[test]
fn crlf_random_bug_1() {
    run_ops(
        "",
        &[
            Op::Insert(0, "\n\n\r\r"),
            Op::Insert(1, "\r\n\r\n"),
            Op::Delete(5, 3),
            Op::Delete(2, 3),
        ],
    );
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

/// A chunk longer than 65535 units is split into several buffers, and a surrogate pair at the
/// cut stays whole in the first one.
#[test]
fn large_insert_splits_without_breaking_surrogates() {
    let head = "a".repeat(65534);
    let text = format!("{head}\u{1F600}b\nc");
    let mut tree = build(&[""], false);
    tree.insert(0, &text, false);
    assert_eq!(tree.line_count(), 2);
    assert_eq!(tree.line_content(1), format!("{head}\u{1F600}b"));
    assert_eq!(tree.line_content(2), "c");
    assert_eq!(tree.length(), 65534 + 2 + 1 + 1 + 1);
    assert_eq!(tree.position_at(65534), (1, 65535));
    assert_eq!(tree.offset_at(1, 65535), 65534);
    tree.assert_valid();
}

/// Deleting half of a `\r\n` breaks the normalization; the tree must stop claiming it.
#[test]
fn delete_cr_clears_normalized_flag() {
    let mut tree = build(&["one\r\ntwo"], true);
    assert!(tree.is_eol_normalized());
    tree.delete(3, 1);
    assert!(!tree.is_eol_normalized());
    assert_eq!(tree.lines_content(), vec!["one", "two"]);
}

/// Converts `text` so that every break is `\n`, for checking `value_in_range(.., "\n")`.
fn model_with_lf(text: &str) -> String {
    model_lines(text).join("\n")
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

/// `suite('random is unsupervised')`: `random insert delete`, with a fixed seed set.
#[test]
fn random_insert_delete_matches_string_model() {
    const ALPHABET: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ\r\n";
    for seed in 1..=200 {
        let mut rng = Rng::new(seed);
        let mut tree = build(&[""], false);
        let mut model = String::new();
        for _ in 0..60 {
            let len = model.len();
            if len == 0 || rng.below(3) != 0 {
                let offset = rng.below(len + 1);
                let text_len = rng.below(8);
                let text: String = (0..text_len)
                    .map(|_| ALPHABET[rng.below(ALPHABET.len())] as char)
                    .collect();
                tree.insert(offset, &text, false);
                model.insert_str(offset, &text);
            } else {
                let offset = rng.below(len);
                let count = 1 + rng.below(len - offset);
                tree.delete(offset, count);
                model.replace_range(offset..offset + count, "");
            }
            check_against_model(&tree, &model);

            // ranges between valid positions read back as the model text, with \n
            for _ in 0..3 {
                let a = rng.below(model.len() + 1);
                let b = rng.below(model.len() + 1);
                let (start, end) = (a.min(b), a.max(b));
                if !is_valid_offset(&model, start) || !is_valid_offset(&model, end) {
                    continue;
                }
                let start_pos = model_position(&model, start);
                let end_pos = model_position(&model, end);
                assert_eq!(
                    tree.value_in_range(start_pos, end_pos, "\n"),
                    model_with_lf(&model[start..end]),
                    "range {start}..{end} of {model:?}"
                );
            }
        }
    }
}

/// `suite('random is unsupervised')` with the first upstream fuzzer's shape: short texts,
/// inserted and deleted in equal measure, on a normalized buffer.
#[test]
fn random_edits_on_normalized_buffer() {
    for seed in 1..=50 {
        let mut rng = Rng::new(seed + 1000);
        let initial = "one\r\ntwo\r\nthree";
        let mut tree = build(&[initial], true);
        let mut model = initial.to_string();
        check_against_model(&tree, &model);
        for _ in 0..30 {
            let len = model.len();
            let offset = rng.below(len + 1);
            if rng.below(2) == 0 {
                let text = ["x", "\n", "y\r\n", "\r", "zz"][rng.below(5)];
                tree.insert(offset, text, false);
                model.insert_str(offset, text);
            } else {
                let count = 1 + rng.below((len - offset).max(1));
                let count = count.min(len - offset);
                tree.delete(offset, count);
                model.replace_range(offset..offset + count, "");
            }
            check_against_model(&tree, &model);
        }
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
