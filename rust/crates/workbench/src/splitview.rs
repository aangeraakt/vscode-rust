//! Port of `src/vs/base/browser/ui/splitview/splitview.ts`: the one-dimensional flex layout behind
//! split views and grids.
//!
//! Ported: size constraints, layout priorities, sizing strategies, proportional layout, view
//! visibility with cached sizes, sash drag math with snapping, and sash enablement states.
//! Not ported: DOM containers, scrollbars, sash pointer handling and styles (rendering, ADR 0002).
//!
//! Divergences from the TypeScript source:
//! - `IView.onDidChange` is not a subscription. The host calls [`SplitView::view_did_change`]
//!   after it changes a view's constraints.
//! - The "Cant modify splitview" busy guard is not needed: `&mut self` rules out re-entrant
//!   modification at compile time.
//! - Out-of-bounds indexes that upstream reports by throwing panic here, because they are caller
//!   bugs.
//! - Constraint changes made from inside `setVisible` (upstream issue #334167) are not delivered
//!   while the view is being shown or hidden. The host sees them after the call returns.

use vscode_api::event::{Emitter, Event};

use crate::sash::SashState;

#[cfg(test)]
mod tests;

/// Priority of a view when space is distributed. Port of `LayoutPriority`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum LayoutPriority {
    /// Resized in index order. The default.
    #[default]
    Normal,
    /// Resized last, so it absorbs space only after the other views.
    Low,
    /// Resized first, so the other views keep their sizes.
    High,
}

/// A view in a [`SplitView`]. Port of `IView`.
///
/// Size constraints are read live on every layout. A view that changes them must tell its
/// split view through [`SplitView::view_did_change`].
pub trait View {
    /// Context passed down from [`SplitView::layout`] to each view's [`View::layout`].
    type Context;

    /// The smallest size this view accepts. Use `0.0` for none.
    fn minimum_size(&self) -> f64;

    /// The largest size this view accepts. Use `f64::INFINITY` for none.
    fn maximum_size(&self) -> f64;

    /// Priority when space is distributed. Only consulted without proportional layout.
    fn priority(&self) -> LayoutPriority {
        LayoutPriority::Normal
    }

    /// Whether the view is resized proportionally when the split view is. Defaults to `true`.
    fn proportional_layout(&self) -> bool {
        true
    }

    /// Whether the view's layout depends only on the latest size and context it received. When
    /// every view says so, an intermediate layout that is immediately superseded is skipped.
    /// Views with nested layout state must return `false`.
    fn path_independent_layout(&self) -> bool {
        false
    }

    /// Whether the view snaps (hides) when dragged to its minimum size.
    fn snap(&self) -> bool {
        false
    }

    /// Lays the view out. `offset` is measured from the start of the split view.
    fn layout(&mut self, size: f64, offset: f64, context: Option<&Self::Context>);

    /// Called when the view is shown or hidden.
    fn set_visible(&mut self, _visible: bool) {}
}

/// How space is found or released when a view is added or removed. Port of `Sizing`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Sizing {
    /// Distribute the whole split view size evenly among all views. Port of `Sizing.Distribute`.
    Distribute,
    /// Take the space from the view at this index. Port of `Sizing.Split`.
    Split(usize),
    /// `Distribute` when the views are already even, otherwise `Split` at this index.
    /// Port of `Sizing.Auto`.
    Auto(usize),
    /// Add the view hidden, with this cached visible size. Port of `Sizing.Invisible`.
    Invisible(f64),
}

/// The size argument of [`SplitView::add_view`]: fixed pixels or a sizing strategy.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ViewSize {
    /// A fixed size in pixels. The view is visible.
    Pixels(f64),
    /// A sizing strategy.
    Sizing(Sizing),
}

impl From<f64> for ViewSize {
    fn from(pixels: f64) -> Self {
        Self::Pixels(pixels)
    }
}

impl From<Sizing> for ViewSize {
    fn from(sizing: Sizing) -> Self {
        Self::Sizing(sizing)
    }
}

/// Construction options. Port of the layout-relevant parts of `ISplitViewOptions`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SplitViewOptions {
    /// Swap the meaning of Alt during a sash drag: Alt resizes both neighbours equally.
    pub inverse_alt_behavior: bool,
    /// Resize views in proportion to their sizes when the split view is resized.
    pub proportional_layout: bool,
}

impl Default for SplitViewOptions {
    fn default() -> Self {
        Self {
            inverse_alt_behavior: false,
            proportional_layout: true,
        }
    }
}

/// A view and its initial state, for [`SplitView::from_descriptor`]. Port of
/// `ISplitViewDescriptor`'s entries.
#[derive(Debug, Clone)]
pub struct ViewDescriptor<V> {
    /// The view.
    pub view: V,
    /// Its size when visible, or its cached visible size when hidden.
    pub size: f64,
    /// Whether the view starts visible.
    pub visible: bool,
}

