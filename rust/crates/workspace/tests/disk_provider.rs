//! Provider-level tests for [`lathe_workspace::DiskFileSystemProvider`].
//!
//! Each test names the upstream case it ports from
//! `src/vs/platform/files/test/node/diskFileService.integrationTest.ts`. Cases that go through the
//! file service (events, `canDelete` and friends) or the file-descriptor API are not ported here.

use std::fs;
use std::path::{Path, PathBuf};

use lathe_workspace::DiskFileSystemProvider;
use tempfile::TempDir;
use vscode_api::files::{
    FileAtomicOptions, FileDeleteOptions, FilePermission, FileReadOptions, FileSystemErrorCode,
    FileSystemProvider, FileType, Stat,
};
use vscode_api::uri::Uri;

struct Fixture {
    dir: TempDir,
    fs: DiskFileSystemProvider,
}

impl Fixture {
    fn new() -> Self {
        let dir = tempfile::tempdir().expect("temp dir");
        // A small tree that most cases start from: a file, a folder with files, and an empty folder.
        fs::write(dir.path().join("lorem.txt"), "Lorem ipsum").unwrap();
        fs::create_dir_all(dir.path().join("deep").join("nested")).unwrap();
        fs::write(dir.path().join("deep").join("conway.js"), "// life").unwrap();
        fs::write(dir.path().join("deep").join("nested").join("a.txt"), "a").unwrap();
        fs::create_dir(dir.path().join("empty")).unwrap();
        Self {
            dir,
            fs: DiskFileSystemProvider::new(),
        }
    }

    fn path(&self, relative: &str) -> PathBuf {
        self.dir.path().join(relative)
    }

    fn uri(&self, relative: &str) -> Uri {
        uri_of(&self.path(relative))
    }
}

fn uri_of(path: &Path) -> Uri {
    Uri::file(&path.to_string_lossy())
}

fn block_on<F: std::future::Future>(future: F) -> F::Output {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime")
        .block_on(future)
}

fn stat(fixture: &Fixture, relative: &str) -> Stat {
    block_on(fixture.fs.stat(&fixture.uri(relative))).expect("stat succeeds")
}

fn error_code<T: std::fmt::Debug>(
    result: Result<T, vscode_api::files::FileSystemError>,
) -> FileSystemErrorCode {
    result.expect_err("operation fails").code
}

// stat

/// Port of `stat - file`.
#[test]
fn stat_file() {
    let fixture = Fixture::new();
    let stat = stat(&fixture, "lorem.txt");
    assert_eq!(stat.file_type, FileType::FILE);
    assert_eq!(stat.size, "Lorem ipsum".len() as u64);
    assert!(stat.mtime > 0);
    assert!(stat.ctime > 0);
    assert_eq!(stat.permissions, None);
}

/// Port of `stat - directory`.
#[test]
fn stat_directory() {
    let fixture = Fixture::new();
    let stat = stat(&fixture, "deep");
    assert_eq!(stat.file_type, FileType::DIRECTORY);
    assert!(stat.mtime > 0);
    assert!(stat.ctime > 0);
}

/// A read-only file reports `LOCKED`, which is how upstream reports a non-writable entry.
#[test]
#[allow(clippy::permissions_set_readonly_false)] // restores the file so the temp dir can go
fn stat_read_only_file_is_locked() {
    let fixture = Fixture::new();
    let path = fixture.path("lorem.txt");
    let mut perms = fs::metadata(&path).unwrap().permissions();
    perms.set_readonly(true);
    fs::set_permissions(&path, perms).unwrap();

    let result = stat(&fixture, "lorem.txt").permissions;
    assert_eq!(result, Some(FilePermission::LOCKED));

    // Restore so the temp dir can be removed on Windows.
    let mut perms = fs::metadata(&path).unwrap().permissions();
    perms.set_readonly(false);
    fs::set_permissions(&path, perms).unwrap();
}

/// Port of `stat - executable`.
#[cfg(unix)]
#[test]
fn stat_executable_bit_is_reported() {
    use std::os::unix::fs::PermissionsExt;
    let fixture = Fixture::new();
    fs::set_permissions(fixture.path("lorem.txt"), fs::Permissions::from_mode(0o755)).unwrap();
    fs::set_permissions(
        fixture.path("deep/conway.js"),
        fs::Permissions::from_mode(0o644),
    )
    .unwrap();

    assert_eq!(
        stat(&fixture, "lorem.txt").permissions,
        Some(FilePermission::EXECUTABLE)
    );
    assert_eq!(stat(&fixture, "deep/conway.js").permissions, None);
}

#[test]
fn stat_missing_path_is_not_found() {
    let fixture = Fixture::new();
    let code = error_code(block_on(fixture.fs.stat(&fixture.uri("404.html"))));
    assert_eq!(code, FileSystemErrorCode::FileNotFound);
}

