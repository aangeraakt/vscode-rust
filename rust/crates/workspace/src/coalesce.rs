//! Event coalescing for file watchers: `coalesceEvents` and its `EventCoalescer`, ported from
//! `src/vs/platform/files/common/watcher.ts`.
//!
//! A batch of raw changes is reduced to what a listener needs. Events for one path are merged
//! (a created then deleted file disappears, a deleted then created file becomes an update), and
//! deletions below an already deleted folder are dropped.

use std::collections::HashMap;
use std::path::MAIN_SEPARATOR;

use vscode_api::files::{FileChange, FileChangeType};

/// Reduces a batch of raw changes to the events worth reporting, in the order upstream reports
/// them: deletions first (shortest path first), then creations and updates.
pub fn coalesce_events(changes: Vec<FileChange>) -> Vec<FileChange> {
    let mut coalescer = EventCoalescer::default();
    for change in changes {
        coalescer.process(change);
    }
    coalescer.coalesce()
}

/// Key used to match events on the same path. Linux paths are case-sensitive; elsewhere the
/// comparison ignores case, as the file system does.
fn key_of(change: &FileChange) -> String {
    let path = change.resource.fs_path();
    if cfg!(target_os = "linux") {
        path
    } else {
        path.to_lowercase()
    }
}

#[derive(Default)]
struct EventCoalescer {
    /// Events to keep, in arrival order. A slot becomes `None` when an event cancels out.
    slots: Vec<Option<FileChange>>,
    /// The slot of the event that currently represents each path key.
    by_key: HashMap<String, usize>,
}

impl EventCoalescer {
    fn process(&mut self, event: FileChange) {
        let key = key_of(&event);
        let Some(&slot) = self.by_key.get(&key) else {
            self.keep(key, event);
            return;
        };
        let existing = self.slots[slot]
            .as_mut()
            .expect("by_key only points at kept slots");

        let existing_path = existing.resource.fs_path();
        let event_path = event.resource.fs_path();
        let current = existing.change_type;
        let next = event.change_type;

        if existing_path != event_path
            && matches!(next, FileChangeType::Deleted | FileChangeType::Added)
        {
            // Case renames on case-insensitive disks: keep both the deletion and the creation.
            self.keep(key, event);
        } else if current == FileChangeType::Added && next == FileChangeType::Deleted {
            // Created and deleted in one batch: nothing to report.
            self.slots[slot] = None;
            self.by_key.remove(&key);
        } else if current == FileChangeType::Deleted && next == FileChangeType::Added {
            // Deleted and recreated: the contents may have changed, so report an update.
            existing.change_type = FileChangeType::Updated;
        } else if current == FileChangeType::Added && next == FileChangeType::Updated {
            // Keep the creation; an update right after it adds nothing.
        } else {
            existing.change_type = next;
        }
    }

    fn keep(&mut self, key: String, event: FileChange) {
        self.slots.push(Some(event));
        self.by_key.insert(key, self.slots.len() - 1);
    }

    fn coalesce(self) -> Vec<FileChange> {
        let mut added_or_changed = Vec::new();
        let mut deleted = Vec::new();
        for change in self.slots.into_iter().flatten() {
            if change.change_type == FileChangeType::Deleted {
                deleted.push(change);
            } else {
                added_or_changed.push(change);
            }
        }

        // A deletion below a deleted folder is implied by the folder's deletion. Shorter paths
        // come first, so every folder is seen before its contents.
        deleted.sort_by_key(|change| change.resource.fs_path().len());
        let mut deleted_paths: Vec<String> = Vec::new();
        let mut result = Vec::new();
        for change in deleted {
            let path = change.resource.fs_path();
            let below_deleted_folder = deleted_paths
                .iter()
                .any(|parent| is_parent(&path, parent, !cfg!(target_os = "linux")));
            if below_deleted_folder {
                continue;
            }
            deleted_paths.push(path);
            result.push(change);
        }
        result.extend(added_or_changed);
        result
    }
}

