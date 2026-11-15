//! The text buffer: a piece tree with the public API of VS Code's `ITextBuffer`.
//!
//! Ported from `src/vs/editor/common/model/pieceTreeTextBuffer/pieceTreeTextBuffer.ts`. It takes
//! positions and ranges from `vscode-api` (1-based, UTF-16 columns, ADR 0003), applies edits
//! bottom to top with inverse edits, and changes the terminator on request.
//!
//! Edits are validated by clamping them into the text (line and column), so a caller that passes
//! an out-of-range position gets a position in the text rather than a panic. Surrogate pairs
//! are not repaired here: the text model rows will validate them before they reach the buffer.

use vscode_api::editor::{
    EndOfLinePreference, EndOfLineSequence, ModelContentChange, Position, Range,
    SingleEditOperation,
};

use crate::piece::normalize_breaks;
use crate::piece_tree::PieceTree;

/// The result of [`PieceTreeTextBuffer::apply_edits`] (`ApplyEditsResult`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApplyEditsResult {
    /// Edits that restore the text as it was before the call, when requested. They apply
    /// bottom to top, the same way the forward edits do.
    pub reverse_edits: Option<Vec<SingleEditOperation>>,
    /// The changes that were made, in the order they were applied (bottom to top).
    pub changes: Vec<ModelContentChange>,
}

/// An edit after validation, with the offsets it needs (`IValidatedEditOperation`).
#[derive(Debug, Clone)]
struct ValidatedEdit {
    sort_index: usize,
    range: Range,
    range_offset: usize,
    range_length: usize,
    /// The new text, with breaks already converted to the buffer's terminator.
    text: String,
    eol_count: usize,
    first_line_length: usize,
    last_line_length: usize,
}

/// A text buffer over a piece tree (`PieceTreeTextBuffer`).
#[derive(Debug)]
pub struct PieceTreeTextBuffer {
    tree: PieceTree,
    bom: String,
}

impl PieceTreeTextBuffer {
    /// Wraps a tree and the BOM it was read with.
    pub(crate) fn new(tree: PieceTree, bom: String) -> Self {
        Self { tree, bom }
    }

    /// The BOM the text was read with, or the empty string (`getBOM`).
    pub fn bom(&self) -> &str {
        &self.bom
    }

    /// The terminator of the buffer (`getEOL`).
    pub fn eol(&self) -> EndOfLineSequence {
        self.tree.eol()
    }

    /// Rewrites every line break to `eol` and makes it the buffer's terminator (`setEOL`).
    pub fn set_eol(&mut self, eol: EndOfLineSequence) {
        self.tree.set_eol(eol);
    }

    /// Length of the text in UTF-16 units, with the buffer's terminator (`getLength`).
    pub fn length(&self) -> usize {
        self.tree.length()
    }

    /// Number of lines; at least 1 (`getLineCount`).
    pub fn line_count(&self) -> u32 {
        self.tree.line_count() as u32
    }

    /// The text of a line without its terminator (`getLineContent`). Positions past the
    /// last line give the last line.
    pub fn line_content(&self, line_number: u32) -> String {
        self.tree
            .line_content(self.clamp_line(line_number) as usize)
    }

    /// Content of every line, without terminators (`getLinesContent`).
    pub fn lines_content(&self) -> Vec<String> {
        self.tree.lines_content()
    }

    /// Length of a line in UTF-16 units, without its terminator (`getLineLength`). Only exact
    /// when the text uses the buffer's terminator everywhere.
    pub fn line_length(&self, line_number: u32) -> u32 {
        self.tree.line_length(self.clamp_line(line_number) as usize) as u32
    }

    /// The column after the last character of a line (`getLineMaxColumn`).
    pub fn line_max_column(&self, line_number: u32) -> u32 {
        self.line_length(line_number) + 1
    }

    /// The UTF-16 unit at `index` of a line, or `None` past its end (`getLineCharCode`).
    pub fn line_char_code(&self, line_number: u32, index: usize) -> Option<u16> {
        self.line_content(line_number).encode_utf16().nth(index)
    }

    /// The UTF-16 offset of a position (`getOffsetAt`). The position is validated first.
    pub fn offset_at(&self, position: Position) -> u32 {
        let p = self.validate_position(position);
        self.tree
            .offset_at(p.line_number as usize, p.column as usize) as u32
    }

    /// The position of a UTF-16 offset, clamped to the text (`getPositionAt`).
    pub fn position_at(&self, offset: u32) -> Position {
        let offset = (offset as usize).min(self.tree.length());
        let (line, column) = self.tree.position_at(offset);
        Position::new(line as u32, column as u32)
    }

