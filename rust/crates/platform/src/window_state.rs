//! Window state model and validation: pure logic, no display API.
//!
//! Ported from:
//! - `src/vs/platform/window/common/window.ts`: `WindowMinimumSize`, `IRectangle`,
//!   `zoomLevelToZoomFactor`, `DEFAULT_EMPTY_WINDOW_SIZE`, `DEFAULT_WORKSPACE_WINDOW_SIZE`
//! - `src/vs/platform/window/electron-main/window.ts`: `IWindowState`, `WindowMode`,
//!   `defaultWindowState`
//! - `src/vs/platform/windows/electron-main/windows.ts`: `WindowStateValidator`
//!   (`validateWindowState`, `validateWindowStateOnDisplay`, `getWorkingArea`)
//! - `src/vs/platform/window/electron-browser/window.ts`: `MIN_ZOOM_LEVEL`, `MAX_ZOOM_LEVEL`
//!
//! Units are logical (device-independent) pixels, as Electron stores window bounds. The caller
//! converts monitor geometry to logical pixels before it calls [`validate_window_state`].
//!
//! Types differ from the TypeScript on purpose: sizes are `u32`, so the "negative values" check
//! is a type property, and the check for zero sizes remains. Missing values are `None`, and
//! validation rejects a state that is missing any of the four bounds, as the TypeScript does.

/// Ported from `WindowMinimumSize` in `window.ts`. Electron enforces these as `minWidth` and
/// `minHeight` when a window is created, so validation does not clamp to them (see
/// `crate::window::WindowConfig`).
pub const MIN_WINDOW_WIDTH: u32 = 400;
/// Minimum width when a vertical panel is visible.
pub const MIN_WINDOW_WIDTH_WITH_VERTICAL_PANEL: u32 = 600;
/// Minimum height of a window.
pub const MIN_WINDOW_HEIGHT: u32 = 270;

/// Default size of a window without a workspace (`DEFAULT_EMPTY_WINDOW_SIZE`).
pub const DEFAULT_EMPTY_WINDOW_SIZE: Size = Size {
    width: 1200,
    height: 800,
};
/// Default size of a window with a workspace (`DEFAULT_WORKSPACE_WINDOW_SIZE`).
pub const DEFAULT_WORKSPACE_WINDOW_SIZE: Size = Size {
    width: 1440,
    height: 900,
};

/// Zoom levels are capped at these values (`MIN_ZOOM_LEVEL`, `MAX_ZOOM_LEVEL`).
pub const MIN_ZOOM_LEVEL: f64 = -8.0;
/// See [`MIN_ZOOM_LEVEL`].
pub const MAX_ZOOM_LEVEL: f64 = 8.0;

/// Distance from a display edge that must stay visible for a window to be dragged back
/// (the `128` literals in `validateWindowState`).
const EDGE_MARGIN: i64 = 128;

/// A point in logical pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Point {
    /// Horizontal position.
    pub x: i32,
    /// Vertical position.
    pub y: i32,
}

/// A size in logical pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Size {
    /// Width.
    pub width: u32,
    /// Height.
    pub height: u32,
}

/// A rectangle in logical pixels (`IRectangle`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    /// Left edge.
    pub x: i32,
    /// Top edge.
    pub y: i32,
    /// Width.
    pub width: u32,
    /// Height.
    pub height: u32,
}

impl Rect {
    /// Right edge, computed in `i64` so that large coordinates cannot overflow.
    fn right(self) -> i64 {
        i64::from(self.x) + i64::from(self.width)
    }

    /// Bottom edge, computed in `i64` so that large coordinates cannot overflow.
    fn bottom(self) -> i64 {
        i64::from(self.y) + i64::from(self.height)
    }
}

/// A connected display (`Electron.Display`), in logical pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Display {
    /// Identifier that a saved window state can refer to.
    pub id: i64,
    /// Full bounds of the display.
    pub bounds: Rect,
    /// Bounds without taskbars or docks. A zero-sized work area falls back to `bounds`.
    pub work_area: Rect,
}