/// A layout container of views separated by sashes. Port of `SplitView`.
///
/// Views are kept in order. Between each pair of views sits one sash, whose state is available
/// from [`SplitView::sash_states`]. Drag and reset gestures are reported with `sash_*` calls and
/// emitted events.
pub struct SplitView<V: View> {
    size: f64,
    layout_context: Option<V::Context>,
    content_size: f64,
    proportions: Option<Vec<Option<f64>>>,
    view_items: Vec<ViewItem<V>>,
    sash_states: Vec<SashState>,
    sash_drag: Option<SashDrag>,
    inverse_alt_behavior: bool,
    proportional_layout: bool,
    start_snapping_enabled: bool,
    end_snapping_enabled: bool,
    sash_change: Emitter<usize>,
    sash_reset: Emitter<usize>,
}

impl<V: View> SplitView<V> {
    /// Creates an empty split view.
    pub fn new(options: SplitViewOptions) -> Self {
        Self {
            size: 0.0,
            layout_context: None,
            content_size: 0.0,
            proportions: None,
            view_items: Vec::new(),
            sash_states: Vec::new(),
            sash_drag: None,
            inverse_alt_behavior: options.inverse_alt_behavior,
            proportional_layout: options.proportional_layout,
            start_snapping_enabled: true,
            end_snapping_enabled: true,
            sash_change: Emitter::new(),
            sash_reset: Emitter::new(),
        }
    }

    /// Creates a split view of `size` pixels holding `views` in order, without laying them out.
    /// Port of the `descriptor` option. Hidden views keep their cached visible size.
    pub fn from_descriptor(
        options: SplitViewOptions,
        size: f64,
        views: Vec<ViewDescriptor<V>>,
    ) -> Self {
        let mut split = Self::new(options);
        split.size = size;
        for (index, descriptor) in views.into_iter().enumerate() {
            let sizing = if descriptor.visible {
                ViewSize::Pixels(descriptor.size)
            } else {
                ViewSize::Sizing(Sizing::Invisible(descriptor.size))
            };
            split.do_add_view(descriptor.view, sizing, index, true);
        }
        split.content_size = split.items_size();
        split.save_proportions();
        split
    }

    /// The number of views.
    pub fn len(&self) -> usize {
        self.view_items.len()
    }

    /// Whether there are no views.
    pub fn is_empty(&self) -> bool {
        self.view_items.is_empty()
    }

    /// The sum of all views' sizes, as of the last layout.
    pub fn content_size(&self) -> f64 {
        self.content_size
    }

    /// The sum of the views' minimum sizes. Hidden views contribute `0`.
    pub fn minimum_size(&self) -> f64 {
        self.view_items.iter().map(ViewItem::minimum_size).sum()
    }

    /// The sum of the views' maximum sizes, or infinity when there are no views.
    pub fn maximum_size(&self) -> f64 {
        if self.view_items.is_empty() {
            f64::INFINITY
        } else {
            self.view_items.iter().map(ViewItem::maximum_size).sum()
        }
    }

    /// The state of each sash, in order. There is one fewer sash than views.
    pub fn sash_states(&self) -> &[SashState] {
        &self.sash_states
    }

    /// Whether snapping at the start of the split view is enabled.
    pub fn start_snapping_enabled(&self) -> bool {
        self.start_snapping_enabled
    }

    /// Enables or disables snapping at the start of the split view. Updates sash states.
    pub fn set_start_snapping_enabled(&mut self, enabled: bool) {
        if self.start_snapping_enabled != enabled {
            self.start_snapping_enabled = enabled;
            self.update_sash_enablement();
        }
    }

    /// Whether snapping at the end of the split view is enabled.
    pub fn end_snapping_enabled(&self) -> bool {
        self.end_snapping_enabled
    }

    /// Enables or disables snapping at the end of the split view. Updates sash states.
    pub fn set_end_snapping_enabled(&mut self, enabled: bool) {
        if self.end_snapping_enabled != enabled {
            self.end_snapping_enabled = enabled;
            self.update_sash_enablement();
        }
    }

    /// Fires with the index of a sash when a drag ends. Port of `onDidSashChange`.
    pub fn on_did_sash_change(&self) -> Event<usize> {
        self.sash_change.event()
    }

    /// Fires with the index of a sash when it is reset (double clicked). Port of `onDidSashReset`.
    pub fn on_did_sash_reset(&self) -> Event<usize> {
        self.sash_reset.event()
    }

    /// The view at `index`.
    ///
    /// # Panics
    /// If `index` is out of bounds.
    pub fn view(&self, index: usize) -> &V {
        &self.view_items[self.checked(index)].view
    }

    /// Mutable access to the view at `index`. After changing its size constraints, call
    /// [`SplitView::view_did_change`].
    ///
    /// # Panics
    /// If `index` is out of bounds.
    pub fn view_mut(&mut self, index: usize) -> &mut V {
        let index = self.checked(index);
        &mut self.view_items[index].view
    }

