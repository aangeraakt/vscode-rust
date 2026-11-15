//! Editor coordinates, edits, and the text model contract.
//!
//! Ported from:
//! - `src/vs/editor/common/core/position.ts` (`Position`)
//! - `src/vs/editor/common/core/range.ts` (`Range`)
//! - `src/vs/editor/common/core/selection.ts` (`Selection`, `SelectionDirection`)
//! - `src/vs/editor/common/model.ts` (`ITextModel` subset, `EndOfLineSequence`,
//!   `EndOfLinePreference`, `IIdentifiedSingleEditOperation`)
//! - `src/vs/editor/common/textModelEvents.ts` (`IModelContentChange`,
//!   `IModelContentChangedEvent`)
//!
//! Coordinates follow ADR 0003: lines and columns are 1-based, and columns and offsets count
//! UTF-16 code units. Tests follow `src/vs/editor/test/common/core/range.test.ts`.
//!
//! The [`TextModel`] trait is implemented by `text-buffer` (role 03) and consumed by `editor`,
//! `language`, `workspace` and `extensions`. Undo/redo stacks and decorations are not part of it;
//! they live in `editor` and `text-buffer` behind their own APIs.

use crate::event::Event;
use crate::ids::LanguageId;
use crate::uri::Uri;
use std::cmp::Ordering;

/// A position in a text model (`Position`). Ordered by line, then column.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Position {
    /// 1-based line number.
    pub line_number: u32,
    /// 1-based column, in UTF-16 code units.
    pub column: u32,
}

impl Position {
    /// Creates a position.
    pub const fn new(line_number: u32, column: u32) -> Self {
        Self {
            line_number,
            column,
        }
    }

    /// Whether `self` comes strictly before `other` (`isBefore`).
    pub fn is_before(self, other: Position) -> bool {
        self < other
    }

    /// Whether `self` comes before or equals `other` (`isBeforeOrEqual`).
    pub fn is_before_or_equal(self, other: Position) -> bool {
        self <= other
    }
}

/// A range between two positions (`Range`). `start <= end` always holds: the fields are private
/// and the constructors swap the ends when needed, as the TypeScript constructor does.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Range {
    start: Position,
    end: Position,
}

impl Range {
    /// Creates a range from line and column numbers, swapping the ends if needed.
    pub fn new(start_line: u32, start_column: u32, end_line: u32, end_column: u32) -> Self {
        Self::from_positions(
            Position::new(start_line, start_column),
            Position::new(end_line, end_column),
        )
    }

    /// Creates a range from two positions in any order (`fromPositions`).
    pub fn from_positions(a: Position, b: Position) -> Self {
        if a <= b {
            Self { start: a, end: b }
        } else {
            Self { start: b, end: a }
        }
    }

    /// The start position (inclusive).
    pub fn start(self) -> Position {
        self.start
    }

    /// The end position (exclusive for text: the column after the last character).
    pub fn end(self) -> Position {
        self.end
    }

    /// An empty range at `position`.
    pub fn empty_at(position: Position) -> Self {
        Self {
            start: position,
            end: position,
        }
    }

    /// Whether the range is empty (`isEmpty`).
    pub fn is_empty(self) -> bool {
        self.start == self.end
    }

    /// Whether `position` is inside the range, ends included (`containsPosition`).
    pub fn contains_position(self, position: Position) -> bool {
        self.start <= position && position <= self.end
    }

    /// Whether `other` lies inside the range, ends included (`containsRange`).
    pub fn contains_range(self, other: Range) -> bool {
        self.start <= other.start && other.end <= self.end
    }

    /// Whether `other` lies inside the range and touches neither end (`strictContainsRange`).
    pub fn strict_contains_range(self, other: Range) -> bool {
        self.start < other.start && other.end < self.end
    }

    /// The smallest range covering both (`plusRange`).
    pub fn plus_range(self, other: Range) -> Range {
        Range {
            start: self.start.min(other.start),
            end: self.end.max(other.end),
        }
    }

    /// The overlap of two ranges, or `None` if they do not touch (`intersectRanges`). Ranges that
    /// only touch at one position intersect in an empty range.
    pub fn intersect(self, other: Range) -> Option<Range> {
        let start = self.start.max(other.start);
        let end = self.end.min(other.end);
        (start <= end).then_some(Range { start, end })
    }

