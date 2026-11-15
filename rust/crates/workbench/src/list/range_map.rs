//! Item sizes and positions for a virtualized list.
//!
//! Ported from `src/vs/base/browser/ui/list/rangeMap.ts` (`RangeMap`, `groupIntersect`, `shift`,
//! `consolidate`) and the half-open range helpers it uses from `src/vs/base/common/range.ts`
//! (`Range.intersect`, `Range.isEmpty`).
//!
//! A [`RangeMap`] stores item sizes as runs ("groups") of consecutive items with the same size,
//! so a list of 10,000 rows of equal height is a single group.
//!
//! Intentional differences from the TypeScript:
//! - Indices are `usize` and positions/sizes are `i64`. Where TypeScript returns `-1`,
//!   [`RangeMap::index_at`] and [`RangeMap::position_at`] return `None`.
//! - `splice` takes the new item sizes directly (`IItem` only carries `size`).
//! - The open end of the "after" range in `splice` is `usize::MAX` instead of `Infinity`.

/// A half-open range of item indices, `start..end` (`IRange`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct IndexRange {
    /// First index in the range.
    pub start: usize,
    /// One past the last index in the range.
    pub end: usize,
}

impl IndexRange {
    /// Creates `start..end`.
    pub const fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }

    /// Whether the range holds no index (`Range.isEmpty`).
    pub fn is_empty(&self) -> bool {
        self.end <= self.start
    }

    /// Number of indices in the range.
    pub fn len(&self) -> usize {
        self.end.saturating_sub(self.start)
    }

    /// The intersection of two ranges, `0..0` when it is empty (`Range.intersect`).
    pub fn intersect(&self, other: &IndexRange) -> IndexRange {
        if self.start >= other.end || other.start >= self.end {
            return IndexRange::default();
        }
        let start = self.start.max(other.start);
        let end = self.end.min(other.end);
        if end <= start {
            return IndexRange::default();
        }
        IndexRange { start, end }
    }
}

/// A run of consecutive items that all have the same size (`IRangedGroup`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RangedGroup {
    /// The item indices of the run.
    pub range: IndexRange,
    /// The size of each item in the run.
    pub size: i64,
}

impl RangedGroup {
    /// Creates a group for `start..end` with items of `size`.
    pub const fn new(start: usize, end: usize, size: i64) -> Self {
        Self {
            range: IndexRange::new(start, end),
            size,
        }
    }
}

/// The parts of `groups` that intersect `range`, in order; empty if none (`groupIntersect`).
///
/// `groups` must be sorted and non-overlapping, as a [`RangeMap`] keeps them.
pub fn group_intersect(range: IndexRange, groups: &[RangedGroup]) -> Vec<RangedGroup> {
    let mut result = Vec::new();
    for group in groups {
        if range.start >= group.range.end {
            continue;
        }
        if range.end < group.range.start {
            break;
        }
        let intersection = range.intersect(&group.range);
        if intersection.is_empty() {
            continue;
        }
        result.push(RangedGroup {
            range: intersection,
            size: group.size,
        });
    }
    result
}

/// Shifts a range by `much` (`shift`).
///
/// # Panics
/// If the result would be negative or overflow `usize`.
pub fn shift(range: IndexRange, much: isize) -> IndexRange {
    let add = |v: usize| {
        v.checked_add_signed(much)
            .expect("shifted range out of bounds")
    };
    IndexRange::new(add(range.start), add(range.end))
}

/// Merges consecutive groups that share the same size (`consolidate`).
pub fn consolidate(groups: impl IntoIterator<Item = RangedGroup>) -> Vec<RangedGroup> {
    let mut result: Vec<RangedGroup> = Vec::new();
    for group in groups {
        if let Some(previous) = result.last_mut()
            && previous.size == group.size
        {
            previous.range.end = group.range.end;
            continue;
        }
        result.push(group);
    }
    result
}

/// Sizes and positions of list items, stored as runs of equal size (`RangeMap`).
#[derive(Debug, Clone, Default)]
pub struct RangeMap {
    groups: Vec<RangedGroup>,
    size: i64,
    padding_top: i64,
}

