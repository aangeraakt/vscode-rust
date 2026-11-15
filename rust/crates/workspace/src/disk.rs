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
//! - Writes are not retried after a failed write, as upstream does three times.
//! - Atomic writes check the target (existence, `create`, `overwrite`, folder) before the
//!   temporary file is written. Upstream skips these checks for atomic writes, so it would
//!   replace an existing file even when `overwrite` is false.
//! - Watching is not implemented yet; `watch` returns a no-op disposable.

use std::fs::{self, File, Metadata, OpenOptions, Permissions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

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
        let base = FileSystemProviderCapabilities::FILE_READ_WRITE
            | FileSystemProviderCapabilities::FILE_FOLDER_COPY
            | FileSystemProviderCapabilities::FILE_WRITE_UNLOCK
            | FileSystemProviderCapabilities::FILE_APPEND
            | FileSystemProviderCapabilities::FILE_ATOMIC_READ
            | FileSystemProviderCapabilities::FILE_ATOMIC_WRITE
            | FileSystemProviderCapabilities::FILE_ATOMIC_DELETE;
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

    fn rename<'a>(
        &'a self,
        from: &'a Uri,
        to: &'a Uri,
        options: FileOverwriteOptions,
    ) -> BoxFuture<'a, FileResult<()>> {
        Box::pin(async move {
            let (from, to) = (to_path(from)?, to_path(to)?);
            run_blocking(move || rename_path(&from, &to, options.overwrite)).await
        })
    }

    fn copy<'a>(
        &'a self,
        from: &'a Uri,
        to: &'a Uri,
        options: FileOverwriteOptions,
    ) -> BoxFuture<'a, FileResult<()>> {
        Box::pin(async move {
            let (from, to) = (to_path(from)?, to_path(to)?);
            run_blocking(move || copy_path(&from, &to, options.overwrite)).await
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
        content: &'a [u8],
        options: FileWriteOptions,
    ) -> BoxFuture<'a, FileResult<()>> {
        Box::pin(async move {
            let path = to_path(resource)?;
            let temp =
                atomic_postfix(options.atomic.as_ref()).map(|postfix| sibling(&path, postfix));
            // The target is locked before the temporary file, as upstream does.
            let _target = self.locks.lock(lock_key(&path)).await;
            let _temp = match &temp {
                Some(temp) => Some(self.locks.lock(lock_key(temp)).await),
                None => None,
            };
            let content = content.to_vec();
            run_blocking(move || write_path(&path, temp.as_deref(), &content, &options)).await
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

// Writes.

/// Whole-file write (`writeFile`). The target is validated for every write mode. Atomic writes
/// go through `temp` unless the target is a link, as upstream refuses atomic writes to links.
fn write_path(
    path: &Path,
    temp: Option<&Path>,
    content: &[u8],
    options: &FileWriteOptions,
) -> FileResult<()> {
    check_target(path, options)?;
    match temp {
        Some(temp) if !is_symlink(path) => write_atomic(path, temp, content, options),
        _ => write_direct(path, content, options),
    }
}

fn check_target(path: &Path, options: &FileWriteOptions) -> FileResult<()> {
    match fs::metadata(path) {
        Ok(meta) if meta.is_dir() => Err(FileSystemError::new(
            FileSystemErrorCode::FileIsADirectory,
            format!("{} is a directory", path.display()),
        )),
        Ok(_) if !options.overwrite => Err(FileSystemError::new(
            FileSystemErrorCode::FileExists,
            "File already exists",
        )),
        Ok(_) => Ok(()),
        Err(err) if err.kind() == io::ErrorKind::NotFound && options.create => Ok(()),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Err(FileSystemError::new(
            FileSystemErrorCode::FileNotFound,
            "File does not exist",
        )),
        Err(err) => Err(io_error(err)),
    }
}

fn write_direct(path: &Path, content: &[u8], options: &FileWriteOptions) -> FileResult<()> {
    if options.unlock {
        unlock(path);
    }
    let mut file = open_for_write(path, options.append).map_err(|err| write_error(path, err))?;
    file.write_all(content)
        .map_err(|err| write_error(path, err))?;
    // Upstream flushes a written file on close and only logs a failure, so a failed flush is
    // not an error here either.
    let _ = file.sync_data();
    Ok(())
}

/// Writes `temp` next to `path`, then renames it over `path`. Readers see the old or the new
/// contents, never a partial file. A failed write or rename removes the temporary file.
fn write_atomic(
    path: &Path,
    temp: &Path,
    content: &[u8],
    options: &FileWriteOptions,
) -> FileResult<()> {
    let temp_options = FileWriteOptions {
        create: true,
        overwrite: true,
        atomic: None,
        ..options.clone()
    };
    let written =
        write_direct(temp, content, &temp_options).and_then(|()| rename_path(temp, path, true));
    if written.is_err() {
        let _ = fs::remove_file(temp);
    }
    written
}

/// Opens `path` for a whole-file write, truncating it, or for appending.
fn open_for_write(path: &Path, append: bool) -> io::Result<File> {
    if append {
        return OpenOptions::new().append(true).create(true).open(path);
    }
    #[cfg(windows)]
    {
        // Upstream opens with "r+" and truncates first, because "w" (CREATE_ALWAYS) fails on
        // hidden and system files. Opening an existing file keeps those working.
        if let Ok(file) = OpenOptions::new().write(true).open(path) {
            if file.set_len(0).is_ok() {
                return Ok(file);
            }
        }
    }
    OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(path)
}

/// Makes a read-only file writable by its owner (`unlock`). A failure is ignored, and the write
/// then reports its own error, as upstream does.
fn unlock(path: &Path) {
    if let Ok(meta) = fs::metadata(path) {
        if !owner_writable(&meta) {
            let _ = fs::set_permissions(path, with_owner_writable(meta.permissions()));
        }
    }
}

#[cfg(unix)]
fn with_owner_writable(mut perms: Permissions) -> Permissions {
    use std::os::unix::fs::PermissionsExt;
    let mode = perms.mode();
    perms.set_mode(mode | 0o200);
    perms
}

#[cfg(not(unix))]
#[allow(clippy::permissions_set_readonly_false)] // that lint is about world-writable Unix modes
fn with_owner_writable(mut perms: Permissions) -> Permissions {
    perms.set_readonly(false);
    perms
}

/// A write error on a read-only file is reported as `FILE_WRITE_LOCKED`, which tells the caller
/// the file can be unlocked (`toFileSystemProviderWriteError`).
fn write_error(path: &Path, err: io::Error) -> FileSystemError {
    let mut mapped = io_error(err);
    if mapped.code == FileSystemErrorCode::NoPermissions
        && fs::metadata(path).is_ok_and(|meta| !owner_writable(&meta))
    {
        mapped.code = FileSystemErrorCode::FileWriteLocked;
    }
    mapped
}

fn is_symlink(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok_and(|meta| meta.is_symlink())
}

// Rename and copy.

#[derive(Clone, Copy, PartialEq, Eq)]
enum TransferMode {
    Move,
    Copy,
}

/// `stat` that follows links, falling back to the link itself when its target is missing.
fn stat_ignore_error(path: &Path) -> Option<Metadata> {
    fs::metadata(path)
        .or_else(|_| fs::symlink_metadata(path))
        .ok()
}

/// Checks a rename or copy, and removes the target when it must be replaced
/// (`validateMoveCopy`). Returns whether the source is a folder.
fn validate_transfer(
    from: &Path,
    to: &Path,
    mode: TransferMode,
    overwrite: bool,
) -> FileResult<bool> {
    let from_meta = stat_ignore_error(from).ok_or_else(|| {
        FileSystemError::new(
            FileSystemErrorCode::FileNotFound,
            "File to move/copy does not exist",
        )
    })?;

    if !path_case_sensitive() && eq_ignore_case(from, to) {
        // The same file under another case on a case-insensitive disk. A move renames it; a copy
        // would overwrite itself.
        return match mode {
            TransferMode::Move => Ok(from_meta.is_dir()),
            TransferMode::Copy => Err(FileSystemError::new(
                FileSystemErrorCode::FileExists,
                "File cannot be copied to same path with different path case",
            )),
        };
    }

    if mode == TransferMode::Copy && from_meta.is_dir() && to.starts_with(from) {
        return Err(FileSystemError::new(
            FileSystemErrorCode::Unknown,
            format!("cannot copy {} into itself", from.display()),
        ));
    }

    if let Some(to_meta) = stat_ignore_error(to) {
        if !overwrite {
            return Err(FileSystemError::new(
                FileSystemErrorCode::FileExists,
                "File at target already exists and thus will not be moved/copied to unless overwrite is specified",
            ));
        }
        // A file can replace a file directly. Anything else is removed first.
        if !(from_meta.is_file() && to_meta.is_file()) {
            remove_recursive(to).map_err(io_error)?;
        }
    }
    Ok(from_meta.is_dir())
}

fn eq_ignore_case(a: &Path, b: &Path) -> bool {
    a.to_string_lossy().to_lowercase() == b.to_string_lossy().to_lowercase()
}

/// Renames `from` to `to`. Same paths are a no-op, as upstream simulates.
fn rename_path(from: &Path, to: &Path, overwrite: bool) -> FileResult<()> {
    if from == to {
        return Ok(());
    }
    let source_is_dir = validate_transfer(from, to, TransferMode::Move, overwrite)?;
    // On Windows, antivirus software briefly locks files that were just written, so a rename is
    // retried for a while. A folder is only retried briefly when nothing gets replaced.
    let timeout = if !cfg!(windows) {
        Duration::ZERO
    } else if source_is_dir && !overwrite {
        Duration::from_secs(1)
    } else {
        Duration::from_secs(60)
    };
    rename_with_retry(from, to, timeout).map_err(io_error)
}

fn rename_with_retry(from: &Path, to: &Path, timeout: Duration) -> io::Result<()> {
    let deadline = Instant::now() + timeout;
    loop {
        match fs::rename(from, to) {
            Err(err) if is_busy(&err) && Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(10));
            }
            result => return result,
        }
    }
}

/// A rename refused because another program holds the entry. Only Windows reports these.
fn is_busy(err: &io::Error) -> bool {
    cfg!(windows)
        && (err.kind() == io::ErrorKind::PermissionDenied
            || matches!(err.raw_os_error(), Some(32 | 33)))
}

/// Copies `from` to `to`. Same paths are a no-op, as upstream simulates.
fn copy_path(from: &Path, to: &Path, overwrite: bool) -> FileResult<()> {
    if from == to {
        return Ok(());
    }
    validate_transfer(from, to, TransferMode::Copy, overwrite)?;
    copy_entry(from, to).map_err(io_error)
}

/// Copies a file, a folder tree, or a link. Links are recreated as links, as upstream's
/// `preserveSymlinks` does.
fn copy_entry(from: &Path, to: &Path) -> io::Result<()> {
    let meta = fs::symlink_metadata(from)?;
    if meta.is_symlink() {
        let target = fs::read_link(from)?;
        let target_is_dir = fs::metadata(from).is_ok_and(|m| m.is_dir());
        return create_symlink(&target, to, target_is_dir);
    }
    if meta.is_dir() {
        fs::create_dir(to)?;
        for entry in fs::read_dir(from)? {
            let entry = entry?;
            copy_entry(&entry.path(), &to.join(entry.file_name()))?;
        }
        return Ok(());
    }
    fs::copy(from, to).map(|_| ())
}

#[cfg(unix)]
fn create_symlink(target: &Path, link: &Path, _target_is_dir: bool) -> io::Result<()> {
    std::os::unix::fs::symlink(target, link)
}

#[cfg(windows)]
fn create_symlink(target: &Path, link: &Path, target_is_dir: bool) -> io::Result<()> {
    if target_is_dir {
        std::os::windows::fs::symlink_dir(target, link)
    } else {
        std::os::windows::fs::symlink_file(target, link)
    }
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
