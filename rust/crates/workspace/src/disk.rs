//! The `file` scheme: a [`FileSystemProvider`] over the local disk.
//!
//! Ported from `src/vs/platform/files/node/diskFileSystemProvider.ts`, with the path rules of
//! `src/vs/platform/files/common/diskFileSystemProvider.ts`. Blocking `std::fs` calls run on
//! tokio's blocking pool, so the UI thread never waits on disk (ADR 0005). Differences from
//! upstream, all deliberate:
//!
//! - No file-descriptor or stream API, because the contract does not have one. Reads and writes
//!   are whole-file.
//! - No trash. `use_trash` fails with `Unavailable`, and `TRASH` is not advertised.
//! - Directory listings are sorted by name. Upstream returns them in completion order.
//! - Creation time falls back to modification time where the platform does not report one.
//! - Recursive delete does not clear read-only entries first, as upstream does on Windows.
//! - Watching is not implemented yet; `watch` returns a no-op disposable.

use std::fs::{self, Metadata};
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use vscode_api::event::{Emitter, Event};
use vscode_api::files::{
    BoxFuture, FileChange, FileDeleteOptions, FileOverwriteOptions, FilePermission,
    FileReadOptions, FileResult, FileSystemError, FileSystemErrorCode, FileSystemProvider,
    FileSystemProviderCapabilities, FileType, FileWriteOptions, Stat, WatchOptions,
};
use vscode_api::lifecycle::Disposable;
use vscode_api::uri::Uri;

use crate::locks::ResourceLocks;

/// The `file` scheme provider. Holds the change emitter and the per-resource locks.
#[derive(Default)]
pub struct DiskFileSystemProvider {
    changes: Emitter<Vec<FileChange>>,
    locks: ResourceLocks,
}

impl DiskFileSystemProvider {
    /// Creates a provider with no watches and no locks held.
    pub fn new() -> Self {
        Self::default()
    }
}

impl FileSystemProvider for DiskFileSystemProvider {
    fn capabilities(&self) -> FileSystemProviderCapabilities {
        let base = FileSystemProviderCapabilities::FILE_ATOMIC_DELETE;
        if path_case_sensitive() {
            base | FileSystemProviderCapabilities::PATH_CASE_SENSITIVE
        } else {
            base
        }
    }

    fn on_did_change_file(&self) -> Event<Vec<FileChange>> {
        self.changes.event()
    }

    fn watch(&self, _resource: &Uri, _options: WatchOptions) -> Disposable {
        Disposable::none()
    }

    fn stat<'a>(&'a self, resource: &'a Uri) -> BoxFuture<'a, FileResult<Stat>> {
        Box::pin(async move {
            let path = to_path(resource)?;
            run_blocking(move || stat_path(&path)).await
        })
    }

    fn mkdir<'a>(&'a self, resource: &'a Uri) -> BoxFuture<'a, FileResult<()>> {
        Box::pin(async move {
            let path = to_path(resource)?;
            run_blocking(move || fs::create_dir(&path).map_err(io_error)).await
        })
    }

    fn read_dir<'a>(
        &'a self,
        resource: &'a Uri,
    ) -> BoxFuture<'a, FileResult<Vec<(String, FileType)>>> {
        Box::pin(async move {
            let path = to_path(resource)?;
            run_blocking(move || read_dir_path(&path)).await
        })
    }

    fn delete<'a>(
        &'a self,
        resource: &'a Uri,
        options: FileDeleteOptions,
    ) -> BoxFuture<'a, FileResult<()>> {
        Box::pin(async move {
            if options.use_trash {
                return Err(unavailable(format!(
                    "moving {resource} to the trash is not supported"
                )));
            }
            let path = to_path(resource)?;
            let postfix = atomic_postfix(options.atomic.as_ref()).map(str::to_owned);
            let recursive = options.recursive;
            run_blocking(move || delete_path(&path, recursive, postfix.as_deref())).await
        })
    }

    // Write, rename and copy follow in the next change of this crate. Until then the contract
    // defaults apply: the capability bits stay unset and the calls answer `Unavailable`.

    fn rename<'a>(
        &'a self,
        from: &'a Uri,
        to: &'a Uri,
        _options: FileOverwriteOptions,
    ) -> BoxFuture<'a, FileResult<()>> {
        Box::pin(async move {
            Err(unavailable(format!(
                "rename {from} to {to} is not implemented yet"
            )))
        })
    }

    fn read_file<'a>(
        &'a self,
        resource: &'a Uri,
        options: FileReadOptions,
    ) -> BoxFuture<'a, FileResult<Vec<u8>>> {
        Box::pin(async move {
            let path = to_path(resource)?;
            // Atomic reads wait for writes to the same resource; writes arrive in the next change.
            let _guard = if options.atomic {
                Some(self.locks.lock(lock_key(&path)).await)
            } else {
                None
            };
            run_blocking(move || read_path(&path)).await
        })
    }

    fn write_file<'a>(
        &'a self,
        resource: &'a Uri,
        _content: &'a [u8],
        _options: FileWriteOptions,
    ) -> BoxFuture<'a, FileResult<()>> {
        Box::pin(async move {
            Err(unavailable(format!(
                "writing {resource} is not implemented yet"
            )))
        })
    }
}