impl RangeMap {
    /// An empty map with no top padding.
    pub fn new() -> Self {
        Self::default()
    }

    /// An empty map whose first item starts at `padding_top`.
    pub fn with_padding_top(padding_top: i64) -> Self {
        Self {
            groups: Vec::new(),
            size: padding_top,
            padding_top,
        }
    }

    /// Space before the first item.
    pub fn padding_top(&self) -> i64 {
        self.padding_top
    }

    /// Changes the space before the first item; [`RangeMap::size`] follows.
    pub fn set_padding_top(&mut self, padding_top: i64) {
        self.size = self.size + padding_top - self.padding_top;
        self.padding_top = padding_top;
    }

    /// The runs of equal-size items, in index order.
    pub fn groups(&self) -> &[RangedGroup] {
        &self.groups
    }

    /// Removes `delete_count` items at `index` and inserts items with the given sizes there.
    pub fn splice(&mut self, index: usize, delete_count: usize, sizes: &[i64]) {
        let before = group_intersect(IndexRange::new(0, index), &self.groups);
        let after_start = index.saturating_add(delete_count);
        let after = group_intersect(IndexRange::new(after_start, usize::MAX), &self.groups);
        let middle = sizes
            .iter()
            .enumerate()
            .map(|(i, &size)| RangedGroup::new(index + i, index + i + 1, size));
        let diff = sizes.len() as isize - delete_count as isize;
        let after = after.into_iter().map(|g| RangedGroup {
            range: shift(g.range, diff),
            size: g.size,
        });
        self.groups = consolidate(before.into_iter().chain(middle).chain(after));
        self.size = self.padding_top
            + self
                .groups
                .iter()
                .map(|g| g.size * g.range.len() as i64)
                .sum::<i64>();
    }

    /// The number of items (`count`).
    pub fn count(&self) -> usize {
        self.groups.last().map_or(0, |g| g.range.end)
    }

    /// The top padding plus the sum of all item sizes (`size`).
    pub fn size(&self) -> i64 {
        self.size
    }

    /// The index of the item at `position` (`indexAt`).
    ///
    /// Positions inside the top padding map to `0`; positions past the end map to
    /// [`RangeMap::count`]. `None` for negative positions (TypeScript returns `-1`).
    pub fn index_at(&self, position: i64) -> Option<usize> {
        if position < 0 {
            return None;
        }
        if position < self.padding_top {
            return Some(0);
        }
        let mut index = 0;
        let mut size = self.padding_top;
        for group in &self.groups {
            let count = group.range.len();
            let new_size = size + count as i64 * group.size;
            if position < new_size {
                // `position < new_size` implies a positive group size here.
                return Some(index + ((position - size) / group.size) as usize);
            }
            index += count;
            size = new_size;
        }
        Some(index)
    }

    /// The index right after the item at `position`, capped at [`RangeMap::count`]
    /// (`indexAfter`).
    pub fn index_after(&self, position: i64) -> usize {
        match self.index_at(position) {
            Some(index) => (index + 1).min(self.count()),
            None => 0,
        }
    }

    /// The start position of the item at `index`, or `None` past the end (`positionAt`).
    pub fn position_at(&self, index: usize) -> Option<i64> {
        let mut position = 0;
        let mut count = 0;
        for group in &self.groups {
            let group_count = group.range.len();
            let new_count = count + group_count;
            if index < new_count {
                return Some(self.padding_top + position + (index - count) as i64 * group.size);
            }
            position += group_count as i64 * group.size;
            count = new_count;
        }
        None
    }
}

#[cfg(test)]
mod tests {
    //! Ported from `src/vs/base/test/browser/ui/list/rangeMap.test.ts`. Test names follow the
    //! upstream `suite > test` names. `indexAt(-1) === -1` becomes `None`, `positionAt(...)
    //! === -1` becomes `None`; the upstream `positionAt(-1)` calls have no `usize` equivalent
    //! and are dropped.

    use super::*;

    fn r(start: usize, end: usize) -> IndexRange {
        IndexRange::new(start, end)
    }

    fn g(start: usize, end: usize, size: i64) -> RangedGroup {
        RangedGroup::new(start, end, size)
    }

