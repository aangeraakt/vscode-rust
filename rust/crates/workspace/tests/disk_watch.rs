//! Watch tests for [`lathe_workspace::DiskFileSystemProvider`], on real folders.
//!
//! Upstream reference: `src/vs/platform/files/test/node/nodejsWatcher.test.ts` and
//! `src/vs/platform/files/test/node/parcelWatcher.test.ts` cover the same request semantics on
//! their backends; this file checks them through the contract. Every wait is bounded, and a test
//! that expects nothing waits a fixed short time, so the tests do not depend on backend timing
//! beyond that. Events are matched on path suffixes, because temporary folders on Windows may use
//! short names.

mod common;

use std::fs;
use std::path::Path;
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::{Duration, Instant};

use common::*;
use vscode_api::files::{FileChange, FileChangeType, FileSystemProvider, WatchOptions};
use vscode_api::lifecycle::Disposable;

/// How long a test waits for an expected event. Backends deliver in tens of milliseconds; this is
/// only a ceiling that keeps a broken backend from hanging the suite.
const EXPECT_TIMEOUT: Duration = Duration::from_secs(10);
/// How long a test waits before changing files, so the backend has registered the watch.
const REGISTER_DELAY: Duration = Duration::from_millis(300);
/// How long a test waits to confirm that nothing else is reported.
const QUIET_PERIOD: Duration = Duration::from_millis(1000);

/// A running watch and the batches it has fired. Dropping it disposes both.
struct Watching {
    _subscription: Disposable,
    _watch: Disposable,
    batches: Receiver<Vec<FileChange>>,
}

fn watch(fixture: &Fixture, root: &str, options: WatchOptions) -> Watching {
    let (sender, batches) = mpsc::channel();
    let subscription = fixture
        .fs
        .on_did_change_file()
        .subscribe(move |batch: &Vec<FileChange>| {
            let _ = sender.send(batch.clone());
        });
    let watch = fixture.fs.watch(&fixture.uri(root), options);
    thread::sleep(REGISTER_DELAY);
    Watching {
        _subscription: subscription,
        _watch: watch,
        batches,
    }
}

/// Collects events until `done` is satisfied or the timeout passes.
fn collect_until(
    batches: &Receiver<Vec<FileChange>>,
    timeout: Duration,
    mut done: impl FnMut(&[FileChange]) -> bool,
) -> Vec<FileChange> {
    let deadline = Instant::now() + timeout;
    let mut seen = Vec::new();
    while !done(&seen) {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            break;
        }
        match batches.recv_timeout(remaining) {
            Ok(batch) => seen.extend(batch),
            Err(_) => break,
        }
    }
    seen
}

/// Whether an event of `change_type` was reported for a path ending in `relative`.
fn has(seen: &[FileChange], relative: &str, change_type: FileChangeType) -> bool {
    seen.iter().any(|change| {
        change.change_type == change_type
            && Path::new(&change.resource.fs_path()).ends_with(relative)
    })
}

/// Whether any event was reported for a path ending in `relative`.
fn mentions(seen: &[FileChange], relative: &str) -> bool {
    seen.iter()
        .any(|change| Path::new(&change.resource.fs_path()).ends_with(relative))
}

fn options(recursive: bool) -> WatchOptions {
    WatchOptions {
        recursive,
        ..Default::default()
    }
}

#[test]
fn created_file_is_reported_with_its_correlation_id() {
    let fixture = Fixture::new();
    let watching = watch(
        &fixture,
        "",
        WatchOptions {
            correlation_id: Some(7),
            ..options(false)
        },
    );
    fs::write(fixture.path("new.txt"), "hello").unwrap();

    let seen = collect_until(&watching.batches, EXPECT_TIMEOUT, |seen| {
        has(seen, "new.txt", FileChangeType::Added)
    });
    assert!(
        has(&seen, "new.txt", FileChangeType::Added),
        "events: {seen:?}"
    );
    assert!(
        seen.iter()
            .filter(|c| Path::new(&c.resource.fs_path()).ends_with("new.txt"))
            .all(|c| c.correlation_id == Some(7))
    );
}

#[test]
fn updated_file_is_reported() {
    let fixture = Fixture::new();
    let watching = watch(&fixture, "", options(false));
    fs::write(fixture.path("lorem.txt"), "changed").unwrap();

    let seen = collect_until(&watching.batches, EXPECT_TIMEOUT, |seen| {
        has(seen, "lorem.txt", FileChangeType::Updated)
    });
    assert!(
        has(&seen, "lorem.txt", FileChangeType::Updated),
        "events: {seen:?}"
    );
}

#[test]
fn deleted_file_is_reported() {
    let fixture = Fixture::new();
    let watching = watch(&fixture, "", options(false));
    fs::remove_file(fixture.path("lorem.txt")).unwrap();

    let seen = collect_until(&watching.batches, EXPECT_TIMEOUT, |seen| {
        has(seen, "lorem.txt", FileChangeType::Deleted)
    });
    assert!(
        has(&seen, "lorem.txt", FileChangeType::Deleted),
        "events: {seen:?}"
    );
}