    /// Whether the ranges overlap in more than a single touching position (`areIntersecting`).
    pub fn are_intersecting(self, other: Range) -> bool {
        !(self.end <= other.start) && !(other.end <= self.start)
    }

    /// An empty range at the start (`collapseToStart`).
    pub fn collapse_to_start(self) -> Range {
        Range::empty_at(self.start)
    }

    /// Orders by start, then by end (`compareRangesUsingStarts`).
    pub fn compare_using_starts(a: &Range, b: &Range) -> Ordering {
        a.start.cmp(&b.start).then(a.end.cmp(&b.end))
    }
}

/// Which way a selection was made (`SelectionDirection`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SelectionDirection {
    /// The active end is after the anchor.
    Ltr,
    /// The active end is before the anchor.
    Rtl,
}

/// A selection: an anchor where it started and the active position where the cursor is
/// (`Selection`: `selectionStart*` is the anchor, `position*` the active end).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Selection {
    /// Where the selection started.
    pub anchor: Position,
    /// Where the cursor is.
    pub active: Position,
}

impl Selection {
    /// Creates a selection.
    pub fn new(anchor: Position, active: Position) -> Self {
        Self { anchor, active }
    }

    /// The covered range, start before end.
    pub fn range(self) -> Range {
        Range::from_positions(self.anchor, self.active)
    }

    /// `Rtl` when the cursor is before the anchor, `Ltr` otherwise (`getDirection`).
    pub fn direction(self) -> SelectionDirection {
        if self.active < self.anchor {
            SelectionDirection::Rtl
        } else {
            SelectionDirection::Ltr
        }
    }

    /// Whether anchor and cursor coincide.
    pub fn is_empty(self) -> bool {
        self.anchor == self.active
    }
}

/// The line terminator stored in a model (`EndOfLineSequence`). Numeric values match VS Code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum EndOfLineSequence {
    /// `\n`
    #[default]
    Lf = 0,
    /// `\r\n`
    CrLf = 1,
}

impl EndOfLineSequence {
    /// The terminator text.
    pub fn as_str(self) -> &'static str {
        match self {
            EndOfLineSequence::Lf => "\n",
            EndOfLineSequence::CrLf => "\r\n",
        }
    }
}

/// Which terminator to use when reading text out of a model (`EndOfLinePreference`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum EndOfLinePreference {
    /// The model's own terminator.
    #[default]
    TextDefined = 0,
    /// `\n`
    Lf = 1,
    /// `\r\n`
    CrLf = 2,
}

/// One edit: replace `range` with `text` (`ISingleEditOperation`).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SingleEditOperation {
    /// The range to replace. An empty range inserts.
    pub range: Range,
    /// The replacement. Line terminators are normalized to the model's EOL.
    pub text: String,
    /// Push markers at the edit position along with the inserted text.
    pub force_move_markers: bool,
}

impl SingleEditOperation {
    /// Replaces `range` with `text`.
    pub fn replace(range: Range, text: impl Into<String>) -> Self {
        Self {
            range,
            text: text.into(),
            force_move_markers: false,
        }
    }

    /// Inserts `text` at `position`.
    pub fn insert(position: Position, text: impl Into<String>) -> Self {
        Self::replace(Range::empty_at(position), text)
    }
}

/// One change inside a content-changed event (`IModelContentChange`).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ModelContentChange {
    /// The replaced range, in coordinates from before the change.
    pub range: Range,
    /// UTF-16 offset of the range start, before the change.
    pub range_offset: u32,
    /// Length of the replaced text in UTF-16 code units.
    pub range_length: u32,
    /// The new text.
    pub text: String,
}

/// What a model fires after its content changes (`IModelContentChangedEvent`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelContentChangedEvent {
    /// The changes, ordered so that applying them in order reproduces the edit.
    pub changes: Vec<ModelContentChange>,
    /// The model's EOL after the change.
    pub eol: EndOfLineSequence,
    /// The version id after the change.
    pub version_id: u64,
    /// The change comes from an undo.
    pub is_undoing: bool,
    /// The change comes from a redo.
    pub is_redoing: bool,
    /// The whole content was replaced (`setValue`).
    pub is_flush: bool,
    /// The change only replaced line terminators (`setEOL`).
    pub is_eol_change: bool,
}

