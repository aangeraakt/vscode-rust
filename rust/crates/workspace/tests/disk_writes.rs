//! Provider-level tests for the write side of [`lathe_workspace::DiskFileSystemProvider`]:
//! `write_file`, `rename` and `copy`.
//!
//! Each test names the upstream case it ports from
//! `src/vs/platform/files/test/node/diskFileService.integrationTest.ts`. Barrier and atomic-read
//! cases use the whole-file API, because the file-descriptor API is not in the contract.

mod common;

use common::*;
use vscode_api::files::{
    FileAtomicOptions, FileOverwriteOptions, FileReadOptions, FileSystemError, FileSystemErrorCode,
    FileSystemProvider, FileSystemProviderCapabilities, FileType, FileWriteOptions,
};

const TEMP_POSTFIX: &str = ".vsctmp";

fn create_or_replace() -> FileWriteOptions {
    FileWriteOptions {
        create: true,
        overwrite: true,
        ..Default::default()
    }
}

fn write(
    fixture: &Fixture,
    relative: &str,
    content: &[u8],
    options: FileWriteOptions,
) -> Result<(), FileSystemError> {
    block_on(
        fixture
            .fs
            .write_file(&fixture.uri(relative), content, options),
    )
}

fn read(fixture: &Fixture, relative: &str) -> Vec<u8> {
    block_on(
        fixture
            .fs
            .read_file(&fixture.uri(relative), FileReadOptions::default()),
    )
    .expect("read succeeds")
}

fn rename(fixture: &Fixture, from: &str, to: &str, overwrite: bool) -> Result<(), FileSystemError> {
    block_on(fixture.fs.rename(
        &fixture.uri(from),
        &fixture.uri(to),
        FileOverwriteOptions { overwrite },
    ))
}

fn copy(fixture: &Fixture, from: &str, to: &str, overwrite: bool) -> Result<(), FileSystemError> {
    block_on(fixture.fs.copy(
        &fixture.uri(from),
        &fixture.uri(to),
        FileOverwriteOptions { overwrite },
    ))
}

/// Sets or clears the read-only bit, which is what upstream reports as `LOCKED`.
#[allow(clippy::permissions_set_readonly_false)] // restores a fixture file; not world-writable
fn set_read_only(fixture: &Fixture, relative: &str, read_only: bool) {
    let path = fixture.path(relative);
    let mut perms = std::fs::metadata(&path).unwrap().permissions();
    perms.set_readonly(read_only);
    std::fs::set_permissions(&path, perms).unwrap();
}

fn error_of(result: Result<(), FileSystemError>) -> FileSystemErrorCode {
    result.expect_err("operation fails").code
}

// Capabilities

#[test]
fn capabilities_advertise_only_what_is_implemented() {
    let fixture = Fixture::new();
    let caps = fixture.fs.capabilities();
    for bit in [
        FileSystemProviderCapabilities::FILE_READ_WRITE,
        FileSystemProviderCapabilities::FILE_FOLDER_COPY,
        FileSystemProviderCapabilities::FILE_WRITE_UNLOCK,
        FileSystemProviderCapabilities::FILE_APPEND,
        FileSystemProviderCapabilities::FILE_ATOMIC_READ,
        FileSystemProviderCapabilities::FILE_ATOMIC_WRITE,
        FileSystemProviderCapabilities::FILE_ATOMIC_DELETE,
    ] {
        assert!(caps.contains(bit));
    }
    // Trash, descriptors, streams, clone and atomic rename are not implemented.
    for bit in [
        FileSystemProviderCapabilities::TRASH,
        FileSystemProviderCapabilities::FILE_OPEN_READ_WRITE_CLOSE,
        FileSystemProviderCapabilities::FILE_READ_STREAM,
        FileSystemProviderCapabilities::FILE_CLONE,
        FileSystemProviderCapabilities::FILE_ATOMIC_RENAME,
    ] {
        assert!(!caps.contains(bit));
    }
}

// writeFile

/// Port of `createFile` and `writeFile - default`.
#[test]
fn write_file_creates_and_reads_back() {
    let fixture = Fixture::new();
    write(&fixture, "new.txt", b"Hello World", create_or_replace()).unwrap();
    assert_eq!(read(&fixture, "new.txt"), b"Hello World");
}

