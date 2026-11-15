//! Line heights, including custom heights set by decorations.
//!
//! Ported from `src/vs/editor/common/viewLayout/lineHeights.ts` (`LineHeightsManager`,
//! `CustomLine`, `CustomLineHeightData`).
//!
//! Custom lines are kept in one ordered list sorted by line number. Lines without a custom
//! height use the default height, so a line's accumulated height is the default height times its
//! line count plus the extra height of the custom lines above it. Queries binary-search the list,
//! so their cost depends on the number of custom lines, not on the number of lines in the file.
//!
//! Divergences from the TypeScript:
//! - Changes are applied immediately instead of being queued until the next read. Each queued
//!   change is processed in order either way, so the results match.
//! - Lookups by decoration id scan the ordered list instead of keeping a second id map.
//! - `CustomLineHeightData.fromDecorations` is not ported. It needs the editor configuration and
//!   model decorations, which this crate does not depend on yet.

use std::collections::HashSet;

/// A custom line height for a range of lines, owned by one decoration.
///
/// Ported from `CustomLineHeightData`.
#[derive(Debug, Clone, PartialEq)]
pub struct CustomLineHeightData {
    /// The decoration that owns this height.
    pub decoration_id: String,
    /// First line of the range, 1-based and inclusive.
    pub start_line_number: i32,
    /// Last line of the range, 1-based and inclusive.
    pub end_line_number: i32,
    /// The height in pixels.
    pub line_height: f64,
}

impl CustomLineHeightData {
    /// Creates the data for one decoration's line range.
    pub fn new(
        decoration_id: impl Into<String>,
        start_line_number: i32,
        end_line_number: i32,
        line_height: f64,
    ) -> Self {
        Self {
            decoration_id: decoration_id.into(),
            start_line_number,
            end_line_number,
            line_height,
        }
    }
}

/// One line with a custom height (`CustomLine`).
#[derive(Debug, Clone)]
struct CustomLine {
    decoration_id: String,
    line_number: i32,
    special_height: f64,
    /// Total height above this line, before its own height.
    prefix_sum: f64,
    /// The tallest custom height among the custom lines with this line number.
    maximum_special_height: f64,
    deleted: bool,
}

impl CustomLine {
    fn new(decoration_id: &str, line_number: i32, special_height: f64) -> Self {
        let height = special_height.round();
        Self {
            decoration_id: decoration_id.to_owned(),
            line_number,
            special_height: height,
            prefix_sum: 0.0,
            maximum_special_height: height,
            deleted: false,
        }
    }
}

/// Work staged during one change, flushed into the ordered list at the end of the change.
#[derive(Debug, Default)]
struct Staging {
    inserts: Vec<CustomLine>,
    /// Smallest index in the ordered list whose prefix sums must be recomputed.
    invalid_index: Option<usize>,
}

fn mark_invalid(staging: &mut Staging, index: usize) {
    staging.invalid_index = Some(staging.invalid_index.map_or(index, |i| i.min(index)));
}

/// Binary search with the same probing as `binarySearch2` in `base/common/arrays.ts`.
///
/// Returns the index of a line numbered `line_number`, or `-(insertion point + 1)` when there is
/// none.
fn search(lines: &[CustomLine], line_number: i32) -> isize {
    let mut low: isize = 0;
    let mut high: isize = lines.len() as isize - 1;
    while low <= high {
        let mid = (low + high) / 2;
        let found = lines[mid as usize].line_number;
        if found == line_number {
            return mid;
        } else if found < line_number {
            low = mid + 1;
        } else {
            high = mid - 1;
        }
    }
    -(low + 1)
}

/// Index used by `_doLinesDeleted` when a search did not find the line number. An insertion point
/// past the end maps to the last index, as in the TypeScript.
fn fallback_index(candidate: isize, len: isize) -> isize {
    let insertion = -(candidate + 1);
    if insertion == len && len > 0 {
        len - 1
    } else {
        insertion
    }
}

/// Manages line heights: the default height plus custom heights from decorations.
///
/// Ported from `LineHeightsManager`.
#[derive(Debug, Clone)]
pub struct LineHeightsManager {
    ordered: Vec<CustomLine>,
    default_line_height: f64,
}

