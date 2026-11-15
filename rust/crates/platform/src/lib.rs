//! Platform layer for Lathe: the window state model.
//!
//! Owned by role 02 (see `COORDINATION.md`). Depends on `vscode-api` only.
//!
//! - [`window_state`]: the window state model and its validation against connected displays.
//!   Ported from `src/vs/platform/window/` and `src/vs/platform/windows/electron-main/`.
//!   Pure logic: no display API is called.

pub mod window_state;

pub use window_state::{Display, Rect, Size, WindowMode, WindowState, validate_window_state};
