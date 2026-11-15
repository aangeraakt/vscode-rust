//! File system provider contract.
//!
//! Ported from `src/vs/platform/files/common/files.ts` (`IFileSystemProvider`,
//! `FileSystemProviderCapabilities`, `FileType`, `FilePermission`, `IStat`, `IFileChange`,
//! `FileChangeType`, `FileSystemProviderErrorCode`, the `I*Options` shapes).
//!
//! A provider serves one URI scheme (`file`, `untitled`, `vscode-userdata`, ...). Async methods
//! return boxed `Send` futures so providers run on the tokio I/O runtime without this crate
//! depending on tokio (ADR 0005). Optional TypeScript methods (`readFile`, `writeFile`, `copy`)
//! are required here or have defaults that answer [`FileSystemErrorCode::Unavailable`]; the
//! capability bits say which ones work. File-descriptor (`open`/`read`/`write`) and stream APIs
//! are not part of this contract yet; add them when a provider needs them.
//!
//! Build the option structs with `..Default::default()`: fields may be added to them.

use crate::event::Event;
use crate::lifecycle::Disposable;
use crate::uri::Uri;
use std::fmt;
use std::future::Future;
use std::pin::Pin;

/// A boxed, `Send` future, the return type of async contract methods.
pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// Result of a file system operation.
pub type FileResult<T> = Result<T, FileSystemError>;

macro_rules! bit_set {
    ($(#[$meta:meta])* $name:ident { $($(#[$m:meta])* $flag:ident = $value:expr,)* }) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
        pub struct $name(pub u32);

        impl $name {
            $($(#[$m])* pub const $flag: Self = Self($value);)*

            /// Whether every bit of `other` is set in `self`.
            pub fn contains(self, other: Self) -> bool {
                self.0 & other.0 == other.0
            }
        }

        impl std::ops::BitOr for $name {
            type Output = Self;
            fn bitor(self, rhs: Self) -> Self {
                Self(self.0 | rhs.0)
            }
        }
    };
}

bit_set!(
    /// The kind of a file system entry. `SYMBOLIC_LINK` combines with `FILE` or `DIRECTORY`.
    FileType {
        /// Unknown kind.
        UNKNOWN = 0,
        /// A regular file.
        FILE = 1,
        /// A directory.
        DIRECTORY = 2,
        /// A symbolic link (to a file or directory).
        SYMBOLIC_LINK = 64,
    }
);

bit_set!(
    /// Permissions of an entry, when the provider knows them.
    FilePermission {
        /// The entry cannot be written.
        READONLY = 1,
        /// The entry is locked but can be unlocked for writing.
        LOCKED = 2,
        /// The entry is executable.
        EXECUTABLE = 4,
    }
);

bit_set!(
    /// What a provider supports. Values match VS Code.
    FileSystemProviderCapabilities {
        /// No optional capability.
        NONE = 0,
        /// `read_file` / `write_file` work.
        FILE_READ_WRITE = 1 << 1,
        /// Open/read/write/close by descriptor (not in this contract yet).
        FILE_OPEN_READ_WRITE_CLOSE = 1 << 2,
        /// `copy` works for folders.
        FILE_FOLDER_COPY = 1 << 3,
        /// Streaming reads (not in this contract yet).
        FILE_READ_STREAM = 1 << 4,
        /// Paths are case-sensitive.
        PATH_CASE_SENSITIVE = 1 << 10,
        /// The whole provider is read-only.
        READONLY = 1 << 11,
        /// `delete` can move to the trash.
        TRASH = 1 << 12,
        /// Writes can unlock locked files.
        FILE_WRITE_UNLOCK = 1 << 13,
        /// Atomic reads.
        FILE_ATOMIC_READ = 1 << 14,
        /// Atomic writes.
        FILE_ATOMIC_WRITE = 1 << 15,
        /// Atomic deletes.
        FILE_ATOMIC_DELETE = 1 << 16,
        /// Copy-on-write cloning.
        FILE_CLONE = 1 << 17,
        /// Resolving real paths.
        FILE_REALPATH = 1 << 18,
        /// Appending writes.
        FILE_APPEND = 1 << 19,
        /// Renames that never delete the target first or fall back to copying.
        FILE_ATOMIC_RENAME = 1 << 20,
    }
);

/// Metadata of an entry (`IStat`). Times are milliseconds since the Unix epoch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Stat {
    /// The entry kind.
    pub file_type: FileType,
    /// Last modification time.
    pub mtime: i64,
    /// Creation time.
    pub ctime: i64,
    /// Size in bytes.
    pub size: u64,
    /// Permissions, if known.
    pub permissions: Option<FilePermission>,
}

/// How an entry changed. Numeric values match VS Code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FileChangeType {
    /// Contents or metadata changed.
    Updated = 0,
    /// The entry was created.
    Added = 1,
    /// The entry was deleted.
    Deleted = 2,
}

/// One change reported by a watcher (`IFileChange`).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FileChange {
    /// What happened.
    pub change_type: FileChangeType,
    /// The affected resource.
    pub resource: Uri,
    /// The correlation id of the watch request that produced it, if any.
    pub correlation_id: Option<u64>,
}

