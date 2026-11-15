//! Shared helpers for the provider tests: a temp folder with a small fixture tree, and blocking
//! wrappers around the async contract.

#![allow(dead_code)] // each test binary uses a different subset

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use lathe_workspace::DiskFileSystemProvider;
use tempfile::TempDir;
use vscode_api::files::{FileSystemError, FileSystemErrorCode, FileSystemProvider, Stat};
use vscode_api::uri::Uri;

pub struct Fixture {
    pub dir: TempDir,
    pub fs: Arc<DiskFileSystemProvider>,
}

impl Fixture {
    pub fn new() -> Self {
        let dir = tempfile::tempdir().expect("temp dir");
        // A small tree that most cases start from: a file, a folder with files, and an empty folder.
        fs::write(dir.path().join("lorem.txt"), "Lorem ipsum").unwrap();
        fs::create_dir_all(dir.path().join("deep").join("nested")).unwrap();
        fs::write(dir.path().join("deep").join("conway.js"), "// life").unwrap();
        fs::write(dir.path().join("deep").join("nested").join("a.txt"), "a").unwrap();
        fs::create_dir(dir.path().join("empty")).unwrap();
        Self {
            dir,
            fs: Arc::new(DiskFileSystemProvider::new()),
        }
    }

    pub fn path(&self, relative: &str) -> PathBuf {
        self.dir.path().join(relative)
    }

    pub fn uri(&self, relative: &str) -> Uri {
        uri_of(&self.path(relative))
    }
}

pub fn uri_of(path: &Path) -> Uri {
    Uri::file(&path.to_string_lossy())
}

pub fn block_on<F: std::future::Future>(future: F) -> F::Output {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime")
        .block_on(future)
}

pub fn stat(fixture: &Fixture, relative: &str) -> Stat {
    block_on(fixture.fs.stat(&fixture.uri(relative))).expect("stat succeeds")
}

pub fn error_code<T: std::fmt::Debug>(result: Result<T, FileSystemError>) -> FileSystemErrorCode {
    result.expect_err("operation fails").code
}