/// Whether `path` is inside `candidate`. Ported from `isParent` in `common/files.ts`: the same
/// path is not its own parent, and the separator is checked, so `/a/bc` is not inside `/a/b`.
fn is_parent(path: &str, candidate: &str, ignore_case: bool) -> bool {
    if path.is_empty() || candidate.is_empty() || path == candidate {
        return false;
    }
    if candidate.len() > path.len() {
        return false;
    }
    let mut prefix = candidate.to_owned();
    if !prefix.ends_with(MAIN_SEPARATOR) {
        prefix.push(MAIN_SEPARATOR);
    }
    if ignore_case {
        path.to_lowercase().starts_with(&prefix.to_lowercase())
    } else {
        path.starts_with(&prefix)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vscode_api::uri::Uri;

    fn change(path: &str, change_type: FileChangeType) -> FileChange {
        FileChange {
            change_type,
            resource: Uri::file(path),
            correlation_id: None,
        }
    }

    /// The `(path, type)` pairs of a result, for comparison in tests.
    fn summary(changes: &[FileChange]) -> Vec<(String, FileChangeType)> {
        changes
            .iter()
            .map(|c| (c.resource.fs_path(), c.change_type))
            .collect()
    }

    fn path(p: &str) -> String {
        Uri::file(p).fs_path()
    }

    /// Port of `simple add/update/delete`.
    #[test]
    fn simple_add_update_delete_are_all_kept() {
        let result = coalesce_events(vec![
            change("/users/data/src/added.txt", FileChangeType::Added),
            change("/users/data/src/updated.txt", FileChangeType::Updated),
            change("/users/data/src/deleted.txt", FileChangeType::Deleted),
        ]);
        assert_eq!(result.len(), 3);
        assert!(result.contains(&change("/users/data/src/added.txt", FileChangeType::Added)));
        assert!(result.contains(&change(
            "/users/data/src/updated.txt",
            FileChangeType::Updated
        )));
        assert!(result.contains(&change(
            "/users/data/src/deleted.txt",
            FileChangeType::Deleted
        )));
    }

    /// Port of `event coalescer: ignore CREATE followed by DELETE`.
    #[test]
    fn ignore_create_followed_by_delete() {
        let result = coalesce_events(vec![
            change("/users/data/src/related", FileChangeType::Added),
            change("/users/data/src/related", FileChangeType::Deleted),
            change("/users/data/src/unrelated", FileChangeType::Updated),
        ]);
        assert_eq!(
            summary(&result),
            vec![(path("/users/data/src/unrelated"), FileChangeType::Updated)]
        );
    }

    /// Port of `event coalescer: flatten DELETE followed by CREATE into CHANGE`.
    #[test]
    fn flatten_delete_followed_by_create_into_change() {
        let result = coalesce_events(vec![
            change("/users/data/src/related", FileChangeType::Deleted),
            change("/users/data/src/related", FileChangeType::Added),
            change("/users/data/src/unrelated", FileChangeType::Updated),
        ]);
        assert_eq!(result.len(), 2);
        assert!(result.contains(&change("/users/data/src/related", FileChangeType::Updated)));
        assert!(result.contains(&change(
            "/users/data/src/unrelated",
            FileChangeType::Updated
        )));
    }

    /// Port of `event coalescer: ignore UPDATE when CREATE received`.
    #[test]
    fn ignore_update_when_create_received() {
        let result = coalesce_events(vec![
            change("/users/data/src/related", FileChangeType::Added),
            change("/users/data/src/related", FileChangeType::Updated),
            change("/users/data/src/unrelated", FileChangeType::Updated),
        ]);
        assert_eq!(result.len(), 2);
        assert!(result.contains(&change("/users/data/src/related", FileChangeType::Added)));
        assert!(!result.contains(&change("/users/data/src/related", FileChangeType::Updated)));
        assert!(result.contains(&change(
            "/users/data/src/unrelated",
            FileChangeType::Updated
        )));
    }

    /// Port of `event coalescer: apply DELETE`: a later deletion replaces earlier updates.
    #[test]
    fn apply_delete_replaces_earlier_updates() {
        let result = coalesce_events(vec![
            change("/users/data/src/related", FileChangeType::Updated),
            change("/users/data/src/related", FileChangeType::Updated),
            change("/users/data/src/unrelated", FileChangeType::Updated),
            change("/users/data/src/related", FileChangeType::Deleted),
        ]);
        assert_eq!(result.len(), 2);
        assert!(result.contains(&change("/users/data/src/related", FileChangeType::Deleted)));
        assert!(!result.contains(&change("/users/data/src/related", FileChangeType::Updated)));
        assert!(result.contains(&change(
            "/users/data/src/unrelated",
            FileChangeType::Updated
        )));
    }

    /// Port of `event coalescer: track case renames`: on case-insensitive disks, a rename that
    /// only changes case keeps both events.
    #[test]
    fn track_case_renames() {
        let result = coalesce_events(vec![
            change("/users/data/src/ADDED", FileChangeType::Added),
            change("/users/data/src/added", FileChangeType::Deleted),
        ]);
        assert_eq!(result.len(), 2);
        assert!(result.contains(&change("/users/data/src/ADDED", FileChangeType::Added)));
        assert!(result.contains(&change("/users/data/src/added", FileChangeType::Deleted)));
    }

    /// Port of `delete only reported for top level folder`: deletions below a deleted folder are
    /// implied by it and dropped, while an unrelated file that shares a name prefix is kept.
    #[test]
    fn delete_only_reported_for_top_level_folder() {
        let root = "/users/data/src";
        let result = coalesce_events(vec![
            change(&format!("{root}/todelete1"), FileChangeType::Deleted),
            change(&format!("{root}/todelete2"), FileChangeType::Deleted),
            change(
                &format!("{root}/todelete2/file.txt"),
                FileChangeType::Deleted,
            ),
            change(
                &format!("{root}/todelete2/more/test.txt"),
                FileChangeType::Deleted,
            ),
            change(
                &format!("{root}/todelete2/super/bar/foo.txt"),
                FileChangeType::Deleted,
            ),
            change(&format!("{root}/deleteme.txt"), FileChangeType::Deleted),
            change(&format!("{root}/added.txt"), FileChangeType::Added),
            change(&format!("{root}/updated.txt"), FileChangeType::Updated),
        ]);
        assert_eq!(result.len(), 5);
        for expected in [
            change(&format!("{root}/todelete1"), FileChangeType::Deleted),
            change(&format!("{root}/todelete2"), FileChangeType::Deleted),
            change(&format!("{root}/deleteme.txt"), FileChangeType::Deleted),
            change(&format!("{root}/added.txt"), FileChangeType::Added),
            change(&format!("{root}/updated.txt"), FileChangeType::Updated),
        ] {
            assert!(result.contains(&expected), "missing {expected:?}");
        }
    }

    #[test]
    fn is_parent_requires_a_separator_boundary() {
        let sep = MAIN_SEPARATOR;
        assert!(is_parent(
            &format!("a{sep}b{sep}c"),
            &format!("a{sep}b"),
            false
        ));
        assert!(!is_parent(&format!("a{sep}bc"), &format!("a{sep}b"), false));
        assert!(!is_parent(&format!("a{sep}b"), &format!("a{sep}b"), false));
        assert!(is_parent(
            &format!("A{sep}B{sep}c"),
            &format!("a{sep}b"),
            true
        ));
        assert!(!is_parent(
            &format!("A{sep}B{sep}c"),
            &format!("a{sep}b"),
            false
        ));
    }
}