/// Why a provider operation failed (`FileSystemProviderErrorCode`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FileSystemErrorCode {
    /// The target already exists (`EntryExists`).
    FileExists,
    /// The target does not exist (`EntryNotFound`).
    FileNotFound,
    /// A directory was expected (`EntryNotADirectory`).
    FileNotADirectory,
    /// A file was expected (`EntryIsADirectory`).
    FileIsADirectory,
    /// The storage quota is exceeded (`EntryExceedsStorageQuota`).
    FileExceedsStorageQuota,
    /// The file is too large (`EntryTooLarge`).
    FileTooLarge,
    /// The file is write-locked (`EntryWriteLocked`).
    FileWriteLocked,
    /// Permission denied (`NoPermissions`).
    NoPermissions,
    /// The provider or operation is unavailable (`Unavailable`).
    Unavailable,
    /// Anything else (`Unknown`).
    Unknown,
}

/// A provider error: a code plus a human-readable message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileSystemError {
    /// The error code.
    pub code: FileSystemErrorCode,
    /// A description, typically including the resource.
    pub message: String,
}

impl FileSystemError {
    /// Creates an error.
    pub fn new(code: FileSystemErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

impl fmt::Display for FileSystemError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} ({:?})", self.message, self.code)
    }
}

impl std::error::Error for FileSystemError {}

/// Options for `watch` (`IWatchOptions`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct WatchOptions {
    /// Watch the whole subtree, not just direct children.
    pub recursive: bool,
    /// Glob patterns to exclude.
    pub excludes: Vec<String>,
    /// Glob patterns to include; empty means everything.
    pub includes: Vec<String>,
    /// Correlates reported changes with this request.
    pub correlation_id: Option<u64>,
}

/// How to perform an atomic write or delete (`IFileAtomicOptions`): through a temporary
/// sibling named `<name><postfix>`, e.g. `foo.txt.vsctmp`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FileAtomicOptions {
    /// Suffix of the temporary file.
    pub postfix: String,
}

/// Options for `read_file` (`IFileAtomicReadOptions`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FileReadOptions {
    /// Read atomically, if the provider has `FILE_ATOMIC_READ`.
    pub atomic: bool,
}

/// Options for `write_file` (`IFileWriteOptions`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FileWriteOptions {
    /// Create the file if it does not exist.
    pub create: bool,
    /// Replace an existing file.
    pub overwrite: bool,
    /// Unlock a write-locked file first.
    pub unlock: bool,
    /// Append instead of replacing, if the provider has `FILE_APPEND`.
    pub append: bool,
    /// Write atomically through a temporary file, if the provider has `FILE_ATOMIC_WRITE`.
    pub atomic: Option<FileAtomicOptions>,
}

/// Options for `delete` (`IFileDeleteOptions`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FileDeleteOptions {
    /// Delete directories with their contents.
    pub recursive: bool,
    /// Move to the trash instead of deleting.
    pub use_trash: bool,
    /// Delete atomically, if the provider has `FILE_ATOMIC_DELETE`.
    pub atomic: Option<FileAtomicOptions>,
}

/// Options for `rename` and `copy` (`IFileOverwriteOptions`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FileOverwriteOptions {
    /// Replace an existing target.
    pub overwrite: bool,
}

/// A file system for one URI scheme (`IFileSystemProvider`).
pub trait FileSystemProvider: Send + Sync {
    /// What the provider supports.
    fn capabilities(&self) -> FileSystemProviderCapabilities;

    /// Batches of changes from active watches.
    fn on_did_change_file(&self) -> Event<Vec<FileChange>>;

    /// Starts watching `resource`; disposing the result stops it.
    fn watch(&self, resource: &Uri, options: WatchOptions) -> Disposable;

    /// Metadata of `resource`.
    fn stat<'a>(&'a self, resource: &'a Uri) -> BoxFuture<'a, FileResult<Stat>>;