    /// Tells the split view that the view at `index` changed its size constraints, relaying out.
    /// Port of `IView.onDidChange`. `size` is the view's preferred size, or `None` to keep its
    /// current size. Indexes out of bounds are ignored, as upstream does.
    pub fn view_did_change(&mut self, index: usize, size: Option<f64>) {
        let Some(item) = self.view_items.get(index) else {
            return;
        };
        let current = item.size;
        let size = clamp(
            size.unwrap_or(current),
            item.minimum_size(),
            item.maximum_size(),
        );

        if self.inverse_alt_behavior && index > 0 {
            // Grow or shrink both sides equally: resize the view before by half the change and
            // let `resize` clamp it.
            let delta = ((current - size) / 2.0).floor();
            self.resize(
                index - 1,
                delta,
                None,
                Priorities::default(),
                Limits::default(),
                Snaps::default(),
            );
            self.distribute_empty_space(None);
            self.layout_views();
        } else {
            self.view_items[index].size = size;
            let (mut low, high) = self.priority_indexes(Some(index));
            low.push(index);
            self.relayout(&low, &high, false);
        }
    }

    /// Adds a view at `index`. When `skip_layout` is true the views are not laid out, so a batch
    /// of additions can be followed by one [`SplitView::layout`].
    ///
    /// # Panics
    /// If `index` is greater than the number of views.
    pub fn add_view(
        &mut self,
        view: V,
        size: impl Into<ViewSize>,
        index: usize,
        skip_layout: bool,
    ) {
        self.do_add_view(view, size.into(), index, skip_layout);
    }

    /// Adds a view at the end and lays out the views.
    pub fn push_view(&mut self, view: V, size: impl Into<ViewSize>) {
        let index = self.len();
        self.do_add_view(view, size.into(), index, false);
    }

    /// Removes the view at `index`, giving its space to the views chosen by `sizing`. `None`
    /// gives it to the views after it, which then absorb the space as a normal relayout does.
    ///
    /// # Panics
    /// If `index` is out of bounds.
    pub fn remove_view(&mut self, index: usize, sizing: Option<Sizing>) -> V {
        self.checked(index);
        let original_len = self.len();
        let sizing = sizing.map(|s| self.resolve_auto(s));

        // The reference view is named before removal, so its index is shifted afterwards.
        let reference = match sizing {
            Some(Sizing::Split(reference)) => Some(reference),
            _ => None,
        };

        let removed = self.view_items.remove(index);

        if let Some(reference) = reference {
            if reference < original_len && reference != index {
                let target = if reference > index {
                    reference - 1
                } else {
                    reference
                };
                self.view_items[target].size += removed.size;
            }
        }

        if !self.view_items.is_empty() {
            self.sash_states.remove(index.saturating_sub(1));
        }

        let distribute = sizing == Some(Sizing::Distribute);
        self.relayout(&[], &[], distribute);
        if distribute {
            self.distribute_view_sizes();
        }

        removed.view
    }

    /// Removes every view, returning them in order.
    pub fn remove_all_views(&mut self) -> Vec<V> {
        let items = std::mem::take(&mut self.view_items);
        self.sash_states.clear();
        self.relayout(&[], &[], false);
        items.into_iter().map(|item| item.view).collect()
    }

    /// Moves the view at `from` so that it ends up at `to`, keeping its size or hidden state.
    ///
    /// # Panics
    /// If `from` is out of bounds.
    pub fn move_view(&mut self, from: usize, to: usize) {
        let sizing = match self.view_cached_visible_size(from) {
            None => ViewSize::Pixels(self.view_size(from)),
            Some(cached) => ViewSize::Sizing(Sizing::Invisible(cached)),
        };
        let view = self.remove_view(from, None);
        self.add_view(view, sizing, to, false);
    }

    /// Swaps the views at `from` and `to`, exchanging their visible sizes. A swap with itself
    /// does nothing.
    ///
    /// # Panics
    /// If either index is out of bounds.
    pub fn swap_views(&mut self, from: usize, to: usize) {
        if from == to {
            self.checked(from);
            return;
        }
        if from > to {
            return self.swap_views(to, from);
        }

        let from_size = self.view_size(from);
        let to_size = self.view_size(to);
        let to_view = self.remove_view(to, None);
        let from_view = self.remove_view(from, None);

        self.add_view(to_view, from_size, from, false);
        self.add_view(from_view, to_size, to, false);
    }

    /// Whether the view at `index` is visible.
    ///
    /// # Panics
    /// If `index` is out of bounds.
    pub fn is_view_visible(&self, index: usize) -> bool {
        self.view_items[self.checked(index)].visible()
    }

    /// Shows or hides the view at `index`. Hiding caches its size, showing restores it.
    ///
    /// # Panics
    /// If `index` is out of bounds.
    pub fn set_view_visible(&mut self, index: usize, visible: bool) {
        let index = self.checked(index);
        self.view_items[index].set_visible(visible, None);

        self.distribute_empty_space(Some(index));
        self.layout_views();
        self.save_proportions();
    }

    /// The visible size the view had before it was hidden, or `None` when it is visible.
    ///
    /// # Panics
    /// If `index` is out of bounds.
    pub fn view_cached_visible_size(&self, index: usize) -> Option<f64> {
        self.view_items[self.checked(index)].cached_visible_size
    }

