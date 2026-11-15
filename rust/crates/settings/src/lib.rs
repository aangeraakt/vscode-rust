//! Settings and keybinding model for Lathe, a Rust port of the behavior of microsoft/vscode (MIT).
//!
//! This crate is owned by role 10 (SCM, Settings & Extensions, CI); see `COORDINATION.md`.
//! Each module cites the TypeScript source it was ported from. Paths are relative to the
//! repository root at the reference commit pinned in `docs/decisions/0001-project-name-and-reference.md`.
//!
//! Ported code is derived from microsoft/vscode, Copyright (c) Microsoft Corporation, released
//! under the MIT License. Its license text is in the repository's `LICENSE.txt`.

pub mod key_codes;
pub mod keybindings;
pub mod platform;