/// Window mode (`WindowMode` in `electron-main/window.ts`).
///
/// The discriminants match the stored integers. `Minimized` is no longer produced, but stored
/// states can still contain it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum WindowMode {
    /// Maximized to the work area.
    Maximized = 0,
    /// Neither maximized nor fullscreen.
    Normal = 1,
    /// Minimized. Not used anymore; kept for stored-state compatibility.
    Minimized = 2,
    /// Fullscreen.
    Fullscreen = 3,
}

/// Saved or live state of one window (`IWindowState`). Every field is optional, as in the
/// TypeScript, because stored state can be partial.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct WindowState {
    /// Left edge.
    pub x: Option<i32>,
    /// Top edge.
    pub y: Option<i32>,
    /// Width.
    pub width: Option<u32>,
    /// Height.
    pub height: Option<u32>,
    /// Window mode.
    pub mode: Option<WindowMode>,
    /// Zoom level, where the zoom factor is `1.2 ^ zoom_level`.
    pub zoom_level: Option<f64>,
    /// Id of the display the window was on, used to restore fullscreen windows.
    pub display: Option<i64>,
}

/// Default state for a new window (`defaultWindowState`). Positions are left unset.
pub fn default_window_state(mode: WindowMode, has_workspace: bool) -> WindowState {
    let size = if has_workspace {
        DEFAULT_WORKSPACE_WINDOW_SIZE
    } else {
        DEFAULT_EMPTY_WINDOW_SIZE
    };
    WindowState {
        width: Some(size.width),
        height: Some(size.height),
        mode: Some(mode),
        ..WindowState::default()
    }
}

/// Zoom factor for a zoom level (`zoomLevelToZoomFactor`): `scale = 1.2 ^ level`.
pub fn zoom_level_to_zoom_factor(zoom_level: f64) -> f64 {
    1.2_f64.powf(zoom_level)
}

/// Caps a zoom level to [`MIN_ZOOM_LEVEL`]..=[`MAX_ZOOM_LEVEL`] (the clamp in `applyZoom`).
///
/// Deliberate difference: a NaN level becomes `0` (the default). The TypeScript `Math.min` and
/// `Math.max` would pass NaN through to the zoom call.
pub fn clamp_zoom_level(zoom_level: f64) -> f64 {
    if zoom_level.is_nan() {
        0.0
    } else {
        zoom_level.clamp(MIN_ZOOM_LEVEL, MAX_ZOOM_LEVEL)
    }
}

/// The part of `display` that windows should fit into (`getWorkingArea`). `None` if neither
/// the work area nor the bounds have a positive size.
fn working_area(display: &Display) -> Option<Rect> {
    if display.work_area.width > 0 && display.work_area.height > 0 {
        Some(display.work_area)
    } else if display.bounds.width > 0 && display.bounds.height > 0 {
        Some(display.bounds)
    } else {
        None
    }
}

/// Whether `bounds` is visible on `display`, meaning it overlaps the display's working area
/// (`validateWindowStateOnDisplay`). A zero-sized rectangle is never visible.
pub fn validate_window_state_on_display(bounds: Rect, display: &Display) -> bool {
    if bounds.width == 0 || bounds.height == 0 {
        return false;
    }
    let Some(area) = working_area(display) else {
        return false;
    };
    i64::from(area.x) < bounds.right()
        && i64::from(area.y) < bounds.bottom()
        && i64::from(bounds.x) < area.right()
        && i64::from(bounds.y) < area.bottom()
}

/// Validates a saved window state against the connected displays
/// (`WindowStateValidator.validateWindowState`).
///
/// Returns the state to restore (possibly moved or resized to fit a single display), or `None`
/// if the state cannot be used and the caller should fall back to [`default_window_state`].
/// `displays` must list the connected displays in logical pixels.
pub fn validate_window_state(state: &WindowState, displays: &[Display]) -> Option<WindowState> {
    let (Some(x), Some(y), Some(width), Some(height)) =
        (state.x, state.y, state.width, state.height)
    else {
        return None;
    };
    if width == 0 || height == 0 {
        return None;
    }
    let bounds = Rect {
        x,
        y,
        width,
        height,
    };

    if let [display] = displays {
        return Some(fit_to_single_display(state, bounds, display));
    }

    // Multi-display, fullscreen: go back to the display the window was on, at its origin.
    let saved_fullscreen_display = match (state.mode, state.display) {
        (Some(WindowMode::Fullscreen), Some(saved_id)) => {
            displays.iter().find(|d| d.id == saved_id)
        }
        _ => None,
    };
    if let Some(display) = saved_fullscreen_display {
        let mut restored = default_window_state(WindowMode::Fullscreen, false);
        restored.x = Some(display.bounds.x);
        restored.y = Some(display.bounds.y);
        return Some(restored);
    }

    // Multi-display, windowed: the window must overlap the display it mostly sits on.
    let display = display_matching(bounds, displays)?;
    validate_window_state_on_display(bounds, display).then_some(*state)
}