impl LineHeightsManager {
    /// Creates a manager with the default height and the given custom heights.
    pub fn new(default_line_height: f64, custom_line_heights: &[CustomLineHeightData]) -> Self {
        let mut manager = Self {
            ordered: Vec::new(),
            default_line_height,
        };
        for data in custom_line_heights {
            manager.insert_or_change_custom_line_height(
                &data.decoration_id,
                data.start_line_number,
                data.end_line_number,
                data.line_height,
            );
        }
        manager
    }

    /// The height of a line without a custom height.
    pub fn default_line_height(&self) -> f64 {
        self.default_line_height
    }

    /// Sets the height of a line without a custom height.
    pub fn set_default_line_height(&mut self, line_height: f64) {
        self.default_line_height = line_height;
    }

    /// Gives the lines in `start..=end` the custom height `line_height`, replacing any custom
    /// heights that `decoration_id` already has.
    pub fn insert_or_change_custom_line_height(
        &mut self,
        decoration_id: &str,
        start_line_number: i32,
        end_line_number: i32,
        line_height: f64,
    ) {
        let mut staging = Staging::default();
        self.do_insert_or_change(
            decoration_id,
            start_line_number,
            end_line_number,
            line_height,
            &mut staging,
        );
        self.flush(&mut staging);
    }

    /// Removes every custom height that `decoration_id` owns.
    pub fn remove_custom_line_height(&mut self, decoration_id: &str) {
        let mut staging = Staging::default();
        self.do_remove(decoration_id, &mut staging);
        self.flush(&mut staging);
    }

    /// The height of one line, in pixels.
    pub fn height_for_line_number(&self, line_number: i32) -> f64 {
        let index = search(&self.ordered, line_number);
        if index >= 0 {
            self.ordered[index as usize].maximum_special_height
        } else {
            self.default_line_height
        }
    }

    /// The sum of the heights of lines `1..=line_number`.
    pub fn accumulated_line_heights_including_line_number(&self, line_number: i32) -> f64 {
        let index = search(&self.ordered, line_number);
        if index >= 0 {
            let line = &self.ordered[index as usize];
            return line.prefix_sum + line.maximum_special_height;
        }
        if index == -1 {
            return self.default_line_height * f64::from(line_number);
        }
        let previous = &self.ordered[(-(index + 1) - 1) as usize];
        previous.prefix_sum
            + previous.maximum_special_height
            + self.default_line_height * f64::from(line_number - previous.line_number)
    }

    /// Lines `from..=to` were deleted. Custom lines in the range collapse onto `from`.
    pub fn on_lines_deleted(&mut self, from_line_number: i32, to_line_number: i32) {
        self.do_lines_deleted(from_line_number, to_line_number);
    }

    /// Lines `from..=to` were inserted. Decorations that spanned the insertion point cover the
    /// inserted lines too.
    pub fn on_lines_inserted(&mut self, from_line_number: i32, to_line_number: i32) {
        let mut staging = Staging::default();
        self.do_lines_inserted(from_line_number, to_line_number, &mut staging);
        self.flush(&mut staging);
    }

    /// Removes the custom lines of `decoration_id`, including ones staged in this change.
    fn do_remove(&mut self, decoration_id: &str, staging: &mut Staging) {
        for (index, line) in self.ordered.iter_mut().enumerate() {
            if !line.deleted && line.decoration_id == decoration_id {
                line.deleted = true;
                mark_invalid(staging, index);
            }
        }
        for line in &mut staging.inserts {
            if line.decoration_id == decoration_id {
                line.deleted = true;
            }
        }
    }

    fn do_insert_or_change(
        &mut self,
        decoration_id: &str,
        start_line_number: i32,
        end_line_number: i32,
        line_height: f64,
        staging: &mut Staging,
    ) {
        self.do_remove(decoration_id, staging);
        for line_number in start_line_number..=end_line_number {
            staging
                .inserts
                .push(CustomLine::new(decoration_id, line_number, line_height));
        }
    }

    /// Inserts the staged lines into the ordered list and recomputes prefix sums from the first
    /// changed index. Deleted lines are dropped.
    fn flush(&mut self, staging: &mut Staging) {
        if staging.inserts.is_empty() && staging.invalid_index.is_none() {
            return;
        }
        for line in std::mem::take(&mut staging.inserts) {
            if line.deleted {
                continue;
            }
            let candidate = search(&self.ordered, line.line_number);
            let insertion = if candidate >= 0 {
                candidate
            } else {
                -(candidate + 1)
            };
            let index = insertion as usize;
            self.ordered.insert(index, line);
            mark_invalid(staging, index);
        }
        let Some(invalid_index) = staging.invalid_index.take() else {
            return;
        };
        self.recompute_from(invalid_index);
    }

