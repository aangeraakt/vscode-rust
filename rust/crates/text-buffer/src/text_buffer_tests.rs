//! Tests for `PieceTreeTextBuffer`: `applyEdits` with inverse edits, EOL changes and the value
//! queries. Ported from the behavior in `pieceTreeTextBuffer.ts`, with the upstream test cases
//! that cover it (`src/vs/editor/test/common/model/pieceTreeTextBuffer/pieceTreeTextBuffer.test.ts`,
//! `buffer api`). The randomized test is seeded.

use vscode_api::editor::{EndOfLinePreference, EndOfLineSequence, Range, SingleEditOperation};

use crate::{PieceTreeTextBuffer, TextBufferBuilder};

/// A buffer over `text`, built the way `createTextBuffer` does, with terminator LF.
fn build_buffer(text: &str) -> PieceTreeTextBuffer {
    let mut builder = TextBufferBuilder::new();
    builder.accept_chunk(text);
    builder
        .finish(false)
        .create_text_buffer(EndOfLineSequence::Lf)
}

/// Seeded xorshift generator for the randomized test.
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        Self(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1)
    }

    fn below(&mut self, bound: usize) -> usize {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        (x % bound as u64) as usize
    }
}

/// The 1-based position of `offset` in an ASCII `text` whose breaks are all `\n`.
fn position_of(text: &str, offset: usize) -> (u32, u32) {
    let before = &text[..offset];
    let line = before.matches('\n').count() as u32 + 1;
    let line_start = before.rfind('\n').map_or(0, |i| i + 1);
    (line, (offset - line_start) as u32 + 1)
}

fn replace(start: (u32, u32), end: (u32, u32), text: &str) -> SingleEditOperation {
    SingleEditOperation::replace(Range::new(start.0, start.1, end.0, end.1), text)
}

#[test]
fn line_access_and_offsets_match_the_text() {
    let buffer = build_buffer("one\ntwo\nthree");
    assert_eq!(buffer.line_count(), 3);
    assert_eq!(buffer.line_content(2), "two");
    assert_eq!(buffer.line_max_column(3), 6);
    assert_eq!(buffer.length(), 13);
    assert_eq!(buffer.offset_at(vscode_api::editor::Position::new(3, 2)), 9);
    assert_eq!(
        buffer.position_at(9),
        vscode_api::editor::Position::new(3, 2)
    );
    // positions past the end are clamped into the text
    assert_eq!(buffer.line_content(9), "three");
    assert_eq!(
        buffer.validate_position(vscode_api::editor::Position::new(1, 99)),
        vscode_api::editor::Position::new(1, 4)
    );
}

#[test]
fn line_char_code_reads_utf16_units() {
    let buffer = build_buffer("a\u{1F600}b");
    assert_eq!(buffer.line_char_code(1, 0), Some(u16::from(b'a')));
    // an astral character is a surrogate pair
    assert_eq!(buffer.line_char_code(1, 1), Some(0xD83D));
    assert_eq!(buffer.line_char_code(1, 2), Some(0xDE00));
    assert_eq!(buffer.line_char_code(1, 3), Some(u16::from(b'b')));
    assert_eq!(buffer.line_char_code(1, 4), None);
}

#[test]
fn apply_edits_reports_changes_from_the_bottom_up() {
    let mut buffer = build_buffer("one\ntwo\nthree");
    let result = buffer.apply_edits(
        &[replace((1, 1), (1, 4), "1"), replace((3, 1), (3, 6), "3")],
        false,
    );
    assert_eq!(buffer.value(EndOfLinePreference::TextDefined), "1\ntwo\n3");
    assert!(result.reverse_edits.is_none());
    let lines: Vec<u32> = result
        .changes
        .iter()
        .map(|c| c.range.start().line_number)
        .collect();
    assert_eq!(lines, vec![3, 1]);
    assert_eq!(result.changes[0].range_offset, 8);
    assert_eq!(result.changes[0].range_length, 5);
    assert_eq!(result.changes[1].range_offset, 0);
    assert_eq!(result.changes[1].range_length, 3);
}

#[test]
fn touching_edits_apply_in_order() {
    let mut buffer = build_buffer("abcdef");
    let result = buffer.apply_edits(
        &[replace((1, 3), (1, 5), "Y"), replace((1, 1), (1, 3), "X")],
        true,
    );
    assert_eq!(buffer.line_content(1), "XYef");
    let reverse = result.reverse_edits.expect("requested");
    buffer.apply_edits(&reverse, false);
    assert_eq!(buffer.line_content(1), "abcdef");
}

#[test]
#[should_panic(expected = "Overlapping ranges are not allowed!")]
fn overlapping_edits_panic() {
    let mut buffer = build_buffer("abcdef");
    buffer.apply_edits(
        &[replace((1, 1), (1, 4), "X"), replace((1, 3), (1, 5), "Y")],
        false,
    );
}

