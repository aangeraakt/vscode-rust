//! Window configuration built from a window state.
//!
//! Ported from the window-creation options in `src/vs/platform/windows/electron-main/windows.ts`
//! (`defaultWindowOptions`: `minWidth`, `minHeight`) and the mode handling in
//! `src/vs/platform/windows/electron-main/windowImpl.ts` (`restoreWindowState`, `setFullScreen`).
//!
//! Sizes are passed to winit as logical (content) sizes. Electron's `BrowserWindow` sizes are
//! outer sizes, so restored windows differ by the title bar height. Fixing that needs the
//! platform frame size and is left for the window-chrome work.

use winit::dpi::{LogicalPosition, LogicalSize};
use winit::window::{Fullscreen, WindowAttributes};

use crate::window_state::{
    MIN_WINDOW_HEIGHT, MIN_WINDOW_WIDTH, Point, Size, WindowMode, WindowState,
};

/// What a new native window should look like when it opens.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowConfig {
    /// Window title.
    pub title: String,
    /// Logical size of the content area.
    pub size: Size,
    /// Logical position, if the state had one.
    pub position: Option<Point>,
    /// Smallest size the user can resize to.
    pub min_size: Size,
    /// Open maximized.
    pub maximized: bool,
    /// Open fullscreen (borderless, on the current display).
    pub fullscreen: bool,
}

impl WindowConfig {
    /// Config for a window restored from `state`. The caller validates the state first with
    /// [`crate::window_state::validate_window_state`]. Missing size fields fall back to
    /// `fallback`, and a missing position leaves placement to the platform.
    pub fn from_state(state: &WindowState, title: impl Into<String>, fallback: Size) -> Self {
        let size = Size {
            width: state.width.unwrap_or(fallback.width),
            height: state.height.unwrap_or(fallback.height),
        };
        let position = match (state.x, state.y) {
            (Some(x), Some(y)) => Some(Point { x, y }),
            _ => None,
        };
        Self {
            title: title.into(),
            size,
            position,
            min_size: Size {
                width: MIN_WINDOW_WIDTH,
                height: MIN_WINDOW_HEIGHT,
            },
            maximized: state.mode == Some(WindowMode::Maximized),
            fullscreen: state.mode == Some(WindowMode::Fullscreen),
        }
    }

    /// The winit attributes for this config.
    pub(crate) fn attributes(&self) -> WindowAttributes {
        let mut attributes = WindowAttributes::default()
            .with_title(self.title.as_str())
            .with_inner_size(logical_size(self.size))
            .with_min_inner_size(logical_size(self.min_size))
            .with_maximized(self.maximized);
        if let Some(position) = self.position {
            attributes = attributes.with_position(LogicalPosition::new(
                f64::from(position.x),
                f64::from(position.y),
            ));
        }
        if self.fullscreen {
            attributes = attributes.with_fullscreen(Some(Fullscreen::Borderless(None)));
        }
        attributes
    }
}

fn logical_size(size: Size) -> LogicalSize<f64> {
    LogicalSize::new(f64::from(size.width), f64::from(size.height))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::window_state::{DEFAULT_EMPTY_WINDOW_SIZE, default_window_state};

    #[test]
    fn restored_state_sets_position_size_and_mode() {
        let state = WindowState {
            x: Some(10),
            y: Some(20),
            width: Some(900),
            height: Some(700),
            mode: Some(WindowMode::Normal),
            ..WindowState::default()
        };
        let config = WindowConfig::from_state(&state, "Lathe", DEFAULT_EMPTY_WINDOW_SIZE);
        assert_eq!(
            config.size,
            Size {
                width: 900,
                height: 700
            }
        );
        assert_eq!(config.position, Some(Point { x: 10, y: 20 }));
        assert!(!config.maximized);
        assert!(!config.fullscreen);
        assert_eq!(
            config.min_size,
            Size {
                width: 400,
                height: 270
            }
        );
    }

    #[test]
    fn missing_fields_use_fallback_and_no_position() {
        let config = WindowConfig::from_state(
            &default_window_state(WindowMode::Maximized, false),
            "Lathe",
            DEFAULT_EMPTY_WINDOW_SIZE,
        );
        assert_eq!(config.size, DEFAULT_EMPTY_WINDOW_SIZE);
        assert_eq!(config.position, None);
        assert!(config.maximized);

        let only_x = WindowState {
            x: Some(5),
            ..WindowState::default()
        };
        let config = WindowConfig::from_state(&only_x, "Lathe", DEFAULT_EMPTY_WINDOW_SIZE);
        assert_eq!(config.position, None);
    }

    #[test]
    fn fullscreen_mode_sets_fullscreen_only() {
        let state = WindowState {
            mode: Some(WindowMode::Fullscreen),
            ..WindowState::default()
        };
        let config = WindowConfig::from_state(&state, "Lathe", DEFAULT_EMPTY_WINDOW_SIZE);
        assert!(config.fullscreen);
        assert!(!config.maximized);
    }
}