/// The text of one document with VS Code's line and offset semantics (an `ITextModel` subset).
///
/// Models live on the UI thread (ADR 0005), so the trait is not `Sync`.
pub trait TextModel {
    /// The resource this model belongs to.
    fn uri(&self) -> &Uri;
    /// The language of the model.
    fn language_id(&self) -> &LanguageId;
    /// Increases on every change, including undo and redo.
    fn version_id(&self) -> u64;
    /// Like `version_id`, but undo and redo restore the earlier value, so it tells whether the
    /// content matches a saved state.
    fn alternative_version_id(&self) -> u64;

    /// The whole text with the given terminators (`getValue`).
    fn value(&self, eol: EndOfLinePreference) -> String;
    /// The text in `range`, after validation (`getValueInRange`).
    fn value_in_range(&self, range: Range, eol: EndOfLinePreference) -> String;
    /// Length of the whole text in UTF-16 code units (`getValueLength` with the model EOL).
    fn value_length(&self) -> u32;

    /// Number of lines; at least 1 (`getLineCount`).
    fn line_count(&self) -> u32;
    /// The text of a line without its terminator (`getLineContent`). Panics if out of range.
    fn line_content(&self, line_number: u32) -> String;
    /// Length of a line in UTF-16 code units (`getLineLength`).
    fn line_length(&self, line_number: u32) -> u32;
    /// `line_length + 1` (`getLineMaxColumn`).
    fn line_max_column(&self, line_number: u32) -> u32 {
        self.line_length(line_number) + 1
    }

    /// The model's terminator.
    fn end_of_line_sequence(&self) -> EndOfLineSequence;
    /// Changes the terminator of every line; counts as an edit.
    fn set_end_of_line_sequence(&mut self, eol: EndOfLineSequence);

    /// UTF-16 offset of a position, after validation (`getOffsetAt`).
    fn offset_at(&self, position: Position) -> u32;
    /// The position of a UTF-16 offset, clamped to the text (`getPositionAt`).
    fn position_at(&self, offset: u32) -> Position;
    /// Clamps a position into the text and, if it falls between the two halves of a surrogate
    /// pair, moves it one column left, before the pair (`validatePosition`).
    fn validate_position(&self, position: Position) -> Position;
    /// Clamps both ends into the text and keeps surrogate pairs whole (`validateRange`): an end
    /// inside a pair moves outward (start left, end right), so the range grows to cover the
    /// whole character; an empty range inside a pair moves one column left instead. For `"😀"`,
    /// `(1,1)-(1,2)` validates to `(1,1)-(1,3)`.
    fn validate_range(&self, range: Range) -> Range;
    /// The range covering the whole text (`getFullModelRange`).
    fn full_model_range(&self) -> Range {
        let last = self.line_count();
        Range::new(1, 1, last, self.line_max_column(last))
    }

    /// Applies edits atomically and returns the inverse edits, which restore the previous text
    /// when applied (`applyEdits(ops, true)`). Ranges are validated first.
    ///
    /// # Panics
    /// If two edit ranges overlap, as TypeScript throws `Overlapping ranges are not allowed!`.
    fn apply_edits(&mut self, edits: &[SingleEditOperation]) -> Vec<SingleEditOperation>;
    /// Replaces the whole text (`setValue`); fires a flush event.
    fn set_value(&mut self, text: &str);

    /// Fires after every content change.
    fn on_did_change_content(&self) -> Event<ModelContentChangedEvent>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn range_constructor_swaps_ends() {
        let r = Range::new(2, 1, 1, 2);
        assert_eq!(r.start(), Position::new(1, 2));
        assert_eq!(r.end(), Position::new(2, 1));
        assert!(Range::new(1, 1, 1, 1).is_empty());
        assert!(!Range::new(1, 1, 1, 2).is_empty());
    }

    #[test]
    fn contains_position() {
        // range.test.ts: "containsPosition"
        let r = Range::new(2, 2, 5, 10);
        assert!(!r.contains_position(Position::new(1, 3)));
        assert!(!r.contains_position(Position::new(2, 1)));
        assert!(r.contains_position(Position::new(2, 2)));
        assert!(r.contains_position(Position::new(2, 3)));
        assert!(r.contains_position(Position::new(3, 1)));
        assert!(r.contains_position(Position::new(5, 9)));
        assert!(r.contains_position(Position::new(5, 10)));
        assert!(!r.contains_position(Position::new(5, 11)));
        assert!(!r.contains_position(Position::new(6, 1)));
    }