// Path mapping and blocking helpers.

fn path_case_sensitive() -> bool {
    cfg!(target_os = "linux")
}

fn to_path(resource: &Uri) -> FileResult<PathBuf> {
    if resource.scheme() != "file" {
        return Err(unavailable(format!("{resource} is not a file URI")));
    }
    Ok(PathBuf::from(resource.fs_path()))
}

/// Key for [`ResourceLocks`]. Windows paths compare case-insensitively.
fn lock_key(path: &Path) -> String {
    let key = path.to_string_lossy();
    if cfg!(windows) {
        key.to_lowercase()
    } else {
        key.into_owned()
    }
}

fn unavailable(message: impl Into<String>) -> FileSystemError {
    FileSystemError::new(FileSystemErrorCode::Unavailable, message)
}

/// The suffix of atomic operations, or `None` when empty (upstream checks `atomic.postfix`).
fn atomic_postfix(atomic: Option<&vscode_api::files::FileAtomicOptions>) -> Option<&str> {
    atomic.map(|a| a.postfix.as_str()).filter(|p| !p.is_empty())
}

/// Maps an I/O error to a provider error (`toFileSystemProviderError`).
fn io_error(err: io::Error) -> FileSystemError {
    let code = match err.kind() {
        io::ErrorKind::NotFound => FileSystemErrorCode::FileNotFound,
        io::ErrorKind::AlreadyExists => FileSystemErrorCode::FileExists,
        io::ErrorKind::IsADirectory => FileSystemErrorCode::FileIsADirectory,
        io::ErrorKind::NotADirectory => FileSystemErrorCode::FileNotADirectory,
        io::ErrorKind::PermissionDenied => FileSystemErrorCode::NoPermissions,
        _ => FileSystemErrorCode::Unknown,
    };
    FileSystemError::new(code, err.to_string())
}

/// Runs blocking file work on the blocking pool. A panic in the work is reported as `Unknown`
/// instead of unwinding into the caller (ADR 0005).
async fn run_blocking<T: Send + 'static>(
    work: impl FnOnce() -> FileResult<T> + Send + 'static,
) -> FileResult<T> {
    match tokio::task::spawn_blocking(work).await {
        Ok(result) => result,
        Err(err) => Err(FileSystemError::new(
            FileSystemErrorCode::Unknown,
            format!("file operation did not finish: {err}"),
        )),
    }
}

// Metadata.

#[cfg(unix)]
fn owner_writable(meta: &Metadata) -> bool {
    use std::os::unix::fs::PermissionsExt;
    meta.permissions().mode() & 0o200 != 0
}

#[cfg(not(unix))]
fn owner_writable(meta: &Metadata) -> bool {
    !meta.permissions().readonly()
}

#[cfg(unix)]
fn executable(meta: &Metadata) -> bool {
    use std::os::unix::fs::PermissionsExt;
    meta.permissions().mode() & 0o111 != 0
}

#[cfg(not(unix))]
fn executable(_meta: &Metadata) -> bool {
    // Windows has no executable bit, and upstream never reports one there.
    false
}

/// `FilePermission` bits as upstream derives them: `LOCKED` when the owner cannot write, and
/// `EXECUTABLE` when any execute bit is set. `None` when neither applies.
fn permissions_of(meta: &Metadata) -> Option<FilePermission> {
    let mut bits = FilePermission::default();
    if !owner_writable(meta) {
        bits = bits | FilePermission::LOCKED;
    }
    if executable(meta) {
        bits = bits | FilePermission::EXECUTABLE;
    }
    (bits != FilePermission::default()).then_some(bits)
}

/// Milliseconds since the Unix epoch, or `0` when the platform does not report the time.
fn to_millis(time: Option<SystemTime>) -> i64 {
    match time.map(|t| t.duration_since(UNIX_EPOCH)) {
        Some(Ok(since)) => i64::try_from(since.as_millis()).unwrap_or(i64::MAX),
        Some(Err(before)) => -i64::try_from(before.duration().as_millis()).unwrap_or(i64::MAX),
        None => 0,
    }
}