    fn map(sizes: &[i64]) -> RangeMap {
        let mut m = RangeMap::new();
        m.splice(0, 0, sizes);
        m
    }

    fn assert_size_count(m: &RangeMap, size: i64, count: usize) {
        assert_eq!(m.size(), size, "size");
        assert_eq!(m.count(), count, "count");
    }

    #[test]
    fn intersection() {
        let cases = [
            ((0, 0), (0, 0), (0, 0)),
            ((0, 0), (5, 5), (0, 0)),
            ((0, 1), (5, 6), (0, 0)),
            ((5, 6), (0, 1), (0, 0)),
            ((0, 5), (2, 2), (0, 0)),
            ((0, 1), (0, 1), (0, 1)),
            ((0, 10), (0, 5), (0, 5)),
            ((0, 5), (0, 10), (0, 5)),
            ((0, 10), (5, 10), (5, 10)),
            ((5, 10), (0, 10), (5, 10)),
            ((0, 10), (2, 8), (2, 8)),
            ((2, 8), (0, 10), (2, 8)),
            ((0, 10), (5, 15), (5, 10)),
            ((5, 15), (0, 10), (5, 10)),
        ];
        for (a, b, expected) in cases {
            assert_eq!(
                r(a.0, a.1).intersect(&r(b.0, b.1)),
                r(expected.0, expected.1),
                "{a:?} & {b:?}"
            );
        }
    }

    #[test]
    fn multi_intersect() {
        let one = [g(0, 10, 1)];
        let two = [g(0, 10, 1), g(10, 20, 5)];
        let three = [g(0, 10, 1), g(10, 20, 5), g(20, 30, 10)];
        assert_eq!(group_intersect(r(0, 0), &one), []);
        assert_eq!(group_intersect(r(10, 20), &one), []);
        assert_eq!(group_intersect(r(2, 8), &one), [g(2, 8, 1)]);
        assert_eq!(group_intersect(r(2, 8), &two), [g(2, 8, 1)]);
        assert_eq!(group_intersect(r(12, 18), &two), [g(12, 18, 5)]);
        assert_eq!(group_intersect(r(2, 18), &two), [g(2, 10, 1), g(10, 18, 5)]);
        assert_eq!(
            group_intersect(r(2, 28), &three),
            [g(2, 10, 1), g(10, 20, 5), g(20, 28, 10)]
        );
    }

    #[test]
    fn shift_moves_both_ends() {
        // Not an upstream test: `shift` is exercised upstream only through `splice`.
        assert_eq!(shift(r(2, 5), 3), r(5, 8));
        assert_eq!(shift(r(2, 5), -2), r(0, 3));
    }

    #[test]
    fn consolidate_groups() {
        assert_eq!(consolidate([]), []);
        assert_eq!(consolidate([g(0, 10, 1)]), [g(0, 10, 1)]);
        assert_eq!(consolidate([g(0, 10, 1), g(10, 20, 1)]), [g(0, 20, 1)]);
        assert_eq!(
            consolidate([g(0, 10, 1), g(10, 20, 1), g(20, 100, 1)]),
            [g(0, 100, 1)]
        );
        assert_eq!(
            consolidate([g(0, 10, 1), g(10, 20, 5), g(20, 30, 10)]),
            [g(0, 10, 1), g(10, 20, 5), g(20, 30, 10)]
        );
        assert_eq!(
            consolidate([g(0, 10, 1), g(10, 20, 2), g(20, 100, 2)]),
            [g(0, 10, 1), g(10, 100, 2)]
        );
    }

    #[test]
    fn empty() {
        assert_size_count(&RangeMap::new(), 0, 0);
    }

    #[test]
    fn length_and_count() {
        assert_size_count(&map(&[1]), 1, 1);
    }

    #[test]
    fn length_and_count_2() {
        assert_size_count(&map(&[1; 5]), 5, 5);
    }

    #[test]
    fn length_and_count_3() {
        assert_size_count(&map(&[5]), 5, 1);
    }

    #[test]
    fn length_and_count_4() {
        assert_size_count(&map(&[5; 5]), 25, 5);
    }