    /// Recomputes `maximum_special_height` and `prefix_sum` for the lines from `invalid_index`
    /// on, and drops deleted lines.
    fn recompute_from(&mut self, invalid_index: usize) {
        let mut rest = self.ordered.split_off(invalid_index);
        // (line number, maximum height, prefix sum) of the last kept line.
        let mut previous: Option<(i32, f64, f64)> = self
            .ordered
            .last()
            .map(|l| (l.line_number, l.maximum_special_height, l.prefix_sum));

        for index in 0..rest.len() {
            if rest[index].deleted {
                continue;
            }
            let line_number = rest[index].line_number;
            let (maximum, prefix) = match previous {
                Some((prev_line, prev_max, prev_prefix)) if prev_line == line_number => {
                    (prev_max, prev_prefix)
                }
                _ => {
                    let mut maximum = rest[index].special_height;
                    for next in &rest[index..] {
                        if next.deleted {
                            continue;
                        }
                        if next.line_number != line_number {
                            break;
                        }
                        maximum = maximum.max(next.special_height);
                    }
                    let prefix = match previous {
                        Some((prev_line, prev_max, prev_prefix)) => {
                            prev_prefix
                                + prev_max
                                + self.default_line_height * f64::from(line_number - prev_line - 1)
                        }
                        None => self.default_line_height * f64::from(line_number - 1),
                    };
                    (maximum, prefix)
                }
            };
            rest[index].maximum_special_height = maximum;
            rest[index].prefix_sum = prefix;
            previous = Some((line_number, maximum, prefix));
        }

        self.ordered.extend(rest.into_iter().filter(|l| !l.deleted));
    }

    /// Port of `_doLinesDeleted`: the range `from..=to` is removed from the custom lines.
    fn do_lines_deleted(&mut self, from_line_number: i32, to_line_number: i32) {
        let delete_count = to_line_number - from_line_number + 1;
        let len = self.ordered.len() as isize;

        let candidate_start = search(&self.ordered, from_line_number);
        let start: isize = if candidate_start >= 0 {
            let mut start = candidate_start;
            while start > 0 && self.ordered[(start - 1) as usize].line_number == from_line_number {
                start -= 1;
            }
            start
        } else {
            fallback_index(candidate_start, len)
        };

        let candidate_end = search(&self.ordered, to_line_number);
        let end: isize = if candidate_end >= 0 {
            let mut end = candidate_end;
            while end + 1 < len && self.ordered[(end + 1) as usize].line_number == to_line_number {
                end += 1;
            }
            end
        } else {
            fallback_index(candidate_end, len)
        };

        let covers_one_line = end == start
            && start >= 0
            && start < len
            && self.ordered[start as usize].line_number >= from_line_number
            && self.ordered[start as usize].line_number <= to_line_number;

        if end > start || covers_one_line {
            let default = self.default_line_height;
            let (start_u, end_u) = (start as usize, end as usize);

            let maximum_on_deleted = self.ordered[start_u..=end_u]
                .iter()
                .fold(0.0_f64, |m, l| m.max(l.maximum_special_height));

            let prefix_on_deleted = if start > 0 {
                let previous = &self.ordered[start_u - 1];
                previous.prefix_sum
                    + previous.maximum_special_height
                    + default * f64::from(from_line_number - previous.line_number - 1)
            } else if from_line_number > 0 {
                f64::from(from_line_number - 1) * default
            } else {
                0.0
            };

            let first = &self.ordered[start_u];
            let last = &self.ordered[end_u];
            let height_after_deletion = match self.ordered.get(end_u + 1) {
                Some(after) if after.line_number == to_line_number + 1 => {
                    after.maximum_special_height
                }
                _ => default,
            };
            let total_height_deleted = last.prefix_sum + last.maximum_special_height
                - first.prefix_sum
                + default * f64::from(to_line_number - last.line_number)
                + default * f64::from(first.line_number - from_line_number)
                + height_after_deletion
                - maximum_on_deleted;

            let mut seen: HashSet<String> = HashSet::new();
            let mut kept = Vec::with_capacity(self.ordered.len());
            for (index, mut line) in std::mem::take(&mut self.ordered).into_iter().enumerate() {
                let index = index as isize;
                let decoration_id = line.decoration_id.clone();
                if index < start {
                    kept.push(line);
                } else if index <= end {
                    if !seen.contains(&decoration_id) {
                        line.line_number = from_line_number;
                        line.prefix_sum = prefix_on_deleted;
                        line.maximum_special_height = maximum_on_deleted;
                        kept.push(line);
                    }
                } else {
                    line.line_number -= delete_count;
                    line.prefix_sum -= total_height_deleted;
                    kept.push(line);
                }
                seen.insert(decoration_id);
            }
            self.ordered = kept;
        } else {
            let total_height_deleted = f64::from(delete_count) * self.default_line_height;
            for line in self.ordered.iter_mut().skip(end.max(0) as usize) {
                if line.line_number > to_line_number {
                    line.line_number -= delete_count;
                    line.prefix_sum -= total_height_deleted;
                }
            }
        }
    }