/// The display that overlaps `bounds` the most (`screen.getDisplayMatching`). `None` if no
/// display overlaps it, in which case the caller would reject the state anyway.
fn display_matching(bounds: Rect, displays: &[Display]) -> Option<&Display> {
    displays
        .iter()
        .map(|display| (display, overlap_area(bounds, display.bounds)))
        .filter(|(_, area)| *area > 0)
        .max_by_key(|(_, area)| *area)
        .map(|(display, _)| display)
}

/// Area of the overlap between two rectangles, in `i64` to avoid overflow.
fn overlap_area(a: Rect, b: Rect) -> i64 {
    let width = a.right().min(b.right()) - i64::from(a.x).max(i64::from(b.x));
    let height = a.bottom().min(b.bottom()) - i64::from(a.y).max(i64::from(b.y));
    if width <= 0 || height <= 0 {
        0
    } else {
        width * height
    }
}

/// Single display: keep the window on screen (the strict branch of `validateWindowState`).
/// Returns a state with bounds adjusted to the working area. If the display has no usable
/// area, the state is returned unchanged.
fn fit_to_single_display(state: &WindowState, bounds: Rect, display: &Display) -> WindowState {
    let Some(area) = working_area(display) else {
        return *state;
    };

    let mut x = i64::from(bounds.x);
    let mut y = i64::from(bounds.y);
    let width = i64::from(bounds.width).min(i64::from(area.width));
    let height = i64::from(bounds.height).min(i64::from(area.height));
    let (area_x, area_y) = (i64::from(area.x), i64::from(area.y));
    let (area_right, area_bottom) = (area.right(), area.bottom());

    // Keep the top-left corner inside the working area.
    x = x.max(area_x);
    y = y.max(area_y);

    // Keep at least `EDGE_MARGIN` pixels reachable on the right and bottom edges, by moving
    // the window back to the far edge.
    if x > area_right - EDGE_MARGIN {
        x = area_right - width;
    }
    if y > area_bottom - EDGE_MARGIN {
        y = area_bottom - height;
    }

    // The move above can push the corner out again, so clamp once more.
    x = x.max(area_x);
    y = y.max(area_y);

    WindowState {
        x: Some(to_i32(x)),
        y: Some(to_i32(y)),
        width: Some(to_u32(width)),
        height: Some(to_u32(height)),
        ..*state
    }
}

/// Converts an `i64` that is known to be within the range of the target type. Values outside
/// the range saturate, which can only happen for a display with out-of-range coordinates.
fn to_i32(value: i64) -> i32 {
    value.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32
}

