//! Vertical layout of lines and view zones (whitespace).
//!
//! Ported from `src/vs/editor/common/viewLayout/linesLayout.ts` (`LinesLayout`,
//! `EditorWhitespace`).
//!
//! Lines take their heights from [`LineHeightsManager`]. Whitespace is a view zone: a block of
//! pixels inserted after a line that pushes the lines below it down. Whitespace lives in one
//! list sorted by `(after_line_number, ordinal)`. Each entry stores the total height of the
//! whitespace up to and including it, so a query is a binary search.
//!
//! Costs for a query, with `n` lines, `c` custom line heights and `w` whitespaces: a vertical
//! offset takes O(log c + log w); a line lookup binary-searches over lines and so takes
//! O(log n * (log c + log w)). No query walks the lines, so a 1M-line file costs the same per query
//! as a short one. Changing lines (`on_lines_inserted`, `on_lines_deleted`) and committing
//! whitespace changes are O(w + c), since they shift or rebuild the lists.
//!
//! Divergences from the TypeScript:
//! - Prefix sums are recomputed when the whitespace list changes, not on first read, so queries
//!   take `&self`.
//! - `getWhitespaceMinWidth` is computed on each call instead of cached.
//! - Integer arguments that the TypeScript truncates with `| 0` are truncated the same way.

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::line_heights::{CustomLineHeightData, LineHeightChangeAccessor, LineHeightsManager};

static INSTANCE_COUNT: AtomicU64 = AtomicU64::new(0);

/// A view zone: vertical space after a line (`EditorWhitespace`).
#[derive(Debug, Clone, PartialEq)]
pub struct EditorWhitespace {
    /// Identifier, unique within one layout.
    pub id: String,
    /// The line this whitespace follows. 0 means before the first line.
    pub after_line_number: i32,
    /// Orders whitespace that follows the same line.
    pub ordinal: i32,
    /// Height in pixels.
    pub height: i32,
    /// Minimum width of the zone in pixels.
    pub min_width: i32,
    /// Total height of this whitespace and all before it in the list.
    prefix_sum: f64,
}

impl EditorWhitespace {
    /// Creates a whitespace entry. The prefix sum is filled in when it is added to a layout.
    pub fn new(
        id: impl Into<String>,
        after_line_number: i32,
        ordinal: i32,
        height: i32,
        min_width: i32,
    ) -> Self {
        Self {
            id: id.into(),
            after_line_number,
            ordinal,
            height,
            min_width,
            prefix_sum: 0.0,
        }
    }
}

/// A view zone as reported to a viewport (`IViewWhitespaceViewportData`).
#[derive(Debug, Clone, PartialEq)]
pub struct WhitespaceViewportData {
    /// Identifier of the whitespace.
    pub id: String,
    /// The line this whitespace follows.
    pub after_line_number: i32,
    /// Top of the whitespace, in pixels.
    pub vertical_offset: f64,
    /// Height of the whitespace, in pixels.
    pub height: i32,
}

/// The lines in a vertical range (`IPartialViewLinesViewportData`).
#[derive(Debug, Clone, PartialEq)]
pub struct PartialViewLinesViewportData {
    /// Subtract this from `scrollTop` to keep offsets under about 1M pixels.
    pub big_numbers_delta: f64,
    /// The first (partially) visible line.
    pub start_line_number: i32,
    /// The last (partially) visible line.
    pub end_line_number: i32,
    /// Top of each line in `start..=end`, relative to `big_numbers_delta`.
    pub relative_vertical_offset: Vec<f64>,
    /// The line at the middle of the range.
    pub centered_line_number: i32,
    /// The first line that is entirely inside the range.
    pub completely_visible_start_line_number: i32,
    /// The last line that is entirely inside the range.
    pub completely_visible_end_line_number: i32,
    /// The default line height.
    pub line_height: f64,
}

#[derive(Debug)]
struct PendingWhitespaceChange {
    id: String,
    new_after_line_number: i32,
    new_height: i32,
}

/// Whitespace edits waiting for the end of a batch.
#[derive(Debug, Default)]
struct PendingChanges {
    inserts: Vec<EditorWhitespace>,
    changes: Vec<PendingWhitespaceChange>,
    removes: Vec<String>,
}