    /// Lays out the split view at `size` pixels. Views keep their proportions when proportional
    /// layout is on. `context` is passed to every view's [`View::layout`].
    pub fn layout(&mut self, size: f64, context: Option<V::Context>) {
        let previous_size = self.size.max(self.content_size);
        self.size = size;
        self.layout_context = context;

        if let Some(proportions) = self.proportions.as_ref() {
            // Views past the end of the saved list, or without a proportion, keep their size.
            let proportion_of = |index: usize| proportions.get(index).copied().flatten();
            let mut available = size;
            let mut total = 0.0;

            for (index, item) in self.view_items.iter_mut().enumerate() {
                match proportion_of(index) {
                    Some(proportion) => total += proportion,
                    None => {
                        // A view that keeps its size still keeps within its bounds, such as one
                        // restored at a size outside them.
                        item.size = clamp(item.size, item.minimum_size(), item.maximum_size());
                        available -= item.size;
                    }
                }
            }

            for (index, item) in self.view_items.iter_mut().enumerate() {
                if let Some(proportion) = proportion_of(index).filter(|_| total > 0.0) {
                    item.size = clamp(
                        js_round(proportion * available / total),
                        item.minimum_size(),
                        item.maximum_size(),
                    );
                }
            }
        } else if let Some(last) = self.len().checked_sub(1) {
            let (low, high) = self.priority_indexes(None);
            self.resize(
                last,
                size - previous_size,
                None,
                Priorities {
                    low: &low,
                    high: &high,
                },
                Limits::default(),
                Snaps::default(),
            );
        }

        self.distribute_empty_space(None);
        self.layout_views();
    }

    /// Resizes the view at `index` to `size`, taking space from or giving it to the others.
    /// Out-of-bounds indexes are ignored, as upstream does.
    pub fn resize_view(&mut self, index: usize, size: f64) {
        let Some(item) = self.view_items.get(index) else {
            return;
        };
        let size = js_round(size);
        let size = clamp(
            size,
            item.minimum_size(),
            item.maximum_size().min(self.size),
        );

        let (mut low, high) = self.priority_indexes(Some(index));
        low.push(index);
        self.view_items[index].size = size;
        self.relayout(&low, &high, false);
    }

    /// Whether every other view is at its minimum size. Out-of-bounds indexes return `false`.
    pub fn is_view_expanded(&self, index: usize) -> bool {
        if index >= self.len() {
            return false;
        }
        self.view_items
            .iter()
            .enumerate()
            .all(|(i, item)| i == index || item.size <= item.minimum_size())
    }

    /// Distributes the whole split view size evenly among the views that can flex.
    pub fn distribute_view_sizes(&mut self) {
        let flexible: Vec<usize> = self
            .view_items
            .iter()
            .enumerate()
            .filter(|(_, item)| item.maximum_size() - item.minimum_size() > 0.0)
            .map(|(i, _)| i)
            .collect();
        let flexible_size: f64 = flexible.iter().map(|&i| self.view_items[i].size).sum();
        let size = (flexible_size / flexible.len() as f64).floor();

        for index in flexible {
            let item = &mut self.view_items[index];
            item.size = clamp(size, item.minimum_size(), item.maximum_size());
        }

        let all: Vec<usize> = (0..self.len()).collect();
        let low = self.with_priority(&all, LayoutPriority::Low);
        let high = self.with_priority(&all, LayoutPriority::High);
        self.relayout(&low, &high, false);
    }

    /// The size of the view at `index`, or `-1.0` when out of bounds, as upstream returns.
    pub fn view_size(&self, index: usize) -> f64 {
        self.view_items.get(index).map_or(-1.0, |item| item.size)
    }

    /// Starts a drag on the sash at `index` from position `start`. `alt` selects the Alt-drag
    /// behaviour.
    ///
    /// # Panics
    /// If `index` is not a sash index.
    pub fn sash_start(&mut self, index: usize, start: f64, alt: bool) {
        assert!(index < self.sash_states.len(), "index out of bounds");
        self.sash_drag = Some(self.sash_drag_from(index, start, alt));
    }

    /// Continues the active drag to position `current`. Does nothing without an active drag.
    pub fn sash_change(&mut self, current: f64) {
        let Some(drag) = self.sash_drag.as_mut() else {
            return;
        };
        drag.current = current;
        let index = drag.index;
        let delta = current - drag.start;
        let sizes = drag.sizes.clone();
        let alt = drag.alt;
        let limits = drag.limits;
        let snaps = drag.snaps;

        let new_delta = self.resize(
            index,
            delta,
            Some(sizes),
            Priorities::default(),
            limits,
            snaps,
        );

        if alt {
            let is_last = index + 1 == self.sash_states.len();
            let new_sizes = self.sizes();
            let view_index = if is_last { index } else { index + 1 };
            let item = &self.view_items[view_index];
            let limits = Limits {
                min: item.size - item.maximum_size(),
                max: item.size - item.minimum_size(),
            };
            let resize_index = if is_last {
                index.checked_sub(1)
            } else {
                Some(index + 1)
            };

            if let Some(resize_index) = resize_index {
                self.resize(
                    resize_index,
                    -new_delta,
                    Some(new_sizes),
                    Priorities::default(),
                    limits,
                    Snaps::default(),
                );
            }
        }

        self.distribute_empty_space(None);
        self.layout_views();
    }