    /// Clamps a position into the text, line first, then column (`validatePosition`).
    pub fn validate_position(&self, position: Position) -> Position {
        let line = self.clamp_line(position.line_number);
        let max_column = self.line_max_column(line);
        Position::new(line, position.column.clamp(1, max_column))
    }

    /// Clamps both ends of a range into the text (`validateRange`).
    pub fn validate_range(&self, range: Range) -> Range {
        Range::from_positions(
            self.validate_position(range.start()),
            self.validate_position(range.end()),
        )
    }

    /// The whole text with the requested terminators (`getValue`).
    pub fn value(&self, eol: EndOfLinePreference) -> String {
        normalize_breaks(&self.tree.raw_content(), self.eol_str(eol))
    }

    /// The text in `range` with the requested terminators (`getValueInRange`). An empty range
    /// gives the empty string.
    pub fn value_in_range(&self, range: Range, eol: EndOfLinePreference) -> String {
        if range.is_empty() {
            return String::new();
        }
        let start = range.start();
        let end = range.end();
        self.tree.value_in_range(
            (start.line_number as usize, start.column as usize),
            (end.line_number as usize, end.column as usize),
            self.eol_str(eol),
        )
    }

    /// Length of the text in `range` in UTF-16 units, counting the requested terminators
    /// (`getValueLengthInRange`).
    pub fn value_length_in_range(&self, range: Range, eol: EndOfLinePreference) -> usize {
        if range.is_empty() {
            return 0;
        }
        let start = range.start();
        let end = range.end();
        if start.line_number == end.line_number {
            return (end.column - start.column) as usize;
        }
        let start_offset = self.offset_at(start) as isize;
        let end_offset = self.offset_at(end) as isize;
        // offsets count the buffer's terminator; correct for a different one
        let desired = self.eol_str(eol).len() as isize;
        let actual = self.eol().as_str().len() as isize;
        let lines = (end.line_number - start.line_number) as isize;
        (end_offset - start_offset + (desired - actual) * lines) as usize
    }

    /// Whether two buffers have the same BOM, terminator and text (`equals`).
    pub fn equals(&self, other: &PieceTreeTextBuffer) -> bool {
        self.bom == other.bom
            && self.eol() == other.eol()
            && self.tree.raw_content() == other.tree.raw_content()
    }

    /// Applies `edits` as one change and returns the changes made (`applyEdits`).
    ///
    /// Each range is validated first, then the edits are applied from the bottom of the text to
    /// the top, so earlier offsets stay valid. Edits may touch but not overlap.
    ///
    /// With `compute_undo`, the result carries the inverse edits, which restore the text when
    /// applied (in their own order).
    ///
    /// # Panics
    /// If two edit ranges overlap (`Overlapping ranges are not allowed!`).
    pub fn apply_edits(
        &mut self,
        edits: &[SingleEditOperation],
        compute_undo: bool,
    ) -> ApplyEditsResult {
        let mut ops: Vec<ValidatedEdit> = edits
            .iter()
            .enumerate()
            .map(|(sort_index, edit)| {
                let range = self.validate_range(edit.range);
                let text = normalize_breaks(&edit.text, self.eol().as_str());
                let (eol_count, first_line_length, last_line_length) = count_eol(&text);
                ValidatedEdit {
                    sort_index,
                    range,
                    range_offset: self.offset_at(range.start()) as usize,
                    range_length: self
                        .value_length_in_range(range, EndOfLinePreference::TextDefined),
                    text,
                    eol_count,
                    first_line_length,
                    last_line_length,
                }
            })
            .collect();

        // ascending by end, then by input order
        ops.sort_by(|a, b| {
            compare_ranges_using_ends(a.range, b.range).then(a.sort_index.cmp(&b.sort_index))
        });

        let mut has_touching_ranges = false;
        for pair in ops.windows(2) {
            let (previous_end, next_start) = (pair[0].range.end(), pair[1].range.start());
            if next_start <= previous_end {
                if next_start < previous_end {
                    panic!("Overlapping ranges are not allowed!");
                }
                has_touching_ranges = true;
            }
        }

        let reverse_edits = compute_undo.then(|| {
            let inverse_ranges = inverse_edit_ranges(&ops);
            let mut reverse: Vec<(usize, SingleEditOperation)> = ops
                .iter()
                .zip(inverse_ranges)
                .map(|(op, range)| {
                    let previous_text =
                        self.value_in_range(op.range, EndOfLinePreference::TextDefined);
                    (
                        op.sort_index,
                        SingleEditOperation::replace(range, previous_text),
                    )
                })
                .collect();
            // Touching edits keep their order; otherwise the reverse edits go back to input order.
            if !has_touching_ranges {
                reverse.sort_by_key(|(sort_index, _)| *sort_index);
            }
            reverse.into_iter().map(|(_, edit)| edit).collect()
        });

        let changes = self.do_apply_edits(ops);
        ApplyEditsResult {
            reverse_edits,
            changes,
        }
    }