#[test]
fn write_file_missing_without_create_is_not_found() {
    let fixture = Fixture::new();
    let options = FileWriteOptions {
        overwrite: true,
        ..Default::default()
    };
    assert_eq!(
        error_of(write(&fixture, "missing.txt", b"x", options)),
        FileSystemErrorCode::FileNotFound
    );
    assert!(!fixture.path("missing.txt").exists());
}

/// Port of `createFile (does not overwrite by default)`.
#[test]
fn write_file_does_not_overwrite_by_default() {
    let fixture = Fixture::new();
    let options = FileWriteOptions {
        create: true,
        ..Default::default()
    };
    assert_eq!(
        error_of(write(&fixture, "lorem.txt", b"Hello World", options)),
        FileSystemErrorCode::FileExists
    );
    assert_eq!(read(&fixture, "lorem.txt"), b"Lorem ipsum");
}

/// Port of `createFile (allows to overwrite existing)`.
#[test]
fn write_file_overwrites_when_asked() {
    let fixture = Fixture::new();
    write(&fixture, "lorem.txt", b"Hello World", create_or_replace()).unwrap();
    assert_eq!(read(&fixture, "lorem.txt"), b"Hello World");
}

/// The contract's `append` flag adds to the end of the file.
#[test]
fn write_file_append_adds_to_the_end() {
    let fixture = Fixture::new();
    let options = FileWriteOptions {
        create: true,
        overwrite: true,
        append: true,
        ..Default::default()
    };
    write(&fixture, "lorem.txt", b"!", options).unwrap();
    assert_eq!(read(&fixture, "lorem.txt"), b"Lorem ipsum!");
}

/// Port of `writeFile - default (atomic)`: the new contents replace the file, and the temporary
/// sibling is gone afterwards.
#[test]
fn write_file_atomic_replaces_and_leaves_no_temporary() {
    let fixture = Fixture::new();
    let options = FileWriteOptions {
        atomic: Some(FileAtomicOptions {
            postfix: TEMP_POSTFIX.to_owned(),
        }),
        ..create_or_replace()
    };
    write(&fixture, "lorem.txt", b"Updated", options).unwrap();
    assert_eq!(read(&fixture, "lorem.txt"), b"Updated");
    assert!(!fixture.path("lorem.txt.vsctmp").exists());
}

#[test]
fn write_file_atomic_creates_a_missing_target() {
    let fixture = Fixture::new();
    let options = FileWriteOptions {
        create: true,
        atomic: Some(FileAtomicOptions {
            postfix: TEMP_POSTFIX.to_owned(),
        }),
        ..Default::default()
    };
    write(&fixture, "fresh.txt", b"fresh", options).unwrap();
    assert_eq!(read(&fixture, "fresh.txt"), b"fresh");
    assert!(!fixture.path("fresh.txt.vsctmp").exists());
}

/// Upstream would replace the file here, because it skips the overwrite check for atomic writes.
/// This crate refuses, and writes no temporary file.
#[test]
fn write_file_atomic_refuses_existing_target_without_overwrite() {
    let fixture = Fixture::new();
    let options = FileWriteOptions {
        create: true,
        atomic: Some(FileAtomicOptions {
            postfix: TEMP_POSTFIX.to_owned(),
        }),
        ..Default::default()
    };
    assert_eq!(
        error_of(write(&fixture, "lorem.txt", b"Changed", options)),
        FileSystemErrorCode::FileExists
    );
    assert_eq!(read(&fixture, "lorem.txt"), b"Lorem ipsum");
    assert!(!fixture.path("lorem.txt.vsctmp").exists());
}

/// Port of `writeFile - unbuffered - readonly throws`: the error says the file is write locked.
#[test]
fn write_file_read_only_target_is_write_locked() {
    let fixture = Fixture::new();
    set_read_only(&fixture, "lorem.txt", true);
    let result = write(&fixture, "lorem.txt", b"Updates", create_or_replace());
    set_read_only(&fixture, "lorem.txt", false);
    assert_eq!(error_of(result), FileSystemErrorCode::FileWriteLocked);
    assert_eq!(read(&fixture, "lorem.txt"), b"Lorem ipsum");
}