    /// Creates a directory. The parent must exist.
    fn mkdir<'a>(&'a self, resource: &'a Uri) -> BoxFuture<'a, FileResult<()>>;

    /// Entries of a directory as `(name, type)` pairs.
    fn read_dir<'a>(
        &'a self,
        resource: &'a Uri,
    ) -> BoxFuture<'a, FileResult<Vec<(String, FileType)>>>;

    /// Deletes `resource`.
    fn delete<'a>(
        &'a self,
        resource: &'a Uri,
        options: FileDeleteOptions,
    ) -> BoxFuture<'a, FileResult<()>>;

    /// Renames or moves `from` to `to`.
    fn rename<'a>(
        &'a self,
        from: &'a Uri,
        to: &'a Uri,
        options: FileOverwriteOptions,
    ) -> BoxFuture<'a, FileResult<()>>;

    /// Reads the whole file. Requires [`FileSystemProviderCapabilities::FILE_READ_WRITE`].
    fn read_file<'a>(
        &'a self,
        resource: &'a Uri,
        options: FileReadOptions,
    ) -> BoxFuture<'a, FileResult<Vec<u8>>>;

    /// Writes the whole file. Requires [`FileSystemProviderCapabilities::FILE_READ_WRITE`].
    fn write_file<'a>(
        &'a self,
        resource: &'a Uri,
        content: &'a [u8],
        options: FileWriteOptions,
    ) -> BoxFuture<'a, FileResult<()>>;

    /// Copies `from` to `to`. Providers without
    /// [`FileSystemProviderCapabilities::FILE_FOLDER_COPY`] keep this default, and callers
    /// fall back to read + write.
    fn copy<'a>(
        &'a self,
        from: &'a Uri,
        to: &'a Uri,
        options: FileOverwriteOptions,
    ) -> BoxFuture<'a, FileResult<()>> {
        let _ = (to, options);
        Box::pin(async move {
            Err(FileSystemError::new(
                FileSystemErrorCode::Unavailable,
                format!("copy is not supported for {from}"),
            ))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::Emitter;
    use std::collections::BTreeMap;
    use std::sync::Mutex;
    use std::task::{Context, Poll, Waker};

    fn block_on<F: Future>(fut: F) -> F::Output {
        let mut fut = std::pin::pin!(fut);
        let mut cx = Context::from_waker(Waker::noop());
        loop {
            if let Poll::Ready(v) = fut.as_mut().poll(&mut cx) {
                return v;
            }
        }
    }

    /// Minimal in-memory provider proving the trait is implementable and object-safe.
    struct MemFs {
        files: Mutex<BTreeMap<String, Vec<u8>>>,
        changes: Emitter<Vec<FileChange>>,
    }

    fn not_found(uri: &Uri) -> FileSystemError {
        FileSystemError::new(FileSystemErrorCode::FileNotFound, uri.to_string())
    }

    impl FileSystemProvider for MemFs {
        fn capabilities(&self) -> FileSystemProviderCapabilities {
            FileSystemProviderCapabilities::FILE_READ_WRITE
                | FileSystemProviderCapabilities::PATH_CASE_SENSITIVE
        }
        fn on_did_change_file(&self) -> Event<Vec<FileChange>> {
            self.changes.event()
        }
        fn watch(&self, _: &Uri, _: WatchOptions) -> Disposable {
            Disposable::none()
        }
        fn stat<'a>(&'a self, r: &'a Uri) -> BoxFuture<'a, FileResult<Stat>> {
            Box::pin(async move {
                let files = self.files.lock().unwrap();
                let data = files.get(r.path()).ok_or_else(|| not_found(r))?;
                Ok(Stat {
                    file_type: FileType::FILE,
                    mtime: 0,
                    ctime: 0,
                    size: data.len() as u64,
                    permissions: None,
                })
            })
        }
        fn mkdir<'a>(&'a self, _: &'a Uri) -> BoxFuture<'a, FileResult<()>> {
            Box::pin(async { Ok(()) })
        }
        fn read_dir<'a>(
            &'a self,
            _: &'a Uri,
        ) -> BoxFuture<'a, FileResult<Vec<(String, FileType)>>> {
            Box::pin(async move {
                let files = self.files.lock().unwrap();
                Ok(files
                    .keys()
                    .map(|k| (k[1..].to_owned(), FileType::FILE))
                    .collect())
            })
        }
        fn delete<'a>(&'a self, r: &'a Uri, _: FileDeleteOptions) -> BoxFuture<'a, FileResult<()>> {
            Box::pin(async move {
                self.files
                    .lock()
                    .unwrap()
                    .remove(r.path())
                    .ok_or_else(|| not_found(r))?;
                self.changes.fire(vec![FileChange {
                    change_type: FileChangeType::Deleted,
                    resource: r.clone(),
                    correlation_id: None,
                }]);
                Ok(())
            })
        }
        fn rename<'a>(
            &'a self,
            f: &'a Uri,
            t: &'a Uri,
            _: FileOverwriteOptions,
        ) -> BoxFuture<'a, FileResult<()>> {
            Box::pin(async move {
                let mut files = self.files.lock().unwrap();
                let data = files.remove(f.path()).ok_or_else(|| not_found(f))?;
                files.insert(t.path().to_owned(), data);
                Ok(())
            })
        }
        fn read_file<'a>(
            &'a self,
            r: &'a Uri,
            _: FileReadOptions,
        ) -> BoxFuture<'a, FileResult<Vec<u8>>> {
            Box::pin(async move {
                self.files
                    .lock()
                    .unwrap()
                    .get(r.path())
                    .cloned()
                    .ok_or_else(|| not_found(r))
            })
        }
        fn write_file<'a>(
            &'a self,
            r: &'a Uri,
            c: &'a [u8],
            o: FileWriteOptions,
        ) -> BoxFuture<'a, FileResult<()>> {
            Box::pin(async move {
                let mut files = self.files.lock().unwrap();
                let exists = files.contains_key(r.path());
                if exists && !o.overwrite {
                    return Err(FileSystemError::new(
                        FileSystemErrorCode::FileExists,
                        r.to_string(),
                    ));
                }
                if !exists && !o.create {
                    return Err(not_found(r));
                }
                files.insert(r.path().to_owned(), c.to_vec());
                Ok(())
            })
        }
    }

    fn mem() -> Box<dyn FileSystemProvider> {
        Box::new(MemFs {
            files: Mutex::new(BTreeMap::new()),
            changes: Emitter::new(),
        })
    }

    #[test]
    fn write_read_stat_rename_delete_through_trait_object() {
        let fs = mem();
        let a = Uri::parse("mem:/a.txt").unwrap();
        let b = Uri::parse("mem:/b.txt").unwrap();
        let create = FileWriteOptions {
            create: true,
            ..Default::default()
        };
        block_on(fs.write_file(&a, b"hi", create.clone())).unwrap();
        assert_eq!(
            block_on(fs.read_file(&a, FileReadOptions::default())).unwrap(),
            b"hi"
        );
        assert_eq!(block_on(fs.stat(&a)).unwrap().size, 2);
        let err = block_on(fs.write_file(&a, b"x", create)).unwrap_err();
        assert_eq!(err.code, FileSystemErrorCode::FileExists);
        block_on(fs.rename(&a, &b, FileOverwriteOptions::default())).unwrap();
        assert_eq!(
            block_on(fs.read_dir(&Uri::parse("mem:/").unwrap())).unwrap(),
            [("b.txt".to_owned(), FileType::FILE)]
        );

        let seen = std::sync::Arc::new(Mutex::new(Vec::new()));
        let s = seen.clone();
        let _l = fs
            .on_did_change_file()
            .subscribe(move |c| s.lock().unwrap().extend(c.clone()));
        block_on(fs.delete(&b, FileDeleteOptions::default())).unwrap();
        assert_eq!(seen.lock().unwrap()[0].change_type, FileChangeType::Deleted);
        assert_eq!(
            block_on(fs.stat(&b)).unwrap_err().code,
            FileSystemErrorCode::FileNotFound
        );
    }

    #[test]
    fn copy_defaults_to_unavailable() {
        let fs = mem();
        let a = Uri::parse("mem:/a").unwrap();
        let err = block_on(fs.copy(&a, &a, FileOverwriteOptions::default())).unwrap_err();
        assert_eq!(err.code, FileSystemErrorCode::Unavailable);
    }

    #[test]
    fn bit_values_match_vscode() {
        let link_to_dir = FileType::SYMBOLIC_LINK | FileType::DIRECTORY;
        assert_eq!(link_to_dir.0, 66);
        assert!(link_to_dir.contains(FileType::DIRECTORY));
        assert!(!link_to_dir.contains(FileType::FILE));
        assert_eq!(FileSystemProviderCapabilities::FILE_APPEND.0, 524_288);
        assert_eq!(
            FileSystemProviderCapabilities::FILE_ATOMIC_RENAME.0,
            1_048_576
        );
        assert_eq!(FileChangeType::Deleted as i32, 2);
    }
}