    /// The text of `eol` as a string (`_getEndOfLine`).
    fn eol_str(&self, eol: EndOfLinePreference) -> &'static str {
        match eol {
            EndOfLinePreference::Lf => "\n",
            EndOfLinePreference::CrLf => "\r\n",
            EndOfLinePreference::TextDefined => self.eol().as_str(),
        }
    }

    /// Clamps a 1-based line number into `1..=line_count`.
    fn clamp_line(&self, line_number: u32) -> u32 {
        line_number.clamp(1, self.line_count())
    }

    /// Applies the edits from the bottom of the text up (`_doApplyEdits`).
    fn do_apply_edits(&mut self, mut ops: Vec<ValidatedEdit>) -> Vec<ModelContentChange> {
        ops.sort_by(|a, b| {
            compare_ranges_using_ends(b.range, a.range).then(b.sort_index.cmp(&a.sort_index))
        });

        let mut changes = Vec::with_capacity(ops.len());
        for op in ops {
            let (start, end) = (op.range.start(), op.range.end());
            if start == end && op.text.is_empty() {
                continue;
            }
            self.tree.delete(op.range_offset, op.range_length);
            if !op.text.is_empty() {
                self.tree.insert(op.range_offset, &op.text, true);
            }
            changes.push(ModelContentChange {
                range: op.range,
                range_offset: op.range_offset as u32,
                range_length: op.range_length as u32,
                text: op.text,
            });
        }
        changes
    }
}

/// The count of line breaks in `text`, and the lengths of its first and last lines, in UTF-16
/// units (`countEOL`). Without breaks, both lengths are the length of the text.
fn count_eol(text: &str) -> (usize, usize, usize) {
    let mut count = 0;
    let mut first_line_length = None;
    let mut line_start = 0;
    let mut units = 0;
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        let break_units = match ch {
            '\r' if chars.peek() == Some(&'\n') => {
                chars.next();
                2
            }
            '\r' | '\n' => 1,
            other => {
                units += other.len_utf16();
                continue;
            }
        };
        first_line_length.get_or_insert(units - line_start);
        count += 1;
        units += break_units;
        line_start = units;
    }
    let first = first_line_length.unwrap_or(units);
    (count, first, units - line_start)
}

/// Ranges of the text that the reverse edits will replace (`_getInverseEditRanges`). `ops` must
/// be sorted ascending.
fn inverse_edit_ranges(ops: &[ValidatedEdit]) -> Vec<Range> {
    let mut result = Vec::with_capacity(ops.len());
    let mut previous: Option<&ValidatedEdit> = None;
    let (mut previous_end_line, mut previous_end_column) = (0u32, 0u32);
    for op in ops {
        let (start_line, start_column) = match previous {
            Some(prev) if prev.range.end().line_number == op.range.start().line_number => (
                previous_end_line,
                previous_end_column + (op.range.start().column - prev.range.end().column),
            ),
            Some(prev) => (
                previous_end_line + (op.range.start().line_number - prev.range.end().line_number),
                op.range.start().column,
            ),
            None => (op.range.start().line_number, op.range.start().column),
        };

        let range = if op.text.is_empty() {
            Range::new(start_line, start_column, start_line, start_column)
        } else {
            let line_count = op.eol_count as u32 + 1;
            if line_count == 1 {
                Range::new(
                    start_line,
                    start_column,
                    start_line,
                    start_column + op.first_line_length as u32,
                )
            } else {
                Range::new(
                    start_line,
                    start_column,
                    start_line + line_count - 1,
                    op.last_line_length as u32 + 1,
                )
            }
        };
        previous_end_line = range.end().line_number;
        previous_end_column = range.end().column;
        result.push(range);
        previous = Some(op);
    }
    result
}

/// Orders ranges by end, then by start (`compareRangesUsingEnds`).
fn compare_ranges_using_ends(a: Range, b: Range) -> std::cmp::Ordering {
    (a.end(), a.start()).cmp(&(b.end(), b.start()))
}
