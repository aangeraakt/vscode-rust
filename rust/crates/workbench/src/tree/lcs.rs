//! Minimal longest-common-subsequence diff for the index tree model's smart splice.
//!
//! Stands in for `LcsDiff` from `src/vs/base/common/diff/diff.ts` (with `pretty = false`),
//! which is not ported yet. It trims the common prefix and suffix, then runs a dynamic program
//! on the rest. Like `LcsDiff`'s `quitEarly`, it gives up (returns `None`) on large inputs, and
//! the caller falls back to a plain splice. Where several minimal diffs exist, the one picked
//! here may differ from `LcsDiff`'s; the resulting tree is the same either way.

/// One changed region (`DiffChange`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct DiffChange {
    pub original_start: usize,
    pub original_length: usize,
    pub modified_start: usize,
    pub modified_length: usize,
}

/// Largest `original * modified` middle section that is diffed; above it the diff gives up.
const MAX_CELLS: usize = 4_000_000;

/// The changes that turn `original` into `modified`, in ascending order, or `None` when the
/// inputs are too large.
pub(crate) fn diff(original: &[String], modified: &[String]) -> Option<Vec<DiffChange>> {
    let prefix = original
        .iter()
        .zip(modified)
        .take_while(|(a, b)| a == b)
        .count();
    let (o, m) = (&original[prefix..], &modified[prefix..]);
    let suffix = o
        .iter()
        .rev()
        .zip(m.iter().rev())
        .take_while(|(a, b)| a == b)
        .count();
    let (o, m) = (&o[..o.len() - suffix], &m[..m.len() - suffix]);
    let (n, k) = (o.len(), m.len());
    if n.saturating_mul(k) > MAX_CELLS {
        return None;
    }
    // lcs[i * (k + 1) + j] = LCS length of o[i..] and m[j..].
    let width = k + 1;
    let mut lcs = vec![0u32; (n + 1) * width];
    for i in (0..n).rev() {
        for j in (0..k).rev() {
            lcs[i * width + j] = if o[i] == m[j] {
                lcs[(i + 1) * width + j + 1] + 1
            } else {
                lcs[(i + 1) * width + j].max(lcs[i * width + j + 1])
            };
        }
    }
    let mut changes = Vec::new();
    let (mut i, mut j) = (0, 0);
    let mut open: Option<DiffChange> = None;
    while i < n || j < k {
        if i < n && j < k && o[i] == m[j] {
            changes.extend(open.take());
            i += 1;
            j += 1;
            continue;
        }
        let change = open.get_or_insert(DiffChange {
            original_start: prefix + i,
            original_length: 0,
            modified_start: prefix + j,
            modified_length: 0,
        });
        if j < k && (i == n || lcs[i * width + j + 1] >= lcs[(i + 1) * width + j]) {
            change.modified_length += 1;
            j += 1;
        } else {
            change.original_length += 1;
            i += 1;
        }
    }
    changes.extend(open);
    Some(changes)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids(s: &str) -> Vec<String> {
        s.chars().map(String::from).collect()
    }

    fn apply(original: &[String], modified: &[String], changes: &[DiffChange]) -> Vec<String> {
        let mut out = original.to_vec();
        for c in changes.iter().rev() {
            out.splice(
                c.original_start..c.original_start + c.original_length,
                modified[c.modified_start..c.modified_start + c.modified_length]
                    .iter()
                    .cloned(),
            );
        }
        out
    }

    #[test]
    fn diff_reproduces_modified_sequence() {
        for (a, b) in [
            ("", ""),
            ("abc", "abc"),
            ("abc", ""),
            ("", "abc"),
            ("abcde", "axcye"),
            ("abc", "cab"),
            ("aaaa", "aa"),
        ] {
            let (a, b) = (ids(a), ids(b));
            let changes = diff(&a, &b).unwrap();
            assert_eq!(apply(&a, &b, &changes), b);
        }
    }

    #[test]
    fn diff_keeps_common_elements() {
        let changes = diff(&ids("abcde"), &ids("axcye")).unwrap();
        assert_eq!(
            changes,
            [
                DiffChange {
                    original_start: 1,
                    original_length: 1,
                    modified_start: 1,
                    modified_length: 1
                },
                DiffChange {
                    original_start: 3,
                    original_length: 1,
                    modified_start: 3,
                    modified_length: 1
                },
            ]
        );
    }
}