    /// Ends the drag on the sash at `index`, fires [`SplitView::on_did_sash_change`] and saves
    /// the proportions.
    pub fn sash_end(&mut self, index: usize) {
        self.sash_change.fire(index);
        self.sash_drag = None;
        self.save_proportions();
    }

    /// Reports that Alt was pressed or released during a drag. The drag restarts from the current
    /// position with the new Alt behaviour, as upstream does. Does nothing without a drag.
    pub fn set_sash_alt(&mut self, alt: bool) {
        if let Some(drag) = &self.sash_drag {
            let (index, current) = (drag.index, drag.current);
            self.sash_drag = Some(self.sash_drag_from(index, current, alt));
        }
    }

    /// Reports a double click on the sash at `index`. Fires [`SplitView::on_did_sash_reset`]
    /// unless the reset would land on a hidden view that a snap should keep hidden.
    pub fn reset_sash(&mut self, index: usize) {
        let up = up_indexes(index);
        let down = down_indexes(index, self.len());
        let snap_before = self.find_first_snap_index(&up);
        let snap_after = self.find_first_snap_index(&down);

        if snap_before.is_some_and(|i| !self.view_items[i].visible())
            || snap_after.is_some_and(|i| !self.view_items[i].visible())
        {
            return;
        }

        self.sash_reset.fire(index);
    }

    fn checked(&self, index: usize) -> usize {
        assert!(index < self.len(), "index out of bounds");
        index
    }

    fn sizes(&self) -> Vec<f64> {
        self.view_items.iter().map(|item| item.size).collect()
    }

    fn items_size(&self) -> f64 {
        self.view_items.iter().map(|item| item.size).sum()
    }

    /// Indexes of the views with the given priority, in index order.
    fn with_priority(&self, indexes: &[usize], priority: LayoutPriority) -> Vec<usize> {
        indexes
            .iter()
            .copied()
            .filter(|&i| self.view_items[i].priority() == priority)
            .collect()
    }

    /// The low and high priority indexes of all views except `exclude`, in index order.
    fn priority_indexes(&self, exclude: Option<usize>) -> (Vec<usize>, Vec<usize>) {
        let indexes: Vec<usize> = (0..self.len()).filter(|&i| Some(i) != exclude).collect();
        (
            self.with_priority(&indexes, LayoutPriority::Low),
            self.with_priority(&indexes, LayoutPriority::High),
        )
    }

    /// Replaces `Auto` with the concrete strategy it stands for at this moment.
    fn resolve_auto(&self, sizing: Sizing) -> Sizing {
        match sizing {
            Sizing::Auto(_) if self.are_views_distributed() => Sizing::Distribute,
            Sizing::Auto(index) => Sizing::Split(index),
            other => other,
        }
    }

    fn do_add_view(&mut self, view: V, size: ViewSize, index: usize, skip_layout: bool) {
        assert!(index <= self.len(), "index out of bounds");

        let sizing = match size {
            ViewSize::Pixels(pixels) => ViewSize::Pixels(pixels),
            ViewSize::Sizing(sizing) => ViewSize::Sizing(self.resolve_auto(sizing)),
        };

        let (initial_size, cached_visible_size) = match sizing {
            ViewSize::Pixels(pixels) => (pixels, None),
            ViewSize::Sizing(Sizing::Split(reference)) => (self.view_size(reference) / 2.0, None),
            ViewSize::Sizing(Sizing::Invisible(cached)) => (0.0, Some(cached)),
            ViewSize::Sizing(Sizing::Distribute | Sizing::Auto(_)) => (view.minimum_size(), None),
        };

        self.view_items.insert(
            index,
            ViewItem {
                view,
                size: initial_size,
                cached_visible_size,
            },
        );

        if self.len() > 1 {
            self.sash_states
                .insert(index.saturating_sub(1), SashState::Disabled);
        }

        if !skip_layout {
            let high = match sizing {
                ViewSize::Sizing(Sizing::Split(reference)) => vec![reference],
                _ => Vec::new(),
            };
            let distribute = sizing == ViewSize::Sizing(Sizing::Distribute);
            self.relayout(&[index], &high, distribute);
            if distribute {
                self.distribute_view_sizes();
            }
        }
    }

    /// Makes the split view's content add up to its size, then lays out the views.
    ///
    /// When `skip_layout` is set, the views are only laid out if one of them needs the
    /// intermediate layout, as upstream decides.
    fn relayout(&mut self, low: &[usize], high: &[usize], skip_layout: bool) {
        let delta = self.size - self.items_size();

        if let Some(last) = self.len().checked_sub(1) {
            self.resize(
                last,
                delta,
                None,
                Priorities { low, high },
                Limits::default(),
                Snaps::default(),
            );
        }
        self.distribute_empty_space(None);

        if !skip_layout
            || !self
                .view_items
                .iter()
                .all(|item| item.view.path_independent_layout())
        {
            self.layout_views();
            self.save_proportions();
        }
    }