/// `stat` with symbolic links resolved (`SymlinkSupport.stat`). A link whose target is missing
/// is reported with the link's own metadata and `Unknown` type, so listings do not fail on it.
fn stat_path(path: &Path) -> FileResult<Stat> {
    let link_meta = fs::symlink_metadata(path).map_err(io_error)?;
    let is_link = link_meta.is_symlink();

    let (target_meta, dangling) = if is_link {
        match fs::metadata(path) {
            Ok(meta) => (Some(meta), false),
            Err(err) if err.kind() == io::ErrorKind::NotFound => (None, true),
            Err(err) => return Err(io_error(err)),
        }
    } else {
        (None, false)
    };
    let meta = target_meta.as_ref().unwrap_or(&link_meta);

    let mut file_type = if dangling {
        FileType::UNKNOWN
    } else if meta.is_file() {
        FileType::FILE
    } else if meta.is_dir() {
        FileType::DIRECTORY
    } else {
        FileType::UNKNOWN
    };
    if is_link {
        file_type = file_type | FileType::SYMBOLIC_LINK;
    }

    Ok(Stat {
        file_type,
        ctime: to_millis(meta.created().ok().or_else(|| meta.modified().ok())),
        mtime: to_millis(meta.modified().ok()),
        size: meta.len(),
        permissions: permissions_of(meta),
    })
}

/// Entries of a directory with their types. Entries that cannot be read are skipped, as
/// upstream does for permission errors on single children.
fn read_dir_path(path: &Path) -> FileResult<Vec<(String, FileType)>> {
    let mut entries = Vec::new();
    for entry in fs::read_dir(path).map_err(io_error)? {
        let Ok(entry) = entry else { continue };
        let child = entry.path();
        let file_type = match entry.file_type() {
            Ok(kind) if kind.is_symlink() => match stat_path(&child) {
                Ok(stat) => stat.file_type,
                Err(_) => continue,
            },
            Ok(kind) if kind.is_dir() => FileType::DIRECTORY,
            Ok(kind) if kind.is_file() => FileType::FILE,
            _ => FileType::UNKNOWN,
        };
        entries.push((entry.file_name().to_string_lossy().into_owned(), file_type));
    }
    entries.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(entries)
}

/// Whole-file read (`readFile`). Directories and non-regular files are rejected before opening,
/// because opening a directory fails on Windows and opening a FIFO can block.
fn read_path(path: &Path) -> FileResult<Vec<u8>> {
    let meta = fs::metadata(path).map_err(io_error)?;
    if meta.is_dir() {
        return Err(FileSystemError::new(
            FileSystemErrorCode::FileIsADirectory,
            format!("{} is a directory", path.display()),
        ));
    }
    if !meta.is_file() {
        return Err(unavailable(format!(
            "{} is not a regular file",
            path.display()
        )));
    }
    fs::read(path).map_err(io_error)
}

/// `delete`. With `recursive` and a non-empty `postfix`, the entry is first renamed to a sibling
/// with that suffix and only then removed, so a half-removed tree is never visible at `path`.
/// Without `recursive`, only files and empty folders are removed.
fn delete_path(path: &Path, recursive: bool, postfix: Option<&str>) -> FileResult<()> {
    if recursive {
        let target = match postfix {
            Some(postfix) => {
                let temp = sibling(path, postfix);
                fs::rename(path, &temp).map_err(io_error)?;
                temp
            }
            None => path.to_path_buf(),
        };
        return remove_recursive(&target).map_err(io_error);
    }

    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(err) => {
            // `remove_file` fails on folders. Only then try `remove_dir`, which fails again
            // unless the folder is empty (upstream: `unlink` then `rmdir`).
            if fs::symlink_metadata(path).is_ok_and(|meta| meta.is_dir()) {
                fs::remove_dir(path).map_err(io_error)
            } else {
                Err(io_error(err))
            }
        }
    }
}

/// Removes a file, a link, or a folder tree. Links are removed, never followed.
fn remove_recursive(path: &Path) -> io::Result<()> {
    let meta = fs::symlink_metadata(path)?;
    if meta.is_dir() {
        return fs::remove_dir_all(path);
    }
    if cfg!(windows) && meta.is_symlink() && fs::metadata(path).is_ok_and(|m| m.is_dir()) {
        // Windows needs `remove_dir` for a link to a folder.
        return fs::remove_dir(path);
    }
    fs::remove_file(path)
}

/// The sibling of `path` whose name is the name of `path` followed by `postfix`.
fn sibling(path: &Path, postfix: &str) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(postfix);
    path.with_file_name(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sibling_appends_postfix_to_the_name() {
        let path = Path::new("dir").join("notes.txt");
        assert_eq!(
            sibling(&path, ".vsctmp"),
            Path::new("dir").join("notes.txt.vsctmp")
        );
    }

    #[test]
    fn millis_before_the_epoch_are_negative() {
        let before = UNIX_EPOCH - std::time::Duration::from_millis(1500);
        assert_eq!(to_millis(Some(before)), -1500);
        assert_eq!(to_millis(Some(UNIX_EPOCH)), 0);
        assert_eq!(to_millis(None), 0);
    }
}
