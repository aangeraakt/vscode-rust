//! Editor view layout for Lathe, a Rust port of the behavior of microsoft/vscode.
//!
//! Owned by role 05 (Editor Rendering). This crate holds the pure layout model that the editor
//! view uses to place lines and view zones (whitespace) vertically, and the scroll model that
//! drives it. It does no rendering.
//!
//! Each module cites the TypeScript source it was ported from under `src/vs/editor/common/viewLayout/`
//! or `src/vs/base/common/`.

pub mod line_heights;
pub mod lines_layout;
pub mod scrollable;

pub use line_heights::{CustomLineHeightData, LineHeightChangeAccessor, LineHeightsManager};
pub use lines_layout::{
    EditorWhitespace, LinesLayout, PartialViewLinesViewportData, WhitespaceChangeAccessor,
    WhitespaceViewportData,
};
pub use scrollable::{
    Clock, NewScrollDimensions, NewScrollPosition, ScrollDimensions, ScrollEvent, ScrollPosition,
    ScrollState, Scrollable, ScrollableOptions, SmoothScrollPosition, SmoothScrollingOperation,
    SmoothScrollingUpdate, SystemClock,
};