    #[test]
    fn insert() {
        let mut m = map(&[5; 5]);
        assert_size_count(&m, 25, 5);
        m.splice(0, 0, &[5; 5]);
        assert_size_count(&m, 50, 10);
        m.splice(5, 0, &[10, 10]);
        assert_size_count(&m, 70, 12);
        m.splice(12, 0, &[200]);
        assert_size_count(&m, 270, 13);
    }

    #[test]
    fn delete() {
        let mut m = map(&[5; 20]);
        assert_size_count(&m, 100, 20);
        m.splice(10, 5, &[]);
        assert_size_count(&m, 75, 15);
        m.splice(0, 1, &[]);
        assert_size_count(&m, 70, 14);
        m.splice(1, 13, &[]);
        assert_size_count(&m, 5, 1);
        m.splice(1, 1, &[]);
        assert_size_count(&m, 5, 1);
    }

    #[test]
    fn insert_and_delete() {
        let mut m = RangeMap::new();
        assert_size_count(&m, 0, 0);
        m.splice(0, 0, &[1]);
        assert_size_count(&m, 1, 1);
        m.splice(0, 1, &[]);
        assert_size_count(&m, 0, 0);
    }

    #[test]
    fn insert_and_delete_2() {
        let mut m = map(&[1; 10]);
        m.splice(2, 6, &[]);
        assert_size_count(&m, 4, 4);
    }

    fn ones_then_twos() -> Vec<i64> {
        [[1; 10], [2; 10]].concat()
    }

    #[test]
    fn insert_and_delete_3() {
        let mut m = map(&ones_then_twos());
        m.splice(8, 4, &[]);
        assert_size_count(&m, 24, 16);
    }

    #[test]
    fn insert_and_delete_4() {
        let mut m = map(&ones_then_twos());
        m.splice(5, 0, &[3; 5]);
        assert_size_count(&m, 45, 25);
        m.splice(4, 7, &[]);
        assert_size_count(&m, 28, 18);
    }

    #[test]
    fn index_at_position_at_empty() {
        let m = RangeMap::new();
        assert_eq!(m.index_at(0), Some(0));
        assert_eq!(m.index_at(10), Some(0));
        assert_eq!(m.index_at(-1), None);
        assert_eq!(m.position_at(0), None);
        assert_eq!(m.position_at(10), None);
    }

    #[test]
    fn index_at_position_at_simple() {
        let m = map(&[1]);
        assert_eq!(m.index_at(0), Some(0));
        assert_eq!(m.index_at(1), Some(1));
        assert_eq!(m.position_at(0), Some(0));
        assert_eq!(m.position_at(1), None);
    }

    #[test]
    fn index_at_position_at_simple_2() {
        let m = map(&[10]);
        assert_eq!(m.index_at(0), Some(0));
        assert_eq!(m.index_at(5), Some(0));
        assert_eq!(m.index_at(9), Some(0));
        assert_eq!(m.index_at(10), Some(1));
        assert_eq!(m.position_at(0), Some(0));
        assert_eq!(m.position_at(1), None);
    }

    #[test]
    fn index_at_position_at_insert() {
        let mut m = map(&[1; 10]);
        for (pos, idx) in [(0, 0), (1, 1), (5, 5), (9, 9), (10, 10), (11, 10)] {
            assert_eq!(m.index_at(pos), Some(idx), "index_at({pos})");
        }
        m.splice(10, 0, &[1; 10]);
        for (pos, idx) in [(10, 10), (19, 19), (20, 20), (21, 20)] {
            assert_eq!(m.index_at(pos), Some(idx), "index_at({pos})");
        }
        assert_eq!(m.position_at(0), Some(0));
        assert_eq!(m.position_at(1), Some(1));
        assert_eq!(m.position_at(19), Some(19));
        assert_eq!(m.position_at(20), None);
    }

    #[test]
    fn index_at_position_at_delete() {
        let mut m = map(&[1; 10]);
        m.splice(2, 6, &[]);
        for (pos, idx) in [(0, 0), (1, 1), (3, 3), (4, 4), (5, 4)] {
            assert_eq!(m.index_at(pos), Some(idx), "index_at({pos})");
        }
        assert_eq!(m.position_at(0), Some(0));
        assert_eq!(m.position_at(1), Some(1));
        assert_eq!(m.position_at(3), Some(3));
        assert_eq!(m.position_at(4), None);
    }

