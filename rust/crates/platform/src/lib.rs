//! Platform layer for Lathe: window state, window configuration, and the winit event loop.
//!
//! Owned by role 02 (see `COORDINATION.md`). Depends on `vscode-api` and winit only.
//!
//! - [`window_state`]: the window state model and its validation against connected displays.
//!   Ported from `src/vs/platform/window/` and `src/vs/platform/windows/electron-main/`.
//!   Pure logic.
//! - [`window`]: window configuration built from a state.
//! - [`event_loop`]: the winit runner. It forwards events as [`PlatformEvent`] and accepts
//!   user events from other threads (ADR 0005).
//!
//! The draw-layer API that roles 05 and 07 will consume is sketched in `DRAW_LAYER.md`.

pub mod event_loop;
pub mod window;
pub mod window_state;

pub use event_loop::{
    EventLoop, EventLoopClosed, EventSender, LoopControl, PlatformError, PlatformEvent, Window,
    WindowId,
};
pub use window::WindowConfig;
pub use window_state::{Display, Rect, Size, WindowMode, WindowState, validate_window_state};