/// Adds, changes and removes whitespace inside one batch (`IWhitespaceChangeAccessor`).
#[derive(Debug)]
pub struct WhitespaceChangeAccessor<'a> {
    pending: &'a mut PendingChanges,
    instance_id: &'a str,
    last_whitespace_id: &'a mut u64,
    had_change: bool,
}

impl WhitespaceChangeAccessor<'_> {
    /// Queues a new whitespace and returns its id.
    pub fn insert_whitespace(
        &mut self,
        after_line_number: i32,
        ordinal: i32,
        height_in_px: i32,
        min_width: i32,
    ) -> String {
        self.had_change = true;
        *self.last_whitespace_id += 1;
        let id = format!("{}{}", self.instance_id, self.last_whitespace_id);
        self.pending.inserts.push(EditorWhitespace::new(
            id.clone(),
            after_line_number,
            ordinal,
            height_in_px,
            min_width,
        ));
        id
    }

    /// Queues a change to the line and height of an existing whitespace.
    pub fn change_one_whitespace(&mut self, id: &str, new_after_line_number: i32, new_height: i32) {
        self.had_change = true;
        self.pending.changes.push(PendingWhitespaceChange {
            id: id.to_owned(),
            new_after_line_number,
            new_height,
        });
    }

    /// Queues the removal of a whitespace.
    pub fn remove_whitespace(&mut self, id: &str) {
        self.had_change = true;
        self.pending.removes.push(id.to_owned());
    }
}

/// Vertical layout of lines and whitespace.
///
/// Ported from `LinesLayout`.
#[derive(Debug)]
pub struct LinesLayout {
    instance_id: String,
    pending: PendingChanges,
    last_whitespace_id: u64,
    arr: Vec<EditorWhitespace>,
    line_count: i32,
    padding_top: f64,
    padding_bottom: f64,
    line_heights: LineHeightsManager,
}

impl LinesLayout {
    /// Creates a layout for `line_count` lines of the default height.
    pub fn new(
        line_count: i32,
        default_line_height: f64,
        padding_top: f64,
        padding_bottom: f64,
        custom_line_heights: &[CustomLineHeightData],
    ) -> Self {
        let instance = INSTANCE_COUNT.fetch_add(1, Ordering::Relaxed) + 1;
        Self {
            instance_id: format!("{instance:x}-"),
            pending: PendingChanges::default(),
            last_whitespace_id: 0,
            arr: Vec::new(),
            line_count,
            padding_top,
            padding_bottom,
            line_heights: LineHeightsManager::new(default_line_height, custom_line_heights),
        }
    }

    /// Returns the index at which `(after_line_number, ordinal)` should be inserted to keep the
    /// list sorted. An entry equal to an existing one goes after it.
    pub fn find_insertion_index(
        arr: &[EditorWhitespace],
        after_line_number: i32,
        ordinal: i32,
    ) -> usize {
        arr.partition_point(|w| {
            w.after_line_number < after_line_number
                || (w.after_line_number == after_line_number && w.ordinal <= ordinal)
        })
    }

    /// Sets the height of a line without a custom height.
    pub fn set_default_line_height(&mut self, line_height: f64) {
        self.line_heights.set_default_line_height(line_height);
    }

    /// Sets the padding above and below the lines.
    pub fn set_padding(&mut self, padding_top: f64, padding_bottom: f64) {
        self.padding_top = padding_top;
        self.padding_bottom = padding_bottom;
    }

    /// Replaces the line count and custom line heights after the model was flushed.
    pub fn on_flushed(&mut self, line_count: i32, custom_line_heights: &[CustomLineHeightData]) {
        self.line_count = line_count;
        let default = self.line_heights.default_line_height();
        self.line_heights = LineHeightsManager::new(default, custom_line_heights);
    }

