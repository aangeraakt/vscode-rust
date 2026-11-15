//! Workspace services for Lathe: the local disk file system provider (`file` scheme) and, later,
//! file watching and search.
//!
//! Owned by role 08 (see `COORDINATION.md`). Ported from `src/vs/platform/files/node/` of
//! microsoft/vscode, which is MIT licensed (see `LICENSE.txt` at the repository root). Each module
//! cites the TypeScript source it was ported from; the TypeScript tree at the pinned reference
//! commit is the behavioral reference.

pub mod disk;
mod locks;

pub use disk::DiskFileSystemProvider;