#[test]
fn empty_edit_list_changes_nothing() {
    let mut buffer = build_buffer("abc");
    let result = buffer.apply_edits(&[], true);
    assert!(result.changes.is_empty());
    assert_eq!(result.reverse_edits, Some(Vec::new()));
    assert_eq!(buffer.value(EndOfLinePreference::TextDefined), "abc");
}

/// Random disjoint, non-touching edits on random text. Applying them matches a `String` model,
/// and the reverse edits restore the original text.
#[test]
fn apply_edits_and_reverse_restore_random_texts() {
    const ALPHABET: &[u8] = b"abc\n";
    for seed in 1..=200 {
        let mut rng = Rng::new(seed);
        let original: String = (0..rng.below(60))
            .map(|_| ALPHABET[rng.below(ALPHABET.len())] as char)
            .collect();
        let mut buffer = build_buffer(&original);

        // pick edits whose ranges do not share a position, starting from valid offsets
        let valid: Vec<usize> = (0..=original.len()).collect();
        let mut edits: Vec<(usize, usize, String)> = Vec::new();
        let mut index = 0;
        while index < valid.len() {
            if rng.below(3) == 0 {
                index += 1;
                continue;
            }
            let end_index = (index + rng.below(3)).min(valid.len() - 1);
            let text: String = (0..rng.below(3))
                .map(|_| b"xy\n"[rng.below(3)] as char)
                .collect();
            edits.push((valid[index], valid[end_index], text));
            index = end_index + 1;
        }
        // input order is shuffled: the buffer must sort it
        for i in (1..edits.len()).rev() {
            edits.swap(i, rng.below(i + 1));
        }

        let ops: Vec<SingleEditOperation> = edits
            .iter()
            .map(|(a, b, text)| {
                replace(position_of(&original, *a), position_of(&original, *b), text)
            })
            .collect();

        let mut expected = original.clone();
        let mut by_start = edits.clone();
        by_start.sort_by_key(|(a, _, _)| std::cmp::Reverse(*a));
        for (a, b, text) in &by_start {
            expected.replace_range(*a..*b, text);
        }

        let result = buffer.apply_edits(&ops, true);
        assert_eq!(
            buffer.value(EndOfLinePreference::TextDefined),
            expected,
            "seed {seed}"
        );
        let reverse = result.reverse_edits.expect("requested");
        buffer.apply_edits(&reverse, false);
        assert_eq!(
            buffer.value(EndOfLinePreference::TextDefined),
            original,
            "seed {seed}: reverse edits"
        );
    }
}

#[test]
fn set_eol_rewrites_every_break() {
    let mut buffer = build_buffer("a\nb\r\nc");
    assert_eq!(buffer.eol(), EndOfLineSequence::Lf);
    buffer.set_eol(EndOfLineSequence::CrLf);
    assert_eq!(buffer.eol(), EndOfLineSequence::CrLf);
    assert_eq!(
        buffer.value(EndOfLinePreference::TextDefined),
        "a\r\nb\r\nc"
    );
    assert_eq!(buffer.line_count(), 3);
    assert_eq!(buffer.line_content(2), "b");
}

#[test]
fn value_and_length_honor_the_requested_eol() {
    let buffer = build_buffer("a\r\nb");
    assert_eq!(buffer.eol(), EndOfLineSequence::CrLf);
    let range = Range::new(1, 1, 2, 2);
    assert_eq!(
        buffer.value_in_range(range, EndOfLinePreference::Lf),
        "a\nb"
    );
    assert_eq!(
        buffer.value_in_range(range, EndOfLinePreference::CrLf),
        "a\r\nb"
    );
    assert_eq!(
        buffer.value_in_range(range, EndOfLinePreference::TextDefined),
        "a\r\nb"
    );
    assert_eq!(
        buffer.value_length_in_range(range, EndOfLinePreference::Lf),
        3
    );
    assert_eq!(
        buffer.value_length_in_range(range, EndOfLinePreference::TextDefined),
        4
    );
    assert_eq!(
        buffer.value_in_range(Range::new(1, 2, 1, 2), EndOfLinePreference::Lf),
        ""
    );
}

#[test]
fn equals_compares_bom_eol_and_text() {
    let mut with_bom = TextBufferBuilder::new();
    with_bom.accept_chunk("\u{feff}a\nb");
    let with_bom = with_bom
        .finish(false)
        .create_text_buffer(EndOfLineSequence::Lf);
    assert_eq!(with_bom.bom(), "\u{feff}");

    let plain = build_buffer("a\nb");
    assert!(!with_bom.equals(&plain), "the BOM differs");
    assert!(plain.equals(&build_buffer("a\nb")));
    assert!(
        !plain.equals(&build_buffer("a\r\nb")),
        "the terminator differs"
    );
}
