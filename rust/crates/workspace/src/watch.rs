//! File watching with `notify`, for the `file` scheme.
//!
//! Follows the request semantics of `src/vs/platform/files/node/watcher/` and
//! `src/vs/platform/files/common/watcher.ts`:
//!
//! - A non-recursive watch reports the watched folder and its direct children. A recursive
//!   watch reports everything below it.
//! - `excludes` drop matching paths. `includes`, when not empty, keep only matching paths.
//!   Relative patterns are matched below the watched folder, as `normalizeWatcherPattern` does.
//!   Patterns use `*`, `?`, `[...]` and `**` and never match across a separator unless `**` is
//!   used; they are case-insensitive off Linux.
//! - Every change carries the request's correlation id.
//! - Raw events are batched for a short window and coalesced (see [`crate::coalesce`]) before a
//!   listener sees them.
//!
//! Each watch owns one batching thread. It exits when the watch is disposed and its queue has
//! drained, so no thread outlives its watch. A thread per watch keeps `watch` synchronous, which
//! the contract needs: callers may start a watch from the UI thread, where no runtime is running.
//!
//! Rename events carry no direction. A path that still exists is reported as added, and one that
//! does not as deleted, as upstream does after a `stat`.

use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::time::{Duration, Instant};

use globset::{GlobBuilder, GlobSet, GlobSetBuilder};
use notify::event::ModifyKind;
use notify::{EventKind, RecursiveMode, Watcher};
use vscode_api::event::Emitter;
use vscode_api::files::{FileChange, FileChangeType, WatchOptions};
use vscode_api::lifecycle::Disposable;
use vscode_api::uri::Uri;

use crate::coalesce::coalesce_events;

/// How long raw events are collected before they are coalesced and reported.
const BATCH_WINDOW: Duration = Duration::from_millis(50);

/// Why a watch could not start. The payloads are not read yet: the contract has no error channel
/// for watches (`on_did_watch_error`), so the provider drops them.
#[derive(Debug)]
#[allow(dead_code)]
pub(crate) enum WatchError {
    /// An include or exclude pattern is not a valid glob.
    Pattern(globset::Error),
    /// `notify` could not watch the path.
    Notify(notify::Error),
    /// The batching thread could not start.
    Thread(io::Error),
}

impl From<globset::Error> for WatchError {
    fn from(err: globset::Error) -> Self {
        Self::Pattern(err)
    }
}

impl From<notify::Error> for WatchError {
    fn from(err: notify::Error) -> Self {
        Self::Notify(err)
    }
}

/// Starts watching `root` for `options`. Changes are fired on `emitter` after coalescing.
/// Disposing the returned value stops the watch.
pub(crate) fn start(
    root: PathBuf,
    options: WatchOptions,
    emitter: Arc<Emitter<Vec<FileChange>>>,
) -> Result<Disposable, WatchError> {
    // A trailing separator would double up when relative patterns are joined to the root.
    let root: PathBuf = root.components().collect();
    let filter = PathFilter::new(&root, &options.includes, &options.excludes)?;
    let (sender, receiver) = mpsc::channel::<FileChange>();
    let active = Arc::new(AtomicBool::new(true));

    let thread_active = active.clone();
    std::thread::Builder::new()
        .name("lathe-file-watch".to_owned())
        .spawn(move || batch_loop(&receiver, &emitter, &thread_active))
        .map_err(WatchError::Thread)?;

    let recursive = options.recursive;
    let correlation_id = options.correlation_id;
    let handler_root = root.clone();
    let mut watcher = notify::recommended_watcher(move |event| {
        // Errors reported by the backend are dropped: the contract has no error channel yet.
        if let Ok(event) = event {
            report(
                &event,
                &handler_root,
                recursive,
                &filter,
                correlation_id,
                &sender,
            );
        }
    })?;
    let mode = if recursive {
        RecursiveMode::Recursive
    } else {
        RecursiveMode::NonRecursive
    };
    watcher.watch(&root, mode)?;

    Ok(Disposable::new(move || {
        active.store(false, Ordering::SeqCst);
        // Dropping the watcher drops the event sender, which ends the batching thread.
        drop(watcher);
    }))
}

/// Collects raw changes for one window, then coalesces and fires them. Ends when the sender side
/// is gone and the queue is empty.
fn batch_loop(
    receiver: &Receiver<FileChange>,
    emitter: &Emitter<Vec<FileChange>>,
    active: &AtomicBool,
) {
    while let Ok(first) = receiver.recv() {
        let mut batch = vec![first];
        let deadline = Instant::now() + BATCH_WINDOW;
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            match receiver.recv_timeout(remaining) {
                Ok(change) => batch.push(change),
                Err(RecvTimeoutError::Timeout | RecvTimeoutError::Disconnected) => break,
            }
        }
        let coalesced = coalesce_events(batch);
        if !coalesced.is_empty() && active.load(Ordering::SeqCst) {
            emitter.fire(coalesced);
        }
    }
}

/// Turns one backend event into queued changes for the paths in scope and not filtered out.
fn report(
    event: &notify::Event,
    root: &Path,
    recursive: bool,
    filter: &PathFilter,
    correlation_id: Option<u64>,
    sender: &Sender<FileChange>,
) {
    for path in &event.paths {
        let Some(change_type) = change_type(&event.kind, path) else {
            continue;
        };
        if !in_scope(path, root, recursive) || !filter.allows(path) {
            continue;
        }
        let _ = sender.send(FileChange {
            change_type,
            resource: Uri::file(&path.to_string_lossy()),
            correlation_id,
        });
    }
}