/// `unlock` makes a read-only file writable by its owner, then writes it.
#[test]
fn write_file_unlock_writes_a_read_only_file() {
    let fixture = Fixture::new();
    set_read_only(&fixture, "lorem.txt", true);
    let options = FileWriteOptions {
        create: true,
        overwrite: true,
        unlock: true,
        ..Default::default()
    };
    write(&fixture, "lorem.txt", b"Updates", options).unwrap();
    assert_eq!(read(&fixture, "lorem.txt"), b"Updates");
}

/// A write to a folder path is `FILE_IS_DIRECTORY`, and the folder keeps its contents.
#[test]
fn write_file_to_a_folder_is_file_is_directory() {
    let fixture = Fixture::new();
    assert_eq!(
        error_of(write(&fixture, "deep", b"x", create_or_replace())),
        FileSystemErrorCode::FileIsADirectory
    );
    assert!(fixture.path("deep/conway.js").exists());
}

/// An atomic write to a folder path must not remove the folder. A rename over a folder would
/// remove it, which is why the target is checked first.
#[test]
fn write_file_atomic_to_a_folder_keeps_the_folder() {
    let fixture = Fixture::new();
    let options = FileWriteOptions {
        atomic: Some(FileAtomicOptions {
            postfix: TEMP_POSTFIX.to_owned(),
        }),
        ..create_or_replace()
    };
    assert_eq!(
        error_of(write(&fixture, "deep", b"x", options)),
        FileSystemErrorCode::FileIsADirectory
    );
    assert!(fixture.path("deep/conway.js").exists());
    assert!(!fixture.path("deep.vsctmp").exists());
}

/// Atomic writes do not apply to links, so writing through a link changes the target and keeps
/// the link (upstream `canWriteFileAtomic`).
#[cfg(unix)]
#[test]
fn write_file_atomic_through_a_symlink_writes_the_target() {
    let fixture = Fixture::new();
    std::os::unix::fs::symlink(fixture.path("lorem.txt"), fixture.path("link.txt")).unwrap();
    let options = FileWriteOptions {
        atomic: Some(FileAtomicOptions {
            postfix: TEMP_POSTFIX.to_owned(),
        }),
        ..create_or_replace()
    };
    write(&fixture, "link.txt", b"Through link", options).unwrap();
    assert_eq!(read(&fixture, "lorem.txt"), b"Through link");
    assert!(
        std::fs::symlink_metadata(fixture.path("link.txt"))
            .unwrap()
            .is_symlink()
    );
}

// Concurrency: the write barrier and atomic reads. These tests check outcomes that must hold for
// any interleaving; they do not force one.

/// Port of `provider - write barrier prevents dirty writes`, on the whole-file API: concurrent
/// writes to one file never interleave, so the file ends up with exactly one writer's contents.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn write_barrier_serializes_concurrent_writes() {
    let fixture = Fixture::new();
    let uri = fixture.uri("lorem.txt");
    let mut tasks = Vec::new();
    for digit in 0..5u8 {
        let fs = fixture.fs.clone();
        let uri = uri.clone();
        let content = vec![b'0' + digit; 256 * 1024];
        tasks.push(tokio::spawn(async move {
            fs.write_file(&uri, &content, create_or_replace())
                .await
                .unwrap();
            content
        }));
    }
    let mut written = Vec::new();
    for task in tasks {
        written.push(task.await.unwrap());
    }
    let final_bytes = fixture
        .fs
        .read_file(&uri, FileReadOptions::default())
        .await
        .unwrap();
    assert!(written.contains(&final_bytes));
}

/// Port of `provider - write barrier is partitioned per resource`: writes to different files do
/// not wait for each other, and both land intact.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn write_barrier_is_partitioned_per_resource() {
    let fixture = Fixture::new();
    let first = fixture.uri("lorem.txt");
    let second = fixture.uri("deep/conway.js");
    let (a, b) = tokio::join!(
        fixture.fs.write_file(&first, b"first", create_or_replace()),
        fixture
            .fs
            .write_file(&second, b"second", create_or_replace()),
    );
    a.unwrap();
    b.unwrap();
    let read_async = |relative: &'static str| {
        let fs = fixture.fs.clone();
        let uri = fixture.uri(relative);
        async move {
            fs.read_file(&uri, FileReadOptions::default())
                .await
                .unwrap()
        }
    };
    assert_eq!(read_async("lorem.txt").await, b"first");
    assert_eq!(read_async("deep/conway.js").await, b"second");
}