/// Port of `resolve / realpath - folder symbolic link`, the stat part: a link to a folder is a
/// directory and a symbolic link at once.
#[cfg(unix)]
#[test]
fn stat_symlink_to_folder_is_directory_and_link() {
    let fixture = Fixture::new();
    std::os::unix::fs::symlink(fixture.path("deep"), fixture.path("deep-link")).unwrap();
    let stat = stat(&fixture, "deep-link");
    assert_eq!(
        stat.file_type,
        FileType::DIRECTORY | FileType::SYMBOLIC_LINK
    );
}

/// Port of `resolve - file symbolic link` (Windows cannot create file links without privileges,
/// as upstream skips it there).
#[cfg(unix)]
#[test]
fn stat_symlink_to_file_is_file_and_link() {
    let fixture = Fixture::new();
    std::os::unix::fs::symlink(fixture.path("lorem.txt"), fixture.path("lorem.txt-linked"))
        .unwrap();
    assert_eq!(
        stat(&fixture, "lorem.txt-linked").file_type,
        FileType::FILE | FileType::SYMBOLIC_LINK
    );
}

// readdir

/// Port of the children part of `resolve - directory`: names and types, sorted here.
#[test]
fn read_dir_lists_children_with_types() {
    let fixture = Fixture::new();
    let entries = block_on(fixture.fs.read_dir(&fixture.uri("deep"))).unwrap();
    assert_eq!(
        entries,
        vec![
            ("conway.js".to_owned(), FileType::FILE),
            ("nested".to_owned(), FileType::DIRECTORY),
        ]
    );
}

/// Port of `resolve - symbolic link pointing to nonexistent file does not break`: a dangling
/// link is listed as `UNKNOWN` plus `SYMBOLIC_LINK`, and does not fail the listing.
#[cfg(unix)]
#[test]
fn read_dir_lists_dangling_link_as_unknown_link() {
    let fixture = Fixture::new();
    std::os::unix::fs::symlink(fixture.path("foo"), fixture.path("bar")).unwrap();
    let entries = block_on(fixture.fs.read_dir(&fixture.uri(""))).unwrap();
    assert!(entries.contains(&(
        "bar".to_owned(),
        FileType::UNKNOWN | FileType::SYMBOLIC_LINK
    )));
}

/// Windows reports a different error for this, as upstream skips it there.
#[cfg(unix)]
#[test]
fn read_dir_of_a_file_is_not_a_directory() {
    let fixture = Fixture::new();
    let code = error_code(block_on(fixture.fs.read_dir(&fixture.uri("lorem.txt"))));
    assert_eq!(code, FileSystemErrorCode::FileNotADirectory);
}

// readFile

/// Port of `readFile - small file - default`.
#[test]
fn read_file_returns_contents() {
    let fixture = Fixture::new();
    let bytes = block_on(
        fixture
            .fs
            .read_file(&fixture.uri("lorem.txt"), FileReadOptions::default()),
    )
    .unwrap();
    assert_eq!(bytes, b"Lorem ipsum");
}

/// Port of `readFile - atomic (natively supported)`: an atomic read returns the same bytes.
#[test]
fn read_file_atomic_returns_contents() {
    let fixture = Fixture::new();
    let bytes = block_on(
        fixture
            .fs
            .read_file(&fixture.uri("lorem.txt"), FileReadOptions { atomic: true }),
    )
    .unwrap();
    assert_eq!(bytes, b"Lorem ipsum");
}

/// Port of `readFile - FILE_IS_DIRECTORY`.
#[test]
fn read_file_of_a_folder_is_file_is_directory() {
    let fixture = Fixture::new();
    let code = error_code(block_on(
        fixture
            .fs
            .read_file(&fixture.uri("deep"), FileReadOptions::default()),
    ));
    assert_eq!(code, FileSystemErrorCode::FileIsADirectory);
}

/// Port of `readFile - FILE_NOT_FOUND`.
#[test]
fn read_file_missing_is_not_found() {
    let fixture = Fixture::new();
    let code = error_code(block_on(
        fixture
            .fs
            .read_file(&fixture.uri("404.html"), FileReadOptions::default()),
    ));
    assert_eq!(code, FileSystemErrorCode::FileNotFound);
}

/// Port of `readFile - FILE_NOT_DIRECTORY` (Windows reports a different error, as upstream skips).
#[cfg(unix)]
#[test]
fn read_file_below_a_file_is_not_a_directory() {
    let fixture = Fixture::new();
    let code = error_code(block_on(fixture.fs.read_file(
        &fixture.uri("lorem.txt/file.txt"),
        FileReadOptions::default(),
    )));
    assert_eq!(code, FileSystemErrorCode::FileNotADirectory);
}

// mkdir

/// Port of `createFolder`.
#[test]
fn mkdir_creates_a_folder() {
    let fixture = Fixture::new();
    block_on(fixture.fs.mkdir(&fixture.uri("created"))).unwrap();
    assert_eq!(stat(&fixture, "created").file_type, FileType::DIRECTORY);
}