    /// Port of `_doLinesInserted`: decorations that span the insertion point are re-added over
    /// the inserted lines.
    fn do_lines_inserted(
        &mut self,
        from_line_number: i32,
        to_line_number: i32,
        staging: &mut Staging,
    ) {
        let insert_count = to_line_number - from_line_number + 1;
        let candidate = search(&self.ordered, from_line_number);
        let start: usize = if candidate >= 0 {
            let mut start = candidate as usize;
            while start > 0 && self.ordered[start - 1].line_number == from_line_number {
                start -= 1;
            }
            start
        } else {
            (-(candidate + 1)) as usize
        };

        let mut after: Vec<String> = Vec::new();
        for line in &self.ordered[start..] {
            if line.line_number == from_line_number && !after.contains(&line.decoration_id) {
                after.push(line.decoration_id.clone());
            }
        }
        let mut before: Vec<String> = Vec::new();
        for line in self.ordered[..start].iter().rev() {
            if line.line_number == from_line_number - 1 && !before.contains(&line.decoration_id) {
                before.push(line.decoration_id.clone());
            }
        }
        let spanning: Vec<String> = before.into_iter().filter(|id| after.contains(id)).collect();

        let prefix_to_add = f64::from(insert_count) * self.default_line_height;
        for line in &mut self.ordered[start..] {
            line.line_number += insert_count;
            line.prefix_sum += prefix_to_add;
        }

        // Collect every spanning decoration's range before re-adding any, as the TypeScript does.
        let mut to_re_add: Vec<(String, i32, i32, f64)> = Vec::new();
        for decoration_id in spanning {
            let mut start_line = from_line_number;
            let mut end_line = from_line_number;
            let mut line_height = 0.0_f64;
            for line in self
                .ordered
                .iter()
                .filter(|l| !l.deleted && l.decoration_id == decoration_id)
            {
                start_line = start_line.min(line.line_number);
                end_line = end_line.max(line.line_number);
                line_height = line_height.max(line.special_height);
            }
            to_re_add.push((decoration_id, start_line, end_line, line_height));
        }
        for (decoration_id, start_line, end_line, line_height) in to_re_add {
            self.do_insert_or_change(&decoration_id, start_line, end_line, line_height, staging);
        }
    }
}

/// Changes to custom line heights, applied through [`LineHeightsManager`].
///
/// Ported from `ILineHeightChangeAccessor`.
#[derive(Debug)]
pub struct LineHeightChangeAccessor<'a> {
    manager: &'a mut LineHeightsManager,
    had_change: bool,
}

impl<'a> LineHeightChangeAccessor<'a> {
    /// Creates an accessor over `manager`. Used by the layout model's batch method.
    pub fn new(manager: &'a mut LineHeightsManager) -> Self {
        Self {
            manager,
            had_change: false,
        }
    }

    /// Gives lines `start..=end` a custom height for `decoration_id`.
    pub fn insert_or_change_custom_line_height(
        &mut self,
        decoration_id: &str,
        start_line_number: i32,
        end_line_number: i32,
        line_height: f64,
    ) {
        self.had_change = true;
        self.manager.insert_or_change_custom_line_height(
            decoration_id,
            start_line_number,
            end_line_number,
            line_height,
        );
    }

    /// Removes the custom heights of `decoration_id`.
    pub fn remove_custom_line_height(&mut self, decoration_id: &str) {
        self.had_change = true;
        self.manager.remove_custom_line_height(decoration_id);
    }

    /// Whether the callback made any change.
    pub fn had_change(&self) -> bool {
        self.had_change
    }
}

#[cfg(test)]
mod tests;