/// Port of `provider - atomic reads (write pending when read starts)` and `(read pending when
/// write starts)`, in one test: an atomic read returns either the old or the new contents, never
/// a mix. The read may or may not overlap the write, so the test does not depend on the order.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn atomic_read_sees_only_complete_writes() {
    let fixture = Fixture::new();
    let uri = fixture.uri("lorem.txt");
    let old = vec![b'o'; 512 * 1024];
    let new = vec![b'n'; 1024 * 1024];
    fixture
        .fs
        .write_file(&uri, &old, create_or_replace())
        .await
        .unwrap();

    for _ in 0..20 {
        let fs = fixture.fs.clone();
        let writer_uri = uri.clone();
        let writer_bytes = new.clone();
        let writer = tokio::spawn(async move {
            fs.write_file(&writer_uri, &writer_bytes, create_or_replace())
                .await
                .unwrap();
        });
        let read = fixture
            .fs
            .read_file(&uri, FileReadOptions { atomic: true })
            .await
            .unwrap();
        assert!(read == old || read == new, "atomic read returned a mix");
        writer.await.unwrap();
        fixture
            .fs
            .write_file(&uri, &old, create_or_replace())
            .await
            .unwrap();
    }
}

// rename

/// Port of `move`.
#[test]
fn rename_moves_the_file() {
    let fixture = Fixture::new();
    rename(&fixture, "lorem.txt", "moved.txt", false).unwrap();
    assert!(!fixture.path("lorem.txt").exists());
    assert_eq!(read(&fixture, "moved.txt"), b"Lorem ipsum");
}

/// Port of `move - MIX CASE`: a case-only rename works on every platform.
#[test]
fn rename_changes_only_the_case_of_a_name() {
    let fixture = Fixture::new();
    rename(&fixture, "lorem.txt", "LOREM.txt", false).unwrap();
    let names: Vec<String> = block_on(fixture.fs.read_dir(&fixture.uri("")))
        .unwrap()
        .into_iter()
        .map(|(name, _)| name)
        .collect();
    assert!(names.contains(&"LOREM.txt".to_owned()));
}

/// Port of `move - same file`: a rename onto the same path is a no-op.
#[test]
fn rename_to_the_same_path_is_a_no_op() {
    let fixture = Fixture::new();
    rename(&fixture, "lorem.txt", "lorem.txt", false).unwrap();
    assert_eq!(read(&fixture, "lorem.txt"), b"Lorem ipsum");
}

/// Port of `move - source parent of target`: a folder cannot move into itself.
#[test]
fn rename_folder_into_itself_fails() {
    let fixture = Fixture::new();
    assert!(rename(&fixture, "deep", "deep/inner", false).is_err());
    assert!(fixture.path("deep/conway.js").exists());
}

/// Port of `move - FILE_MOVE_CONFLICT`.
#[test]
fn rename_onto_existing_target_without_overwrite_conflicts() {
    let fixture = Fixture::new();
    write(&fixture, "other.txt", b"other", create_or_replace()).unwrap();
    assert_eq!(
        error_of(rename(&fixture, "lorem.txt", "other.txt", false)),
        FileSystemErrorCode::FileExists
    );
    assert_eq!(read(&fixture, "other.txt"), b"other");
    assert_eq!(read(&fixture, "lorem.txt"), b"Lorem ipsum");
}

/// Port of `move - overwrite folder with file`: with `overwrite`, the file replaces the folder.
#[test]
fn rename_file_over_folder_with_overwrite_replaces_the_folder() {
    let fixture = Fixture::new();
    rename(&fixture, "lorem.txt", "deep", true).unwrap();
    assert_eq!(stat(&fixture, "deep").file_type, FileType::FILE);
    assert_eq!(read(&fixture, "deep"), b"Lorem ipsum");
}