    /// Moves `delta` pixels of space across the views around the sash at `index`, giving the
    /// neighbours in priority order, and returns the delta actually applied.
    ///
    /// `sizes` is the snapshot to read sizes from (the current sizes when `None`). Snaps may
    /// hide a view, in which case the call repeats with the same snapshot and no snaps.
    fn resize(
        &mut self,
        index: usize,
        mut delta: f64,
        sizes: Option<Vec<f64>>,
        priorities: Priorities<'_>,
        limits: Limits,
        snaps: Snaps,
    ) -> f64 {
        if index >= self.len() {
            return 0.0;
        }

        let sizes = sizes.unwrap_or_else(|| self.sizes());
        let (up, down) = self.resize_order(index, priorities);

        let min_delta_up: f64 = up
            .iter()
            .map(|&i| self.view_items[i].minimum_size() - sizes[i])
            .sum();
        let max_delta_up: f64 = up
            .iter()
            .map(|&i| self.view_items[i].maximum_size() - sizes[i])
            .sum();
        let (max_delta_down, min_delta_down) = if down.is_empty() {
            (f64::INFINITY, f64::NEG_INFINITY)
        } else {
            (
                down.iter()
                    .map(|&i| sizes[i] - self.view_items[i].minimum_size())
                    .sum(),
                down.iter()
                    .map(|&i| sizes[i] - self.view_items[i].maximum_size())
                    .sum(),
            )
        };
        let min_delta = min_delta_up.max(min_delta_down).max(limits.min);
        let max_delta = max_delta_down.min(max_delta_up).min(limits.max);

        let mut snapped = false;

        if let Some(snap) = snaps.before {
            let visible = delta >= snap.limit_delta;
            let view = &mut self.view_items[snap.index];
            snapped = visible != view.visible();
            view.set_visible(visible, Some(snap.size));
        }

        if !snapped {
            if let Some(snap) = snaps.after {
                let visible = delta < snap.limit_delta;
                let view = &mut self.view_items[snap.index];
                snapped = visible != view.visible();
                view.set_visible(visible, Some(snap.size));
            }
        }

        if snapped {
            return self.resize(
                index,
                delta,
                Some(sizes),
                priorities,
                limits,
                Snaps::default(),
            );
        }

        delta = clamp(delta, min_delta, max_delta);

        let mut delta_up = delta;
        for &i in &up {
            let item = &mut self.view_items[i];
            let size = clamp(
                sizes[i] + delta_up,
                item.minimum_size(),
                item.maximum_size(),
            );
            delta_up -= size - sizes[i];
            item.size = size;
        }

        let mut delta_down = delta;
        for &i in &down {
            let item = &mut self.view_items[i];
            let size = clamp(
                sizes[i] - delta_down,
                item.minimum_size(),
                item.maximum_size(),
            );
            delta_down += size - sizes[i];
            item.size = size;
        }

        delta
    }

    /// The order in which `resize` visits the views before and after the sash at `index`.
    fn resize_order(&self, index: usize, priorities: Priorities<'_>) -> (Vec<usize>, Vec<usize>) {
        let mut up = up_indexes(index);
        let mut down = down_indexes(index, self.len());

        for &i in priorities.high {
            push_to_start(&mut up, i);
            push_to_start(&mut down, i);
        }
        for &i in priorities.low {
            push_to_end(&mut up, i);
            push_to_end(&mut down, i);
        }

        (up, down)
    }

    /// Spreads the unused space (or takes the overflow) over the views, respecting priorities.
    /// `low_priority_index` is resized last, whatever its priority.
    fn distribute_empty_space(&mut self, low_priority_index: Option<usize>) {
        let mut empty = self.size - self.items_size();

        let mut indexes: Vec<usize> = (0..self.len()).rev().collect();
        let low = self.with_priority(&indexes, LayoutPriority::Low);
        let high = self.with_priority(&indexes, LayoutPriority::High);

        for index in high {
            push_to_start(&mut indexes, index);
        }
        for index in low {
            push_to_end(&mut indexes, index);
        }
        if let Some(index) = low_priority_index {
            push_to_end(&mut indexes, index);
        }

        for index in indexes {
            if empty == 0.0 {
                break;
            }
            let item = &mut self.view_items[index];
            let size = clamp(item.size + empty, item.minimum_size(), item.maximum_size());
            empty -= size - item.size;
            item.size = size;
        }
    }

    fn layout_views(&mut self) {
        self.content_size = self.items_size();

        let mut offset = 0.0;
        for item in &mut self.view_items {
            item.view
                .layout(item.size, offset, self.layout_context.as_ref());
            offset += item.size;
        }

        self.update_sash_enablement();
    }

    fn save_proportions(&mut self) {
        if self.proportional_layout && self.content_size > 0.0 {
            self.proportions = Some(
                self.view_items
                    .iter()
                    .map(|item| {
                        (item.view.proportional_layout() && item.visible())
                            .then(|| item.size / self.content_size)
                    })
                    .collect(),
            );
        }
    }