/// A rename has no direction in the backend's event. The old path is reported as deleted and the
/// new path as added.
#[test]
fn renamed_file_is_reported_as_deleted_and_added() {
    let fixture = Fixture::new();
    let watching = watch(&fixture, "", options(false));
    fs::rename(fixture.path("lorem.txt"), fixture.path("moved.txt")).unwrap();

    let seen = collect_until(&watching.batches, EXPECT_TIMEOUT, |seen| {
        has(seen, "lorem.txt", FileChangeType::Deleted)
            && has(seen, "moved.txt", FileChangeType::Added)
    });
    assert!(
        has(&seen, "lorem.txt", FileChangeType::Deleted),
        "events: {seen:?}"
    );
    assert!(
        has(&seen, "moved.txt", FileChangeType::Added),
        "events: {seen:?}"
    );
}

/// A non-recursive watch on the root reports direct children only. A marker in the root is the
/// positive control: once it arrives, the backend is known to work, so the quiet period is
/// meaningful.
#[test]
fn non_recursive_watch_ignores_nested_changes() {
    let fixture = Fixture::new();
    let watching = watch(&fixture, "", options(false));
    fs::write(fixture.path("deep/nested/hidden.txt"), "nested").unwrap();
    fs::write(fixture.path("marker.txt"), "marker").unwrap();

    let seen = collect_until(&watching.batches, EXPECT_TIMEOUT, |seen| {
        mentions(seen, "marker.txt")
    });
    assert!(mentions(&seen, "marker.txt"), "events: {seen:?}");
    let rest = collect_until(&watching.batches, QUIET_PERIOD, |_| false);
    assert!(
        !mentions(&rest, "hidden.txt"),
        "nested change reported: {rest:?}"
    );
    assert!(
        !mentions(&seen, "hidden.txt"),
        "nested change reported: {seen:?}"
    );
}

#[test]
fn recursive_watch_reports_nested_changes() {
    let fixture = Fixture::new();
    let watching = watch(&fixture, "", options(true));
    fs::write(fixture.path("deep/nested/new.txt"), "nested").unwrap();

    let seen = collect_until(&watching.batches, EXPECT_TIMEOUT, |seen| {
        has(seen, "deep/nested/new.txt", FileChangeType::Added)
    });
    assert!(
        has(&seen, "deep/nested/new.txt", FileChangeType::Added),
        "events: {seen:?}"
    );
}

/// `excludes` drop matching paths. The marker written after the excluded change is the positive
/// control, and the excluded change is written first so it would arrive before the marker.
#[test]
fn excludes_drop_matching_paths() {
    let fixture = Fixture::new();
    fs::create_dir(fixture.path("skip")).unwrap();
    let watching = watch(
        &fixture,
        "",
        WatchOptions {
            excludes: vec!["skip/**".to_owned()],
            ..options(true)
        },
    );
    fs::write(fixture.path("skip/a.txt"), "skipped").unwrap();
    fs::write(fixture.path("keep.txt"), "kept").unwrap();

    let seen = collect_until(&watching.batches, EXPECT_TIMEOUT, |seen| {
        has(seen, "keep.txt", FileChangeType::Added)
    });
    assert!(
        has(&seen, "keep.txt", FileChangeType::Added),
        "events: {seen:?}"
    );
    let rest = collect_until(&watching.batches, QUIET_PERIOD, |_| false);
    assert!(
        !mentions(&seen, "a.txt") && !mentions(&rest, "a.txt"),
        "excluded: {seen:?} {rest:?}"
    );
}

/// `includes`, when not empty, keep only matching paths.
#[test]
fn includes_keep_only_matching_paths() {
    let fixture = Fixture::new();
    let watching = watch(
        &fixture,
        "",
        WatchOptions {
            includes: vec!["keep*.txt".to_owned()],
            ..options(false)
        },
    );
    fs::write(fixture.path("other.txt"), "other").unwrap();
    fs::write(fixture.path("keep1.txt"), "kept").unwrap();

    let seen = collect_until(&watching.batches, EXPECT_TIMEOUT, |seen| {
        has(seen, "keep1.txt", FileChangeType::Added)
    });
    assert!(
        has(&seen, "keep1.txt", FileChangeType::Added),
        "events: {seen:?}"
    );
    let rest = collect_until(&watching.batches, QUIET_PERIOD, |_| false);
    assert!(
        !mentions(&seen, "other.txt") && !mentions(&rest, "other.txt"),
        "included: {seen:?} {rest:?}"
    );
}

/// After disposal, changes are not reported. The quiet period is longer than a batch window, so
/// events still queued at disposal would show up in it.
#[test]
fn disposing_a_watch_stops_reporting() {
    let fixture = Fixture::new();
    let watching = watch(&fixture, "", options(false));
    let Watching {
        _subscription,
        _watch,
        batches,
    } = watching;
    drop(_watch);
    thread::sleep(REGISTER_DELAY);
    fs::write(fixture.path("after.txt"), "after").unwrap();

    let seen = collect_until(&batches, QUIET_PERIOD, |_| false);
    assert!(
        !mentions(&seen, "after.txt"),
        "reported after dispose: {seen:?}"
    );
}

/// A path that cannot be watched gives a no-op disposable, and does not panic.
#[test]
fn watching_a_missing_path_reports_nothing() {
    let fixture = Fixture::new();
    let watching = watch(&fixture, "missing-folder", options(true));
    fs::create_dir_all(fixture.path("missing-folder")).unwrap();
    fs::write(fixture.path("missing-folder/x.txt"), "x").unwrap();
    let seen = collect_until(&watching.batches, QUIET_PERIOD, |_| false);
    assert!(
        !mentions(&seen, "x.txt"),
        "reported without a watch: {seen:?}"
    );
}