    /// Runs a batch of custom line height changes. Returns whether anything changed.
    pub fn change_line_heights(
        &mut self,
        callback: impl FnOnce(&mut LineHeightChangeAccessor<'_>),
    ) -> bool {
        let mut accessor = LineHeightChangeAccessor::new(&mut self.line_heights);
        callback(&mut accessor);
        accessor.had_change()
    }

    /// Runs a batch of whitespace changes and commits them at the end. Returns whether anything
    /// changed.
    pub fn change_whitespace(
        &mut self,
        callback: impl FnOnce(&mut WhitespaceChangeAccessor<'_>),
    ) -> bool {
        let had_change = {
            let mut accessor = WhitespaceChangeAccessor {
                pending: &mut self.pending,
                instance_id: &self.instance_id,
                last_whitespace_id: &mut self.last_whitespace_id,
                had_change: false,
            };
            callback(&mut accessor);
            accessor.had_change
        };
        let pending = std::mem::take(&mut self.pending);
        self.commit_pending_changes(pending);
        had_change
    }

    fn commit_pending_changes(&mut self, pending: PendingChanges) {
        let PendingChanges {
            inserts,
            changes,
            removes,
        } = pending;

        if inserts.len() + changes.len() + removes.len() <= 1 {
            // One change: apply it directly, as the TypeScript does.
            for insert in inserts {
                self.insert_whitespace(insert);
            }
            for change in changes {
                self.change_one_whitespace(
                    &change.id,
                    change.new_after_line_number,
                    change.new_height,
                );
            }
            for id in removes {
                if let Some(index) = self.find_whitespace_index(&id) {
                    self.arr.remove(index);
                }
            }
        } else {
            // Many changes: rebuild the list in one pass and sort it.
            let to_remove: HashSet<&str> = removes.iter().map(String::as_str).collect();
            // A later change to the same id wins, as with a Map in the TypeScript.
            let mut to_change: HashMap<&str, &PendingWhitespaceChange> = HashMap::new();
            for change in &changes {
                to_change.insert(change.id.as_str(), change);
            }
            let apply = |list: Vec<EditorWhitespace>| -> Vec<EditorWhitespace> {
                list.into_iter()
                    .filter(|w| !to_remove.contains(w.id.as_str()))
                    .map(|mut w| {
                        if let Some(change) = to_change.get(w.id.as_str()) {
                            w.after_line_number = change.new_after_line_number;
                            w.height = change.new_height;
                        }
                        w
                    })
                    .collect()
            };
            let mut result = apply(std::mem::take(&mut self.arr));
            result.extend(apply(inserts));
            // Stable sort, so equal keys keep their order.
            result.sort_by(|a, b| {
                a.after_line_number
                    .cmp(&b.after_line_number)
                    .then(a.ordinal.cmp(&b.ordinal))
            });
            self.arr = result;
        }
        self.recompute_prefix_sums();
    }

    fn recompute_prefix_sums(&mut self) {
        let mut running = 0.0;
        for whitespace in &mut self.arr {
            running += f64::from(whitespace.height);
            whitespace.prefix_sum = running;
        }
    }

    fn insert_whitespace(&mut self, whitespace: EditorWhitespace) {
        let index =
            Self::find_insertion_index(&self.arr, whitespace.after_line_number, whitespace.ordinal);
        self.arr.insert(index, whitespace);
    }

    fn find_whitespace_index(&self, id: &str) -> Option<usize> {
        self.arr.iter().position(|w| w.id == id)
    }

    fn change_one_whitespace(&mut self, id: &str, new_after_line_number: i32, new_height: i32) {
        let Some(index) = self.find_whitespace_index(id) else {
            return;
        };
        self.arr[index].height = new_height;
        if self.arr[index].after_line_number != new_after_line_number {
            // Moving the whitespace can change its position in the sorted list, so remove it and
            // insert it again.
            let mut whitespace = self.arr.remove(index);
            whitespace.after_line_number = new_after_line_number;
            self.insert_whitespace(whitespace);
        }
    }

    /// Lines `from..=to` were deleted. Whitespace after a deleted line moves to just before the
    /// first deleted line.
    pub fn on_lines_deleted(&mut self, from_line_number: i32, to_line_number: i32) {
        let count = to_line_number - from_line_number + 1;
        self.line_count -= count;
        for whitespace in &mut self.arr {
            let after = whitespace.after_line_number;
            if from_line_number <= after && after <= to_line_number {
                whitespace.after_line_number = from_line_number - 1;
            } else if after > to_line_number {
                whitespace.after_line_number -= count;
            }
        }
        self.line_heights
            .on_lines_deleted(from_line_number, to_line_number);
    }

    /// Lines `from..=to` were inserted. Whitespace at or after `from` moves down with them.
    pub fn on_lines_inserted(&mut self, from_line_number: i32, to_line_number: i32) {
        let count = to_line_number - from_line_number + 1;
        self.line_count += count;
        for whitespace in &mut self.arr {
            if from_line_number <= whitespace.after_line_number {
                whitespace.after_line_number += count;
            }
        }
        self.line_heights
            .on_lines_inserted(from_line_number, to_line_number);
    }

    /// Total height of all whitespace.
    pub fn whitespaces_total_height(&self) -> f64 {
        self.arr.last().map_or(0.0, |w| w.prefix_sum)
    }

    /// Total height of the whitespace at `0..=index`.
    pub fn whitespaces_accumulated_height(&self, index: usize) -> f64 {
        self.arr[index].prefix_sum
    }

    /// Total height of lines, whitespace and padding.
    pub fn lines_total_height(&self) -> f64 {
        self.line_heights
            .accumulated_line_heights_including_line_number(self.line_count)
            + self.whitespaces_total_height()
            + self.padding_top
            + self.padding_bottom
    }

    /// Total height of the whitespace that comes before `line_number`.
    pub fn whitespace_accumulated_height_before_line_number(&self, line_number: i32) -> f64 {
        let index = self.last_whitespace_before_line_number(line_number);
        if index < 0 {
            0.0
        } else {
            self.arr[index as usize].prefix_sum
        }
    }

    /// Index of the last whitespace whose line is before `line_number`, or -1.
    fn last_whitespace_before_line_number(&self, line_number: i32) -> isize {
        self.arr
            .partition_point(|w| w.after_line_number < line_number) as isize
            - 1
    }

    /// Index of the first whitespace whose line is at or after `line_number`, or -1.
    pub fn first_whitespace_index_after_line_number(&self, line_number: i32) -> isize {
        let index = (self.last_whitespace_before_line_number(line_number) + 1) as usize;
        if index < self.arr.len() {
            index as isize
        } else {
            -1
        }
    }

    /// Vertical offset of the top of `line_number`. With `include_view_zones`, whitespace after
    /// the line before it is counted.
    pub fn vertical_offset_for_line_number(
        &self,
        line_number: i32,
        include_view_zones: bool,
    ) -> f64 {
        let previous_lines = if line_number > 1 {
            self.line_heights
                .accumulated_line_heights_including_line_number(line_number - 1)
        } else {
            0.0
        };
        let whitespace_line = line_number - i32::from(include_view_zones);
        previous_lines
            + self.whitespace_accumulated_height_before_line_number(whitespace_line)
            + self.padding_top
    }

    /// Vertical offset of the bottom of `line_number`. With `include_view_zones`, whitespace after
    /// the line is counted.
    pub fn vertical_offset_after_line_number(
        &self,
        line_number: i32,
        include_view_zones: bool,
    ) -> f64 {
        let previous_lines = self
            .line_heights
            .accumulated_line_heights_including_line_number(line_number);
        let whitespace_line = line_number + i32::from(include_view_zones);
        previous_lines
            + self.whitespace_accumulated_height_before_line_number(whitespace_line)
            + self.padding_top
    }

    /// Height of `line_number` in pixels.
    pub fn line_height_for_line_number(&self, line_number: i32) -> f64 {
        self.line_heights.height_for_line_number(line_number)
    }

    /// Whether there is any whitespace.
    pub fn has_whitespace(&self) -> bool {
        !self.arr.is_empty()
    }

    /// The largest minimum width of any whitespace, or 0.
    pub fn whitespace_min_width(&self) -> i32 {
        self.arr.iter().map(|w| w.min_width).fold(0, i32::max)
    }

    /// Whether `vertical_offset` is below the last line.
    pub fn is_after_lines(&self, vertical_offset: f64) -> bool {
        vertical_offset > self.lines_total_height()
    }

    /// Whether `vertical_offset` is in the top padding.
    pub fn is_in_top_padding(&self, vertical_offset: f64) -> bool {
        self.padding_top != 0.0 && vertical_offset < self.padding_top
    }

    /// Whether `vertical_offset` is in the bottom padding.
    pub fn is_in_bottom_padding(&self, vertical_offset: f64) -> bool {
        self.padding_bottom != 0.0
            && vertical_offset >= self.lines_total_height() - self.padding_bottom
    }

    /// The first line whose top is at or after `vertical_offset`. The line that contains
    /// `vertical_offset` is also returned.
    pub fn line_number_at_or_after_vertical_offset(&self, vertical_offset: f64) -> i32 {
        let vertical_offset = vertical_offset.trunc();
        if vertical_offset < 0.0 {
            return 1;
        }
        let line_count = self.line_count;
        let mut min_line = 1;
        let mut max_line = line_count;
        while min_line < max_line {
            let mid = (min_line + max_line) / 2;
            let line_height = self.line_height_for_line_number(mid);
            let mid_offset = self.vertical_offset_for_line_number(mid, false).trunc();
            if vertical_offset >= mid_offset + line_height {
                min_line = mid + 1;
            } else if vertical_offset >= mid_offset {
                return mid;
            } else {
                max_line = mid;
            }
        }
        min_line.min(line_count)
    }

    /// The lines that intersect `start..end` in vertical offsets.
    pub fn lines_viewport_data(
        &self,
        vertical_offset1: f64,
        vertical_offset2: f64,
    ) -> PartialViewLinesViewportData {
        const STEP_SIZE: f64 = 500_000.0;

        let vertical_offset1 = vertical_offset1.trunc();
        let vertical_offset2 = vertical_offset2.trunc();
        let default_line_height = self.line_heights.default_line_height();

        // Find the first line. The line number may start before or after `vertical_offset1`.
        let start_line_number = self.line_number_at_or_after_vertical_offset(vertical_offset1);
        let start_offset = self
            .vertical_offset_for_line_number(start_line_number, false)
            .trunc();
        let mut end_line_number = self.line_count;

        // Track the whitespace the loop has reached.
        let whitespace_count = self.arr.len() as isize;
        let mut whitespace_index = self.first_whitespace_index_after_line_number(start_line_number);
        let (mut whitespace_after, mut whitespace_height) = if whitespace_index == -1 {
            whitespace_index = whitespace_count;
            (end_line_number + 1, 0.0)
        } else {
            let whitespace = &self.arr[whitespace_index as usize];
            (whitespace.after_line_number, f64::from(whitespace.height))
        };

        let mut current_offset = start_offset;
        let mut current_relative_offset = current_offset;
        // Keep offsets below about 1M pixels: subtract a whole number of steps and of lines.
        let mut big_numbers_delta = 0.0;
        if start_offset >= STEP_SIZE {
            big_numbers_delta = (start_offset / STEP_SIZE).floor() * STEP_SIZE;
            big_numbers_delta =
                (big_numbers_delta / default_line_height).floor() * default_line_height;
            current_relative_offset -= big_numbers_delta;
        }

        let mut relative_vertical_offset = Vec::new();
        let vertical_center = vertical_offset1 + (vertical_offset2 - vertical_offset1) / 2.0;
        let mut centered_line_number = -1;

        // Walk the lines until the viewport is covered.
        let mut line_number = start_line_number;
        while line_number <= end_line_number {
            let line_height = self.line_height_for_line_number(line_number);
            if centered_line_number == -1 {
                let top = current_offset;
                let bottom = current_offset + line_height;
                if (top <= vertical_center && vertical_center < bottom) || top > vertical_center {
                    centered_line_number = line_number;
                }
            }

            current_offset += line_height;
            relative_vertical_offset.push(current_relative_offset);

            // The next line starts after this one, pushed down by any whitespace that follows it.
            current_relative_offset += line_height;
            while whitespace_after == line_number {
                current_relative_offset += whitespace_height;
                current_offset += whitespace_height;
                whitespace_index += 1;
                if whitespace_index >= whitespace_count {
                    whitespace_after = end_line_number + 1;
                } else {
                    let whitespace = &self.arr[whitespace_index as usize];
                    whitespace_after = whitespace.after_line_number;
                    whitespace_height = f64::from(whitespace.height);
                }
            }

            if current_offset >= vertical_offset2 {
                end_line_number = line_number;
                break;
            }
            line_number += 1;
        }

        if centered_line_number == -1 {
            centered_line_number = end_line_number;
        }

        let end_offset = self
            .vertical_offset_for_line_number(end_line_number, false)
            .trunc();
        let mut completely_visible_start = start_line_number;
        let mut completely_visible_end = end_line_number;
        if completely_visible_start < completely_visible_end && start_offset < vertical_offset1 {
            completely_visible_start += 1;
        }
        if completely_visible_start < completely_visible_end
            && end_offset + self.line_height_for_line_number(end_line_number) > vertical_offset2
        {
            completely_visible_end -= 1;
        }

        PartialViewLinesViewportData {
            big_numbers_delta,
            start_line_number,
            end_line_number,
            relative_vertical_offset,
            centered_line_number,
            completely_visible_start_line_number: completely_visible_start,
            completely_visible_end_line_number: completely_visible_end,
            line_height: default_line_height,
        }
    }

    /// Vertical offset of the top of the whitespace at `index`.
    pub fn vertical_offset_for_whitespace_index(&self, index: usize) -> f64 {
        let whitespace = &self.arr[index];
        let previous_lines = if whitespace.after_line_number >= 1 {
            self.line_heights
                .accumulated_line_heights_including_line_number(whitespace.after_line_number)
        } else {
            0.0
        };
        let previous_whitespace = if index > 0 {
            self.arr[index - 1].prefix_sum
        } else {
            0.0
        };
        previous_lines + previous_whitespace + self.padding_top
    }

    /// Index of the first whitespace whose bottom is below `vertical_offset`, or -1 if none.
    pub fn whitespace_index_at_or_after_vertical_offset(&self, vertical_offset: f64) -> isize {
        let vertical_offset = vertical_offset.trunc();
        let count = self.arr.len();
        if count == 0 {
            return -1;
        }
        let mut min_index = 0;
        let mut max_index = count - 1;

        let max_offset = self.vertical_offset_for_whitespace_index(max_index);
        let max_height = f64::from(self.arr[max_index].height);
        if vertical_offset >= max_offset + max_height {
            return -1;
        }

        while min_index < max_index {
            let mid = (min_index + max_index) / 2;
            let mid_offset = self.vertical_offset_for_whitespace_index(mid);
            let mid_height = f64::from(self.arr[mid].height);
            if vertical_offset >= mid_offset + mid_height {
                min_index = mid + 1;
            } else if vertical_offset >= mid_offset {
                return mid as isize;
            } else {
                max_index = mid;
            }
        }
        min_index as isize
    }

    /// The whitespace drawn at `vertical_offset`, if any.
    pub fn whitespace_at_vertical_offset(
        &self,
        vertical_offset: f64,
    ) -> Option<WhitespaceViewportData> {
        let vertical_offset = vertical_offset.trunc();
        let candidate = self.whitespace_index_at_or_after_vertical_offset(vertical_offset);
        if candidate < 0 || candidate as usize >= self.arr.len() {
            return None;
        }
        let index = candidate as usize;
        let top = self.vertical_offset_for_whitespace_index(index);
        if top > vertical_offset {
            return None;
        }
        let whitespace = &self.arr[index];
        Some(WhitespaceViewportData {
            id: whitespace.id.clone(),
            after_line_number: whitespace.after_line_number,
            vertical_offset: top,
            height: whitespace.height,
        })
    }

    /// The whitespace that intersects `vertical_offset1..vertical_offset2`.
    pub fn whitespace_viewport_data(
        &self,
        vertical_offset1: f64,
        vertical_offset2: f64,
    ) -> Vec<WhitespaceViewportData> {
        let vertical_offset1 = vertical_offset1.trunc();
        let vertical_offset2 = vertical_offset2.trunc();
        let start = self.whitespace_index_at_or_after_vertical_offset(vertical_offset1);
        if start < 0 {
            return Vec::new();
        }
        let mut result = Vec::new();
        for index in start as usize..self.arr.len() {
            let top = self.vertical_offset_for_whitespace_index(index);
            if top >= vertical_offset2 {
                break;
            }
            let whitespace = &self.arr[index];
            result.push(WhitespaceViewportData {
                id: whitespace.id.clone(),
                after_line_number: whitespace.after_line_number,
                vertical_offset: top,
                height: whitespace.height,
            });
        }
        result
    }

    /// All whitespace, in order.
    pub fn whitespaces(&self) -> Vec<EditorWhitespace> {
        self.arr.clone()
    }

    /// Number of whitespace entries.
    pub fn whitespaces_count(&self) -> usize {
        self.arr.len()
    }

    /// Id of the whitespace at `index`.
    pub fn id_for_whitespace_index(&self, index: usize) -> &str {
        &self.arr[index].id
    }

    /// The line the whitespace at `index` follows.
    pub fn after_line_number_for_whitespace_index(&self, index: usize) -> i32 {
        self.arr[index].after_line_number
    }

    /// Height of the whitespace at `index`.
    pub fn height_for_whitespace_index(&self, index: usize) -> i32 {
        self.arr[index].height
    }
}

#[cfg(test)]
mod tests;