    #[test]
    fn contains_range() {
        // range.test.ts: "containsRange"
        let r = Range::new(2, 2, 5, 10);
        assert!(!r.contains_range(Range::new(1, 3, 2, 2)));
        assert!(!r.contains_range(Range::new(2, 1, 2, 2)));
        assert!(!r.contains_range(Range::new(2, 2, 5, 11)));
        assert!(!r.contains_range(Range::new(2, 2, 6, 1)));
        assert!(!r.contains_range(Range::new(5, 9, 6, 1)));
        assert!(!r.contains_range(Range::new(5, 10, 6, 1)));
        assert!(r.contains_range(Range::new(2, 2, 5, 10)));
        assert!(r.contains_range(Range::new(2, 3, 5, 9)));
        assert!(r.contains_range(Range::new(3, 100, 4, 100)));
        assert!(!r.strict_contains_range(Range::new(2, 2, 5, 10)));
        assert!(r.strict_contains_range(Range::new(2, 3, 5, 9)));
    }

    #[test]
    fn intersect_ranges() {
        // No upstream test; cases derived from `Range.intersectRanges`.
        let i = |a: Range, b: Range| a.intersect(b);
        assert_eq!(i(Range::new(1, 1, 1, 5), Range::new(1, 7, 1, 9)), None);
        assert_eq!(
            i(Range::new(1, 1, 1, 5), Range::new(1, 5, 1, 9)),
            Some(Range::new(1, 5, 1, 5))
        );
        assert_eq!(
            i(Range::new(1, 3, 2, 3), Range::new(2, 1, 2, 8)),
            Some(Range::new(2, 1, 2, 3))
        );
        assert_eq!(i(Range::new(1, 1, 2, 1), Range::new(3, 1, 4, 1)), None);
    }

    #[test]
    fn are_intersecting() {
        // range.test.ts: "areIntersecting"
        let t = |a: Range, b: Range| a.are_intersecting(b);
        assert!(!t(Range::new(2, 2, 3, 2), Range::new(4, 2, 5, 2)));
        assert!(!t(Range::new(4, 2, 5, 2), Range::new(2, 2, 3, 2)));
        assert!(!t(Range::new(4, 2, 5, 2), Range::new(5, 2, 6, 2)));
        assert!(!t(Range::new(5, 2, 6, 2), Range::new(4, 2, 5, 2)));
        assert!(t(Range::new(2, 2, 2, 7), Range::new(2, 4, 2, 6)));
        assert!(t(Range::new(2, 2, 2, 7), Range::new(2, 4, 2, 9)));
        assert!(t(Range::new(2, 4, 2, 9), Range::new(2, 2, 2, 7)));
    }

    #[test]
    fn plus_range_and_ordering() {
        assert_eq!(
            Range::new(1, 5, 2, 3).plus_range(Range::new(1, 2, 1, 9)),
            Range::new(1, 2, 2, 3)
        );
        let mut ranges = [
            Range::new(2, 1, 2, 2),
            Range::new(1, 1, 3, 1),
            Range::new(1, 1, 1, 4),
        ];
        ranges.sort_by(Range::compare_using_starts);
        assert_eq!(ranges[0], Range::new(1, 1, 1, 4));
        assert_eq!(ranges[2], Range::new(2, 1, 2, 2));
    }

    #[test]
    fn selection_direction() {
        let s = Selection::new(Position::new(3, 4), Position::new(1, 2));
        assert_eq!(s.direction(), SelectionDirection::Rtl);
        assert_eq!(s.range(), Range::new(1, 2, 3, 4));
        assert_eq!(
            Selection::new(Position::new(1, 1), Position::new(1, 1)).direction(),
            SelectionDirection::Ltr
        );
    }

    #[test]
    fn eol_values_match_vscode() {
        assert_eq!(EndOfLineSequence::CrLf as i32, 1);
        assert_eq!(EndOfLinePreference::CrLf as i32, 2);
        assert_eq!(EndOfLineSequence::CrLf.as_str(), "\r\n");
    }

    /// The trait must stay object-safe: consumers hold `Box<dyn TextModel>`.
    #[allow(dead_code)]
    fn object_safe(_: &dyn TextModel) {}
}