/// See [`to_i32`].
fn to_u32(value: i64) -> u32 {
    value.clamp(0, i64::from(u32::MAX)) as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    fn display(id: i64, x: i32, y: i32, width: u32, height: u32) -> Display {
        let bounds = Rect {
            x,
            y,
            width,
            height,
        };
        Display {
            id,
            bounds,
            work_area: bounds,
        }
    }

    fn state(x: i32, y: i32, width: u32, height: u32) -> WindowState {
        WindowState {
            x: Some(x),
            y: Some(y),
            width: Some(width),
            height: Some(height),
            ..WindowState::default()
        }
    }

    fn bounds_of(state: &WindowState) -> (i32, i32, u32, u32) {
        (
            state.x.unwrap(),
            state.y.unwrap(),
            state.width.unwrap(),
            state.height.unwrap(),
        )
    }

    #[test]
    fn zoom_factor_matches_electron_formula() {
        assert_eq!(zoom_level_to_zoom_factor(0.0), 1.0);
        assert!((zoom_level_to_zoom_factor(1.0) - 1.2).abs() < 1e-12);
        assert!((zoom_level_to_zoom_factor(-2.0) - 1.0 / 1.44).abs() < 1e-12);
    }

    #[test]
    fn zoom_level_is_capped_and_nan_becomes_default() {
        assert_eq!(clamp_zoom_level(20.0), MAX_ZOOM_LEVEL);
        assert_eq!(clamp_zoom_level(-20.0), MIN_ZOOM_LEVEL);
        assert_eq!(clamp_zoom_level(f64::INFINITY), MAX_ZOOM_LEVEL);
        assert_eq!(clamp_zoom_level(f64::NAN), 0.0);
        assert_eq!(clamp_zoom_level(2.5), 2.5);
    }

    #[test]
    fn default_state_uses_workspace_or_empty_size() {
        let empty = default_window_state(WindowMode::Normal, false);
        assert_eq!((empty.width, empty.height), (Some(1200), Some(800)));
        assert_eq!(empty.mode, Some(WindowMode::Normal));
        assert_eq!(empty.x, None);

        let workspace = default_window_state(WindowMode::Maximized, true);
        assert_eq!((workspace.width, workspace.height), (Some(1440), Some(900)));
        assert_eq!(workspace.mode, Some(WindowMode::Maximized));
    }

    #[test]
    fn partial_or_zero_state_is_rejected() {
        let display = display(1, 0, 0, 1920, 1080);
        let partial = WindowState {
            x: Some(10),
            ..WindowState::default()
        };
        assert_eq!(validate_window_state(&partial, &[display]), None);
        assert_eq!(
            validate_window_state(&state(10, 10, 0, 500), &[display]),
            None
        );
        assert_eq!(
            validate_window_state(&state(10, 10, 500, 0), &[display]),
            None
        );
    }

    #[test]
    fn no_displays_rejects_state() {
        assert_eq!(validate_window_state(&state(0, 0, 800, 600), &[]), None);
    }

    #[test]
    fn single_display_keeps_window_inside_work_area() {
        let display = display(1, 0, 0, 1920, 1080);
        // Window starting above and left of the screen is moved to the corner.
        let restored = validate_window_state(&state(-300, -50, 800, 600), &[display]).unwrap();
        assert_eq!(bounds_of(&restored), (0, 0, 800, 600));
    }

    #[test]
    fn single_display_shrinks_oversized_window() {
        let display = display(1, 0, 0, 1920, 1080);
        let restored = validate_window_state(&state(100, 100, 3000, 2000), &[display]).unwrap();
        assert_eq!(bounds_of(&restored), (100, 100, 1920, 1080));
    }

    #[test]
    fn single_display_moves_window_back_from_right_and_bottom_edges() {
        let display = display(1, 0, 0, 1920, 1080);
        // x is beyond right edge minus 128 px: pulled back so the window ends at the edge.
        let restored = validate_window_state(&state(1900, 1000, 800, 600), &[display]).unwrap();
        assert_eq!(bounds_of(&restored), (1920 - 800, 1080 - 600, 800, 600));
    }

    #[test]
    fn single_display_respects_work_area_offset() {
        // A taskbar at the bottom: the work area is shorter than the bounds.
        let display = Display {
            id: 1,
            bounds: Rect {
                x: 0,
                y: 0,
                width: 1920,
                height: 1080,
            },
            work_area: Rect {
                x: 0,
                y: 0,
                width: 1920,
                height: 1040,
            },
        };
        // y = 950 is below the work area's bottom minus 128 px, so it moves to the bottom edge.
        let restored = validate_window_state(&state(0, 950, 800, 600), &[display]).unwrap();
        assert_eq!(bounds_of(&restored).1, 1040 - 600);
    }

    #[test]
    fn single_display_with_empty_area_returns_state_unchanged() {
        let display = Display {
            id: 1,
            bounds: Rect {
                x: 0,
                y: 0,
                width: 0,
                height: 0,
            },
            work_area: Rect {
                x: 0,
                y: 0,
                width: 0,
                height: 0,
            },
        };
        let input = state(-50, -50, 800, 600);
        assert_eq!(validate_window_state(&input, &[display]), Some(input));
    }

    #[test]
    fn single_display_keeps_other_fields() {
        let display = display(1, 0, 0, 1920, 1080);
        let input = WindowState {
            mode: Some(WindowMode::Maximized),
            zoom_level: Some(2.0),
            ..state(-10, 0, 800, 600)
        };
        let restored = validate_window_state(&input, &[display]).unwrap();
        assert_eq!(restored.mode, Some(WindowMode::Maximized));
        assert_eq!(restored.zoom_level, Some(2.0));
    }

    #[test]
    fn multi_display_accepts_window_on_a_display() {
        let left = display(1, 0, 0, 1920, 1080);
        let right = display(2, 1920, 0, 1920, 1080);
        let input = state(2000, 100, 800, 600);
        assert_eq!(validate_window_state(&input, &[left, right]), Some(input));
    }

    #[test]
    fn multi_display_rejects_window_off_all_displays() {
        let left = display(1, 0, 0, 1920, 1080);
        let right = display(2, 1920, 0, 1920, 1080);
        assert_eq!(
            validate_window_state(&state(5000, 5000, 800, 600), &[left, right]),
            None
        );
    }

    #[test]
    fn multi_display_rejects_window_only_in_taskbar_strip() {
        let taskbar_display = Display {
            id: 1,
            bounds: Rect {
                x: 0,
                y: 0,
                width: 1920,
                height: 1080,
            },
            work_area: Rect {
                x: 0,
                y: 0,
                width: 1920,
                height: 1040,
            },
        };
        let other = display(2, 1920, 0, 1920, 1080);
        // Overlaps only the bottom 10 px of display 1, which is outside its work area.
        assert_eq!(
            validate_window_state(&state(0, 1070, 100, 100), &[taskbar_display, other]),
            None
        );
    }

    #[test]
    fn multi_display_fullscreen_returns_saved_display_origin() {
        let left = display(1, 0, 0, 1920, 1080);
        let right = display(2, 1920, 0, 2560, 1440);
        let input = WindowState {
            mode: Some(WindowMode::Fullscreen),
            display: Some(2),
            ..state(-4000, -4000, 10, 10)
        };
        let restored = validate_window_state(&input, &[left, right]).unwrap();
        assert_eq!(restored.mode, Some(WindowMode::Fullscreen));
        assert_eq!((restored.x, restored.y), (Some(1920), Some(0)));
        assert_eq!((restored.width, restored.height), (Some(1200), Some(800)));
        assert_eq!(restored.display, None);
    }

    #[test]
    fn multi_display_fullscreen_with_unknown_display_uses_bounds_check() {
        let left = display(1, 0, 0, 1920, 1080);
        let right = display(2, 1920, 0, 1920, 1080);
        let input = WindowState {
            mode: Some(WindowMode::Fullscreen),
            display: Some(99),
            ..state(100, 100, 800, 600)
        };
        assert_eq!(validate_window_state(&input, &[left, right]), Some(input));
    }

    #[test]
    fn display_matching_picks_largest_overlap() {
        let left = display(1, 0, 0, 1920, 1080);
        let right = display(2, 1920, 0, 1920, 1080);
        // 300 px on the left display, 500 px on the right display.
        let bounds = Rect {
            x: 1620,
            y: 0,
            width: 800,
            height: 100,
        };
        assert_eq!(
            display_matching(bounds, &[left, right]).map(|d| d.id),
            Some(2)
        );
    }

    #[test]
    fn overlap_area_handles_disjoint_and_nested_rectangles() {
        let outer = Rect {
            x: 0,
            y: 0,
            width: 100,
            height: 100,
        };
        let inner = Rect {
            x: 10,
            y: 10,
            width: 20,
            height: 20,
        };
        let far = Rect {
            x: 500,
            y: 500,
            width: 10,
            height: 10,
        };
        assert_eq!(overlap_area(outer, inner), 400);
        assert_eq!(overlap_area(outer, far), 0);
    }

    #[test]
    fn large_coordinates_do_not_overflow() {
        let display = display(1, i32::MAX - 100, 0, 100, 100);
        let input = state(i32::MAX - 50, 0, u32::MAX, 10);
        // Must not panic. Width is clamped to the area width.
        let restored = validate_window_state(&input, &[display]).unwrap();
        assert_eq!(restored.width, Some(100));
    }
}