    /// Derives every sash's state from the views around it. Port of `updateSashEnablement`.
    fn update_sash_enablement(&mut self) {
        let collapses: Vec<bool> = self
            .view_items
            .iter()
            .map(|item| item.size - item.minimum_size() > 0.0)
            .collect();
        let expands: Vec<bool> = self
            .view_items
            .iter()
            .map(|item| item.maximum_size() - item.size > 0.0)
            .collect();
        let collapses_down = running_or(&collapses, false);
        let expands_down = running_or(&expands, false);
        let collapses_up = running_or(&collapses, true);
        let expands_up = running_or(&expands, true);

        let mut position = 0.0;
        for index in 0..self.sash_states.len() {
            position += self.view_items[index].size;

            let min = !(collapses_down[index] && expands_up[index + 1]);
            let max = !(expands_down[index] && collapses_up[index + 1]);

            let state = if min && max {
                let snapped_before = self
                    .find_first_snap_index(&up_indexes(index))
                    .is_some_and(|i| !self.view_items[i].visible());
                let snapped_after = self
                    .find_first_snap_index(&down_indexes(index, self.len()))
                    .is_some_and(|i| !self.view_items[i].visible());

                if snapped_before
                    && collapses_up[index]
                    && (position > 0.0 || self.start_snapping_enabled)
                {
                    SashState::AtMinimum
                } else if snapped_after
                    && collapses_down[index]
                    && (position < self.content_size || self.end_snapping_enabled)
                {
                    SashState::AtMaximum
                } else {
                    SashState::Disabled
                }
            } else if min {
                SashState::AtMinimum
            } else if max {
                SashState::AtMaximum
            } else {
                SashState::Enabled
            };

            self.sash_states[index] = state;
        }
    }

    /// The first view in `indexes` that snaps, preferring visible views. Hidden views count only
    /// when no visible view before them still has room to shrink. Port of `findFirstSnapIndex`.
    fn find_first_snap_index(&self, indexes: &[usize]) -> Option<usize> {
        for &index in indexes {
            let item = &self.view_items[index];
            if item.visible() && item.snap() {
                return Some(index);
            }
        }

        for &index in indexes {
            let item = &self.view_items[index];
            if item.visible() && item.maximum_size() - item.minimum_size() > 0.0 {
                return None;
            }
            if !item.visible() && item.snap() {
                return Some(index);
            }
        }

        None
    }

    fn are_views_distributed(&self) -> bool {
        let mut range: Option<(f64, f64)> = None;
        for item in &self.view_items {
            let (min, max) = range.map_or((item.size, item.size), |(min, max)| {
                (min.min(item.size), max.max(item.size))
            });
            if max - min > 2.0 {
                return false;
            }
            range = Some((min, max));
        }
        true
    }

    fn sash_drag_from(&self, index: usize, start: f64, alt: bool) -> SashDrag {
        let sizes = self.sizes();
        let alt = alt != self.inverse_alt_behavior;
        let mut limits = Limits::default();
        let mut snaps = Snaps::default();

        if alt {
            // Alt resizes the views on both sides of the sash equally. The last sash has no view
            // after it to borrow from, so the view before it moves the other way.
            if index + 1 == self.sash_states.len() {
                let item = &self.view_items[index];
                limits = Limits {
                    min: (item.minimum_size() - item.size) / 2.0,
                    max: (item.maximum_size() - item.size) / 2.0,
                };
            } else {
                let item = &self.view_items[index + 1];
                limits = Limits {
                    min: (item.size - item.maximum_size()) / 2.0,
                    max: (item.size - item.minimum_size()) / 2.0,
                };
            }
        } else {
            let up = up_indexes(index);
            let down = down_indexes(index, self.len());
            let min_delta_up: f64 = up
                .iter()
                .map(|&i| self.view_items[i].minimum_size() - sizes[i])
                .sum();
            let max_delta_up: f64 = up
                .iter()
                .map(|&i| self.view_items[i].view_maximum_size() - sizes[i])
                .sum();
            let (max_delta_down, min_delta_down) = if down.is_empty() {
                (f64::INFINITY, f64::NEG_INFINITY)
            } else {
                (
                    down.iter()
                        .map(|&i| sizes[i] - self.view_items[i].minimum_size())
                        .sum(),
                    down.iter()
                        .map(|&i| sizes[i] - self.view_items[i].view_maximum_size())
                        .sum(),
                )
            };
            // Upstream keeps the unbounded limits above for a non-Alt drag; these bounds only
            // place the snap thresholds.
            let min_delta = min_delta_up.max(min_delta_down);
            let max_delta = max_delta_down.min(max_delta_up);

            snaps.before = self.find_first_snap_index(&up).map(|i| {
                let item = &self.view_items[i];
                let half = (item.view_minimum_size() / 2.0).floor();
                Snap {
                    index: i,
                    limit_delta: if item.visible() {
                        min_delta - half
                    } else {
                        min_delta + half
                    },
                    size: item.size,
                }
            });
            snaps.after = self.find_first_snap_index(&down).map(|i| {
                let item = &self.view_items[i];
                let half = (item.view_minimum_size() / 2.0).floor();
                Snap {
                    index: i,
                    limit_delta: if item.visible() {
                        max_delta + half
                    } else {
                        max_delta - half
                    },
                    size: item.size,
                }
            });
        }

        SashDrag {
            index,
            start,
            current: start,
            sizes,
            limits,
            alt,
            snaps,
        }
    }
}

