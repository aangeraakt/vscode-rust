//! Port of `src/vs/base/browser/ui/sash/sash.ts`: the enums that describe a sash.
//!
//! Only the state vocabulary is ported. Hit testing, pointer capture and drawing are rendering
//! concerns and belong to the widget layer (ADR 0002).

/// The axis a split view or grid branch lays its children along. Port of `Orientation`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Orientation {
    /// Children are stacked top to bottom, separated by horizontal sashes. The default.
    Vertical,
    /// Children sit left to right, separated by vertical sashes.
    Horizontal,
}

/// Which drag directions a sash allows. Port of `SashState`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SashState {
    /// No interaction: neither neighbour can grow or shrink.
    Disabled,
    /// Dragging is allowed in both directions.
    Enabled,
    /// Dragging is allowed only down or to the right: the view before the sash cannot shrink.
    AtMinimum,
    /// Dragging is allowed only up or to the left: the view after the sash cannot shrink.
    AtMaximum,
}
