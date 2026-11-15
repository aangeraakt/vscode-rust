//! Environment variable merge rules.
//!
//! Ported from:
//! - `src/vs/workbench/contrib/terminal/common/terminalEnvironment.ts` (`mergeEnvironments`,
//!   `_mergeEnvironmentValue`);
//! - `src/vs/base/common/objects.ts` (`getCaseInsensitive`).
//!
//! Matched behavior (see the tests):
//! - a key present in the patch with a value sets that key in the parent;
//! - a key present in the patch with `None` (the TypeScript `null`) deletes it from the parent;
//! - a key absent from the patch (the TypeScript `undefined`) leaves the parent alone;
//! - on Windows, patch keys match parent keys ignoring case, and the existing key keeps its
//!   spelling.

use std::collections::BTreeMap;

/// A full process environment: variable name to value.
pub type ProcessEnvironment = BTreeMap<String, String>;

/// A change set applied over a [`ProcessEnvironment`]. `None` removes the variable.
pub type EnvironmentPatch = BTreeMap<String, Option<String>>;

/// Applies `patch` to `parent` in place (`mergeEnvironments`).
///
/// `is_windows` selects the Windows key rules: patch keys match parent keys ignoring case, and
/// the spelling already in `parent` is kept. Elsewhere keys are case-sensitive.
pub fn merge_environments(
    parent: &mut ProcessEnvironment,
    patch: &EnvironmentPatch,
    is_windows: bool,
) {
    for (patch_key, value) in patch {
        let key = if is_windows {
            let lower = patch_key.to_lowercase();
            parent
                .keys()
                .find(|k| k.to_lowercase() == lower)
                .cloned()
                .unwrap_or_else(|| patch_key.clone())
        } else {
            patch_key.clone()
        };
        match value {
            Some(value) => {
                parent.insert(key, value.clone());
            }
            None => {
                parent.remove(&key);
            }
        }
    }
}

/// Looks up `key` ignoring case: the first key that matches ignoring case wins (the TypeScript
/// order), and an exact `key` is the fallback. Port of `getCaseInsensitive`.
pub fn get_case_insensitive<'a>(env: &'a ProcessEnvironment, key: &str) -> Option<&'a String> {
    let lower = key.to_lowercase();
    env.iter()
        .find(|(k, _)| k.to_lowercase() == lower)
        .map(|(_, v)| v)
        .or_else(|| env.get(key))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env(pairs: &[(&str, &str)]) -> ProcessEnvironment {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    fn patch(pairs: &[(&str, Option<&str>)]) -> EnvironmentPatch {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.map(str::to_string)))
            .collect()
    }

    #[test]
    fn sets_adds_and_deletes_keys() {
        let mut parent = env(&[("A", "1"), ("B", "2")]);
        merge_environments(
            &mut parent,
            &patch(&[("A", Some("x")), ("C", Some("3")), ("B", None)]),
            false,
        );
        assert_eq!(parent, env(&[("A", "x"), ("C", "3")]));
    }

    #[test]
    fn absent_keys_are_left_alone() {
        let mut parent = env(&[("A", "1"), ("B", "2")]);
        merge_environments(&mut parent, &patch(&[("A", Some("9"))]), false);
        assert_eq!(parent, env(&[("A", "9"), ("B", "2")]));
    }

    #[test]
    fn windows_matches_keys_ignoring_case_and_keeps_parent_spelling() {
        let mut parent = env(&[("Path", r"C:\bin")]);
        merge_environments(&mut parent, &patch(&[("PATH", Some(r"D:\tools"))]), true);
        assert_eq!(parent, env(&[("Path", r"D:\tools")]));

        merge_environments(&mut parent, &patch(&[("path", None)]), true);
        assert!(parent.is_empty());
    }

    #[test]
    fn unix_keys_are_case_sensitive() {
        let mut parent = env(&[("Path", "a")]);
        merge_environments(&mut parent, &patch(&[("PATH", Some("b"))]), false);
        assert_eq!(parent, env(&[("Path", "a"), ("PATH", "b")]));
    }

    #[test]
    fn get_case_insensitive_matches_any_case() {
        let e = env(&[("Path", "one")]);
        assert_eq!(
            get_case_insensitive(&e, "PATH").map(String::as_str),
            Some("one")
        );
        assert_eq!(
            get_case_insensitive(&e, "Path").map(String::as_str),
            Some("one")
        );
        assert_eq!(get_case_insensitive(&e, "HOME"), None);
    }
}