/// One view and its sizing state.
struct ViewItem<V: View> {
    view: V,
    size: f64,
    cached_visible_size: Option<f64>,
}

impl<V: View> ViewItem<V> {
    fn visible(&self) -> bool {
        self.cached_visible_size.is_none()
    }

    /// Shows or hides the item. Hiding caches `size` (or the given size) and collapses to zero.
    fn set_visible(&mut self, visible: bool, size: Option<f64>) {
        if visible == self.visible() {
            return;
        }

        if visible {
            let cached = self.cached_visible_size.unwrap_or(0.0);
            self.size = clamp(cached, self.view.minimum_size(), self.view.maximum_size());
            self.cached_visible_size = None;
        } else {
            self.cached_visible_size = Some(size.unwrap_or(self.size));
            self.size = 0.0;
        }

        self.view.set_visible(visible);
    }

    /// The view's minimum size, or `0` while hidden.
    fn minimum_size(&self) -> f64 {
        if self.visible() {
            self.view.minimum_size()
        } else {
            0.0
        }
    }

    /// The view's maximum size, or `0` while hidden.
    fn maximum_size(&self) -> f64 {
        if self.visible() {
            self.view.maximum_size()
        } else {
            0.0
        }
    }

    /// The view's own minimum size, even while hidden.
    fn view_minimum_size(&self) -> f64 {
        self.view.minimum_size()
    }

    /// The view's own maximum size, even while hidden.
    fn view_maximum_size(&self) -> f64 {
        self.view.maximum_size()
    }

    fn priority(&self) -> LayoutPriority {
        self.view.priority()
    }

    fn snap(&self) -> bool {
        self.view.snap()
    }
}

/// Which views a resize visits first (high) or last (low). Both lists are in visit order.
#[derive(Clone, Copy, Default)]
struct Priorities<'a> {
    low: &'a [usize],
    high: &'a [usize],
}

/// Extra bounds on the delta of a resize, used by Alt-drags.
#[derive(Clone, Copy)]
struct Limits {
    min: f64,
    max: f64,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            min: f64::NEG_INFINITY,
            max: f64::INFINITY,
        }
    }
}

/// A view that a drag may hide, once the delta crosses `limit_delta`.
#[derive(Clone, Copy)]
struct Snap {
    index: usize,
    limit_delta: f64,
    size: f64,
}

#[derive(Clone, Copy, Default)]
struct Snaps {
    before: Option<Snap>,
    after: Option<Snap>,
}

/// The state of an active sash drag. Port of `ISashDragState`.
struct SashDrag {
    index: usize,
    start: f64,
    current: f64,
    sizes: Vec<f64>,
    limits: Limits,
    alt: bool,
    snaps: Snaps,
}

/// Indexes from `index` back to the first view, nearest first.
fn up_indexes(index: usize) -> Vec<usize> {
    (0..=index).rev().collect()
}

/// Indexes from just after `index` to the last of `len` views, in index order.
fn down_indexes(index: usize, len: usize) -> Vec<usize> {
    (index + 1..len).collect()
}

/// Moves `value` to the front of `values` if it is present. Port of `pushToStart`.
fn push_to_start(values: &mut Vec<usize>, value: usize) {
    if let Some(position) = values.iter().position(|&v| v == value) {
        values.remove(position);
        values.insert(0, value);
    }
}

/// Moves `value` to the back of `values` if it is present. Port of `pushToEnd`.
fn push_to_end(values: &mut Vec<usize>, value: usize) {
    if let Some(position) = values.iter().position(|&v| v == value) {
        values.remove(position);
        values.push(value);
    }
}

/// Prefix (or, reversed, suffix) OR of `flags`: entry `i` is true when any flag at or before
/// (or at or after) `i` is true.
fn running_or(flags: &[bool], from_end: bool) -> Vec<bool> {
    let mut result = Vec::with_capacity(flags.len());
    let mut previous = false;
    if from_end {
        for &flag in flags.iter().rev() {
            previous = flag || previous;
            result.push(previous);
        }
        result.reverse();
    } else {
        for &flag in flags {
            previous = flag || previous;
            result.push(previous);
        }
    }
    result
}

/// Clamps `value` into `[min, max]`. Port of `clamp` in `common/numbers.ts`.
fn clamp(value: f64, min: f64, max: f64) -> f64 {
    value.max(min).min(max)
}

/// Rounds half up, like JavaScript's `Math.round` (Rust's `round` rounds half away from zero).
fn js_round(value: f64) -> f64 {
    (value + 0.5).floor()
}