    #[test]
    fn index_at_position_at_delete_2() {
        let mut m = map(&[10; 10]);
        m.splice(2, 6, &[]);
        for (pos, idx) in [(0, 0), (1, 0), (30, 3), (40, 4), (50, 4)] {
            assert_eq!(m.index_at(pos), Some(idx), "index_at({pos})");
        }
        assert_eq!(m.position_at(0), Some(0));
        assert_eq!(m.position_at(1), Some(10));
        assert_eq!(m.position_at(2), Some(20));
        assert_eq!(m.position_at(3), Some(30));
        assert_eq!(m.position_at(4), None);
    }

    #[test]
    fn index_after_caps_at_count() {
        // Not an upstream test: `indexAfter` has no direct upstream test.
        let m = map(&[10; 3]);
        assert_eq!(m.index_after(-1), 0);
        assert_eq!(m.index_after(0), 1);
        assert_eq!(m.index_after(25), 3);
        assert_eq!(m.index_after(1000), 3);
    }

    #[test]
    fn many_equal_rows_stay_one_group() {
        // Not an upstream test: the M2 Explorer scenario lists 10,000 rows.
        let mut m = map(&[22; 10_000]);
        assert_eq!(m.groups().len(), 1);
        assert_size_count(&m, 220_000, 10_000);
        assert_eq!(m.index_at(22 * 5_000 + 3), Some(5_000));
        m.splice(5_000, 1, &[44]);
        assert_eq!(m.groups().len(), 3);
        assert_eq!(m.position_at(5_001), Some(22 * 5_000 + 44));
    }

    mod with_top_padding {
        use super::*;

        fn padded(sizes: &[i64]) -> RangeMap {
            let mut m = RangeMap::with_padding_top(10);
            m.splice(0, 0, sizes);
            m
        }

        #[test]
        fn empty() {
            assert_size_count(&RangeMap::with_padding_top(10), 10, 0);
        }

        #[test]
        fn length_and_count() {
            assert_size_count(&padded(&[1]), 11, 1);
        }

        #[test]
        fn length_and_count_2() {
            assert_size_count(&padded(&[1; 5]), 15, 5);
        }

        #[test]
        fn length_and_count_3() {
            assert_size_count(&padded(&[5]), 15, 1);
        }

        #[test]
        fn length_and_count_4() {
            assert_size_count(&padded(&[5; 5]), 35, 5);
        }

        #[test]
        fn insert() {
            let mut m = padded(&[5; 5]);
            assert_size_count(&m, 35, 5);
            m.splice(0, 0, &[5; 5]);
            assert_size_count(&m, 60, 10);
            m.splice(5, 0, &[10, 10]);
            assert_size_count(&m, 80, 12);
            m.splice(12, 0, &[200]);
            assert_size_count(&m, 280, 13);
        }

        #[test]
        fn index_at_position_at_empty() {
            let m = RangeMap::with_padding_top(10);
            assert_eq!(m.index_at(0), Some(0));
            assert_eq!(m.index_at(10), Some(0));
            assert_eq!(m.index_at(-1), None);
            assert_eq!(m.position_at(0), None);
            assert_eq!(m.position_at(10), None);
        }

        #[test]
        fn index_at_position_at_simple() {
            let m = padded(&[1]);
            assert_eq!(m.index_at(0), Some(0));
            assert_eq!(m.index_at(1), Some(0));
            assert_eq!(m.index_at(10), Some(0));
            assert_eq!(m.index_at(11), Some(1));
            assert_eq!(m.position_at(0), Some(10));
            assert_eq!(m.position_at(1), None);
        }

        #[test]
        fn set_padding_top_updates_size() {
            // Not an upstream test: covers the `paddingTop` setter.
            let mut m = padded(&[5; 2]);
            m.set_padding_top(4);
            assert_eq!(m.size(), 14);
            assert_eq!(m.position_at(1), Some(9));
        }
    }
}