/// The contract's `mkdir` needs the parent to exist, as upstream's non-recursive `mkdir` does.
#[test]
fn mkdir_without_parent_fails() {
    let fixture = Fixture::new();
    let code = error_code(block_on(fixture.fs.mkdir(&fixture.uri("missing/child"))));
    assert_eq!(code, FileSystemErrorCode::FileNotFound);
}

#[test]
fn mkdir_existing_folder_is_file_exists() {
    let fixture = Fixture::new();
    let code = error_code(block_on(fixture.fs.mkdir(&fixture.uri("deep"))));
    assert_eq!(code, FileSystemErrorCode::FileExists);
}

// delete

fn delete(
    fixture: &Fixture,
    relative: &str,
    recursive: bool,
) -> Result<(), vscode_api::files::FileSystemError> {
    block_on(fixture.fs.delete(
        &fixture.uri(relative),
        FileDeleteOptions {
            recursive,
            ..Default::default()
        },
    ))
}

/// Port of `deleteFile (non recursive)`, and the second delete that expects `FILE_NOT_FOUND`.
#[test]
fn delete_file_non_recursive() {
    let fixture = Fixture::new();
    delete(&fixture, "deep/conway.js", false).unwrap();
    assert!(!fixture.path("deep/conway.js").exists());
    assert_eq!(
        error_code(delete(&fixture, "deep/conway.js", false)),
        FileSystemErrorCode::FileNotFound
    );
}

/// Port of `deleteFile (recursive)`.
#[test]
fn delete_file_recursive() {
    let fixture = Fixture::new();
    delete(&fixture, "deep/conway.js", true).unwrap();
    assert!(!fixture.path("deep/conway.js").exists());
}

/// Trash is not supported (`TRASH` is not advertised), so `use_trash` must not delete anything.
#[test]
fn delete_with_use_trash_is_unavailable_and_keeps_the_file() {
    let fixture = Fixture::new();
    let result = block_on(fixture.fs.delete(
        &fixture.uri("deep/conway.js"),
        FileDeleteOptions {
            use_trash: true,
            ..Default::default()
        },
    ));
    assert_eq!(error_code(result), FileSystemErrorCode::Unavailable);
    assert!(fixture.path("deep/conway.js").exists());
}

/// Port of `deleteFolder (recursive)`.
#[test]
fn delete_folder_recursive() {
    let fixture = Fixture::new();
    delete(&fixture, "deep", true).unwrap();
    assert!(!fixture.path("deep").exists());
}

/// Port of `deleteFolder (recursive, atomic)`: the folder is gone and no temporary sibling is left.
#[test]
fn delete_folder_recursive_atomic_leaves_no_temporary() {
    let fixture = Fixture::new();
    block_on(fixture.fs.delete(
        &fixture.uri("deep"),
        FileDeleteOptions {
            recursive: true,
            atomic: Some(FileAtomicOptions {
                postfix: ".vsctmp".to_owned(),
            }),
            ..Default::default()
        },
    ))
    .unwrap();
    assert!(!fixture.path("deep").exists());
    assert!(!fixture.path("deep.vsctmp").exists());
}

/// Port of `deleteFolder (non recursive)`: a folder with children is not removed.
#[test]
fn delete_folder_non_recursive_fails_when_not_empty() {
    let fixture = Fixture::new();
    assert!(delete(&fixture, "deep", false).is_err());
    assert!(fixture.path("deep/conway.js").exists());
}

/// Port of `deleteFolder empty folder (recursive)`.
#[test]
fn delete_empty_folder_recursive() {
    let fixture = Fixture::new();
    delete(&fixture, "empty", true).unwrap();
    assert!(!fixture.path("empty").exists());
}

/// Port of `deleteFolder empty folder (non recursive)`.
#[test]
fn delete_empty_folder_non_recursive() {
    let fixture = Fixture::new();
    delete(&fixture, "empty", false).unwrap();
    assert!(!fixture.path("empty").exists());
}

/// Port of `deleteFile - symbolic link (exists)`: the link goes, the target stays.
#[cfg(unix)]
#[test]
fn delete_symlink_keeps_its_target() {
    let fixture = Fixture::new();
    std::os::unix::fs::symlink(fixture.path("lorem.txt"), fixture.path("lorem.txt-linked"))
        .unwrap();
    delete(&fixture, "lorem.txt-linked", false).unwrap();
    assert!(!fixture.path("lorem.txt-linked").exists());
    assert!(fixture.path("lorem.txt").exists());
}

/// Port of `deleteFile - symbolic link (pointing to nonexistent file)`.
#[cfg(unix)]
#[test]
fn delete_dangling_symlink() {
    let fixture = Fixture::new();
    std::os::unix::fs::symlink(fixture.path("foo"), fixture.path("bar")).unwrap();
    delete(&fixture, "bar", false).unwrap();
    assert!(fs::symlink_metadata(fixture.path("bar")).is_err());
}