#[test]
fn rename_file_over_file_with_overwrite_replaces_it() {
    let fixture = Fixture::new();
    write(&fixture, "other.txt", b"other", create_or_replace()).unwrap();
    rename(&fixture, "lorem.txt", "other.txt", true).unwrap();
    assert_eq!(read(&fixture, "other.txt"), b"Lorem ipsum");
    assert!(!fixture.path("lorem.txt").exists());
}

#[test]
fn rename_missing_source_is_not_found() {
    let fixture = Fixture::new();
    assert_eq!(
        error_of(rename(&fixture, "404.html", "x.html", false)),
        FileSystemErrorCode::FileNotFound
    );
}

// copy

/// Port of `copy`.
#[test]
fn copy_file_keeps_the_source() {
    let fixture = Fixture::new();
    copy(&fixture, "lorem.txt", "copy.txt", false).unwrap();
    assert_eq!(read(&fixture, "copy.txt"), b"Lorem ipsum");
    assert_eq!(read(&fixture, "lorem.txt"), b"Lorem ipsum");
}

/// Copies a folder tree, as the folder copy capability promises.
#[test]
fn copy_folder_copies_the_tree() {
    let fixture = Fixture::new();
    copy(&fixture, "deep", "deep-copy", false).unwrap();
    assert_eq!(read(&fixture, "deep-copy/conway.js"), b"// life");
    assert_eq!(read(&fixture, "deep-copy/nested/a.txt"), b"a");
    assert!(fixture.path("deep/conway.js").exists());
}

/// Port of `copy - same file`.
#[test]
fn copy_to_the_same_path_is_a_no_op() {
    let fixture = Fixture::new();
    copy(&fixture, "lorem.txt", "lorem.txt", false).unwrap();
    assert_eq!(read(&fixture, "lorem.txt"), b"Lorem ipsum");
}

#[test]
fn copy_onto_existing_target_without_overwrite_conflicts() {
    let fixture = Fixture::new();
    assert_eq!(
        error_of(copy(&fixture, "lorem.txt", "deep/conway.js", false)),
        FileSystemErrorCode::FileExists
    );
    assert_eq!(read(&fixture, "deep/conway.js"), b"// life");
}

/// Port of `copy - overwrite folder with file`.
#[test]
fn copy_file_over_folder_with_overwrite_replaces_the_folder() {
    let fixture = Fixture::new();
    copy(&fixture, "lorem.txt", "deep", true).unwrap();
    assert_eq!(stat(&fixture, "deep").file_type, FileType::FILE);
    assert_eq!(read(&fixture, "deep"), b"Lorem ipsum");
}

/// Port of `copy - MIX CASE same target - no overwrite`: on a case-sensitive disk the copy is a
/// new file; elsewhere the case-variant is the same file and the copy is refused.
#[test]
fn copy_to_a_case_variant_of_the_source() {
    let fixture = Fixture::new();
    let result = copy(&fixture, "lorem.txt", "LOREM.txt", false);
    if cfg!(target_os = "linux") {
        result.unwrap();
        assert_eq!(read(&fixture, "LOREM.txt"), b"Lorem ipsum");
    } else {
        assert_eq!(error_of(result), FileSystemErrorCode::FileExists);
    }
}

/// A folder cannot be copied into itself. Refusing it also stops the recursion.
#[test]
fn copy_folder_into_itself_fails() {
    let fixture = Fixture::new();
    assert!(copy(&fixture, "deep", "deep/inner", false).is_err());
    assert!(!fixture.path("deep/inner").exists());
}

/// A link is copied as a link (upstream `preserveSymlinks`).
#[cfg(unix)]
#[test]
fn copy_symlink_is_recreated_as_a_link() {
    let fixture = Fixture::new();
    std::os::unix::fs::symlink(fixture.path("lorem.txt"), fixture.path("link.txt")).unwrap();
    copy(&fixture, "link.txt", "link-copy.txt", false).unwrap();
    assert_eq!(
        std::fs::read_link(fixture.path("link-copy.txt")).unwrap(),
        fixture.path("lorem.txt")
    );
    assert_eq!(
        stat(&fixture, "link-copy.txt").file_type,
        FileType::FILE | FileType::SYMBOLIC_LINK
    );
    assert_eq!(stat(&fixture, "link-copy.txt").permissions, None);
}