/// The change a backend event means for `path`, or `None` for events that change nothing
/// (reads).
fn change_type(kind: &EventKind, path: &Path) -> Option<FileChangeType> {
    let exists = path.exists();
    match kind {
        EventKind::Create(_) => Some(FileChangeType::Added),
        EventKind::Remove(_) => Some(FileChangeType::Deleted),
        // A rename reports the old and the new path without a direction; existence decides.
        EventKind::Modify(ModifyKind::Name(_)) => Some(if exists {
            FileChangeType::Added
        } else {
            FileChangeType::Deleted
        }),
        EventKind::Modify(_) | EventKind::Any | EventKind::Other => Some(if exists {
            FileChangeType::Updated
        } else {
            FileChangeType::Deleted
        }),
        EventKind::Access(_) => None,
    }
}

/// Whether `path` is reported for a watch on `root`.
fn in_scope(path: &Path, root: &Path, recursive: bool) -> bool {
    if path == root {
        return true;
    }
    if recursive {
        path.starts_with(root)
    } else {
        path.parent() == Some(root)
    }
}

/// The include and exclude patterns of one watch, compiled.
struct PathFilter {
    includes: Option<GlobSet>,
    excludes: Option<GlobSet>,
}

impl PathFilter {
    fn new(root: &Path, includes: &[String], excludes: &[String]) -> Result<Self, globset::Error> {
        Ok(Self {
            includes: compile(root, includes)?,
            excludes: compile(root, excludes)?,
        })
    }

    fn allows(&self, path: &Path) -> bool {
        let text = slashed(path);
        if self
            .excludes
            .as_ref()
            .is_some_and(|set| set.is_match(&text))
        {
            return false;
        }
        self.includes.as_ref().is_none_or(|set| set.is_match(&text))
    }
}

/// Compiles patterns into one set, or `None` when there are none.
fn compile(root: &Path, patterns: &[String]) -> Result<Option<GlobSet>, globset::Error> {
    if patterns.is_empty() {
        return Ok(None);
    }
    let mut builder = GlobSetBuilder::new();
    for pattern in patterns {
        builder.add(
            GlobBuilder::new(&normalize_pattern(root, pattern))
                .case_insensitive(!cfg!(target_os = "linux"))
                .literal_separator(true)
                .build()?,
        );
    }
    builder.build().map(Some)
}

/// A relative pattern is matched below `root`, unless it starts with `**`. Absolute patterns are
/// used as they are (`normalizeWatcherPattern`).
fn normalize_pattern(root: &Path, pattern: &str) -> String {
    let pattern = pattern.replace('\\', "/");
    if pattern.starts_with("**") || pattern.starts_with('/') || Path::new(&pattern).is_absolute() {
        pattern
    } else {
        format!("{}/{pattern}", slashed(root))
    }
}

/// A path with `/` separators, the form patterns are matched against.
fn slashed(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relative_pattern_is_matched_below_the_root() {
        assert_eq!(
            normalize_pattern(Path::new("/data/src"), "skip/**"),
            "/data/src/skip/**"
        );
    }

    #[test]
    fn globstar_and_absolute_patterns_are_used_as_they_are() {
        assert_eq!(
            normalize_pattern(Path::new("/data"), "**/node_modules/**"),
            "**/node_modules/**"
        );
        assert_eq!(
            normalize_pattern(Path::new("/data"), "/other/*.txt"),
            "/other/*.txt"
        );
    }

    #[test]
    fn backslashes_in_patterns_become_slashes() {
        assert_eq!(
            normalize_pattern(Path::new(r"c:\data"), r"x\*.txt"),
            "c:/data/x/*.txt"
        );
    }

    #[test]
    fn excludes_and_includes_match_as_documented() {
        let root = Path::new("/data/src");
        let filter =
            PathFilter::new(root, &["keep*.txt".to_owned()], &["skip/**".to_owned()]).unwrap();
        assert!(filter.allows(Path::new("/data/src/keep1.txt")));
        assert!(!filter.allows(Path::new("/data/src/other.txt")));
        assert!(!filter.allows(Path::new("/data/src/skip/keep1.txt")));
    }

    #[test]
    fn in_scope_depends_on_recursive() {
        let root = Path::new("/data/src");
        assert!(in_scope(Path::new("/data/src"), root, false));
        assert!(in_scope(Path::new("/data/src/a.txt"), root, false));
        assert!(!in_scope(Path::new("/data/src/deep/a.txt"), root, false));
        assert!(in_scope(Path::new("/data/src/deep/a.txt"), root, true));
        assert!(!in_scope(Path::new("/data/other/a.txt"), root, true));
    }

    #[test]
    fn change_type_reads_renames_by_existence() {
        use notify::event::{AccessKind, CreateKind, ModifyKind, RemoveKind, RenameMode};
        let missing = Path::new("/definitely/not/here/xyz");
        assert_eq!(
            change_type(&EventKind::Create(CreateKind::Any), missing),
            Some(FileChangeType::Added)
        );
        assert_eq!(
            change_type(&EventKind::Remove(RemoveKind::Any), missing),
            Some(FileChangeType::Deleted)
        );
        assert_eq!(
            change_type(
                &EventKind::Modify(ModifyKind::Name(RenameMode::From)),
                missing
            ),
            Some(FileChangeType::Deleted)
        );
        assert_eq!(
            change_type(
                &EventKind::Modify(ModifyKind::Name(RenameMode::To)),
                Path::new(env!("CARGO_MANIFEST_DIR"))
            ),
            Some(FileChangeType::Added)
        );
        assert_eq!(
            change_type(&EventKind::Access(AccessKind::Any), missing),
            None
        );
    }
}
