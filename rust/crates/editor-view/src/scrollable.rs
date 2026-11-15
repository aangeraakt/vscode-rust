//! Scroll state, scroll events and smooth-scroll animation.
//!
//! Ported from `src/vs/base/common/scrollable.ts` (`ScrollState`, `Scrollable`,
//! `SmoothScrollingOperation`, `SmoothScrollingUpdate`).
//!
//! Divergences from the TypeScript:
//! - TypeScript schedules each animation frame through an injected callback. Here the caller
//!   drives frames: it calls [`Scrollable::perform_smooth_scrolling`] while the function returns
//!   `true`. Time comes from an injectable [`Clock`] instead of `Date.now()`.
//! - Scroll events go through `vscode_api::Emitter` rather than the TypeScript `Emitter`.
//! - The TypeScript `| 0` integer forcing is `trunc`, which matches it for values in range.

use std::time::{SystemTime, UNIX_EPOCH};

use vscode_api::{Emitter, Event};

/// Source of the current time in milliseconds. Replaces `Date.now()` so tests can run animations
/// deterministically.
pub trait Clock {
    /// The current time in milliseconds.
    fn now_ms(&self) -> f64;
}

/// The wall clock.
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now_ms(&self) -> f64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0.0, |d| d.as_secs_f64() * 1000.0)
    }
}

/// Dimensions of a scrollable area (`IScrollDimensions`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScrollDimensions {
    /// Visible width.
    pub width: f64,
    /// Total scrollable width.
    pub scroll_width: f64,
    /// Visible height.
    pub height: f64,
    /// Total scrollable height.
    pub scroll_height: f64,
}

/// A partial dimensions update (`INewScrollDimensions`).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct NewScrollDimensions {
    /// New visible width, if changed.
    pub width: Option<f64>,
    /// New total width, if changed.
    pub scroll_width: Option<f64>,
    /// New visible height, if changed.
    pub height: Option<f64>,
    /// New total height, if changed.
    pub scroll_height: Option<f64>,
}

/// A scroll position (`IScrollPosition`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScrollPosition {
    /// Horizontal position.
    pub scroll_left: f64,
    /// Vertical position.
    pub scroll_top: f64,
}

/// A partial position update (`INewScrollPosition`).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct NewScrollPosition {
    /// New horizontal position, if changed.
    pub scroll_left: Option<f64>,
    /// New vertical position, if changed.
    pub scroll_top: Option<f64>,
}

/// A position plus the viewport it was measured in (`ISmoothScrollPosition`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SmoothScrollPosition {
    /// Horizontal position.
    pub scroll_left: f64,
    /// Vertical position.
    pub scroll_top: f64,
    /// Viewport width, used to size the animation.
    pub width: f64,
    /// Viewport height, used to size the animation.
    pub height: f64,
}

impl SmoothScrollPosition {
    fn from_state(state: &ScrollState) -> Self {
        Self {
            scroll_left: state.scroll_left,
            scroll_top: state.scroll_top,
            width: state.width,
            height: state.height,
        }
    }
}

/// A scroll position and its dimensions, validated and clamped (`ScrollState`).
///
/// Clamping keeps the viewport inside the scrollable area: a negative size becomes 0, and the
/// position is pulled back so `position + size` does not pass `scroll size`, then to 0 at the
/// low end. The raw positions are kept before clamping, so a later resize can restore them.
#[derive(Debug, Clone, Copy)]
pub struct ScrollState {
    force_integer_values: bool,
    /// Horizontal position before clamping.
    pub raw_scroll_left: f64,
    /// Vertical position before clamping.
    pub raw_scroll_top: f64,
    /// Visible width.
    pub width: f64,
    /// Total scrollable width.
    pub scroll_width: f64,
    /// Horizontal position after clamping.
    pub scroll_left: f64,
    /// Visible height.
    pub height: f64,
    /// Total scrollable height.
    pub scroll_height: f64,
    /// Vertical position after clamping.
    pub scroll_top: f64,
}

impl ScrollState {
    /// Creates a state and clamps it. With `force_integer_values`, all inputs are truncated first.
    pub fn new(
        force_integer_values: bool,
        width: f64,
        scroll_width: f64,
        scroll_left: f64,
        height: f64,
        scroll_height: f64,
        scroll_top: f64,
    ) -> Self {
        let trunc = |v: f64| if force_integer_values { v.trunc() } else { v };
        let scroll_width = trunc(scroll_width);
        let scroll_height = trunc(scroll_height);
        let (mut width, mut scroll_left) = (trunc(width), trunc(scroll_left));
        let (mut height, mut scroll_top) = (trunc(height), trunc(scroll_top));

        let raw_scroll_left = scroll_left;
        let raw_scroll_top = scroll_top;

        if width < 0.0 {
            width = 0.0;
        }
        if scroll_left + width > scroll_width {
            scroll_left = scroll_width - width;
        }
        if scroll_left < 0.0 {
            scroll_left = 0.0;
        }

        if height < 0.0 {
            height = 0.0;
        }
        if scroll_top + height > scroll_height {
            scroll_top = scroll_height - height;
        }
        if scroll_top < 0.0 {
            scroll_top = 0.0;
        }

        Self {
            force_integer_values,
            raw_scroll_left,
            raw_scroll_top,
            width,
            scroll_width,
            scroll_left,
            height,
            scroll_height,
            scroll_top,
        }
    }

    /// Whether two states hold the same raw and clamped values.
    pub fn equals(&self, other: &ScrollState) -> bool {
        self.raw_scroll_left == other.raw_scroll_left
            && self.raw_scroll_top == other.raw_scroll_top
            && self.width == other.width
            && self.scroll_width == other.scroll_width
            && self.scroll_left == other.scroll_left
            && self.height == other.height
            && self.scroll_height == other.scroll_height
            && self.scroll_top == other.scroll_top
    }

    /// A new state with some dimensions replaced and the positions re-clamped. With
    /// `use_raw_scroll_positions`, the raw positions are used instead of the clamped ones.
    pub fn with_scroll_dimensions(
        &self,
        update: NewScrollDimensions,
        use_raw_scroll_positions: bool,
    ) -> Self {
        Self::new(
            self.force_integer_values,
            update.width.unwrap_or(self.width),
            update.scroll_width.unwrap_or(self.scroll_width),
            if use_raw_scroll_positions {
                self.raw_scroll_left
            } else {
                self.scroll_left
            },
            update.height.unwrap_or(self.height),
            update.scroll_height.unwrap_or(self.scroll_height),
            if use_raw_scroll_positions {
                self.raw_scroll_top
            } else {
                self.scroll_top
            },
        )
    }

    /// A new state with the position replaced and clamped. Unset fields keep the raw position.
    pub fn with_scroll_position(&self, update: NewScrollPosition) -> Self {
        Self::new(
            self.force_integer_values,
            self.width,
            self.scroll_width,
            update.scroll_left.unwrap_or(self.raw_scroll_left),
            self.height,
            self.scroll_height,
            update.scroll_top.unwrap_or(self.raw_scroll_top),
        )
    }

    /// The event for moving from `previous` to `self`.
    pub fn create_scroll_event(
        &self,
        previous: &ScrollState,
        in_smooth_scrolling: bool,
    ) -> ScrollEvent {
        ScrollEvent {
            in_smooth_scrolling,
            old_width: previous.width,
            old_scroll_width: previous.scroll_width,
            old_scroll_left: previous.scroll_left,
            width: self.width,
            scroll_width: self.scroll_width,
            scroll_left: self.scroll_left,
            old_height: previous.height,
            old_scroll_height: previous.scroll_height,
            old_scroll_top: previous.scroll_top,
            height: self.height,
            scroll_height: self.scroll_height,
            scroll_top: self.scroll_top,
            width_changed: self.width != previous.width,
            scroll_width_changed: self.scroll_width != previous.scroll_width,
            scroll_left_changed: self.scroll_left != previous.scroll_left,
            height_changed: self.height != previous.height,
            scroll_height_changed: self.scroll_height != previous.scroll_height,
            scroll_top_changed: self.scroll_top != previous.scroll_top,
        }
    }
}

/// Fired when a scrollable's state changes (`ScrollEvent`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScrollEvent {
    /// Whether the change came from a smooth-scroll frame.
    pub in_smooth_scrolling: bool,
    /// Previous visible width.
    pub old_width: f64,
    /// Previous total width.
    pub old_scroll_width: f64,
    /// Previous horizontal position.
    pub old_scroll_left: f64,
    /// Visible width.
    pub width: f64,
    /// Total width.
    pub scroll_width: f64,
    /// Horizontal position.
    pub scroll_left: f64,
    /// Previous visible height.
    pub old_height: f64,
    /// Previous total height.
    pub old_scroll_height: f64,
    /// Previous vertical position.
    pub old_scroll_top: f64,
    /// Visible height.
    pub height: f64,
    /// Total height.
    pub scroll_height: f64,
    /// Vertical position.
    pub scroll_top: f64,
    /// Whether the visible width changed.
    pub width_changed: bool,
    /// Whether the total width changed.
    pub scroll_width_changed: bool,
    /// Whether the horizontal position changed.
    pub scroll_left_changed: bool,
    /// Whether the visible height changed.
    pub height_changed: bool,
    /// Whether the total height changed.
    pub scroll_height_changed: bool,
    /// Whether the vertical position changed.
    pub scroll_top_changed: bool,
}

/// Options for [`Scrollable::new`] (`IScrollableOptions`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScrollableOptions {
    /// Whether scroll values are always integers.
    pub force_integer_values: bool,
    /// Duration in milliseconds of smooth scroll animations. 0 disables them.
    pub smooth_scroll_duration: f64,
}

/// One frame of a smooth scroll (`SmoothScrollingUpdate`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SmoothScrollingUpdate {
    /// Horizontal position for this frame.
    pub scroll_left: f64,
    /// Vertical position for this frame.
    pub scroll_top: f64,
    /// Whether the animation has reached its target.
    pub is_done: bool,
}

/// A scroll position with a state and an optional smooth animation (`Scrollable`).
///
/// The caller owns the frame loop. After a call to `set_scroll_position_smooth`, it calls
/// [`Scrollable::perform_smooth_scrolling`] on each frame while the animation runs.
pub struct Scrollable {
    smooth_scroll_duration: f64,
    state: ScrollState,
    smooth_scrolling: Option<SmoothScrollingOperation>,
    on_scroll: Emitter<ScrollEvent>,
}

impl Scrollable {
    /// Creates a scrollable with an empty state.
    pub fn new(options: ScrollableOptions) -> Self {
        Self {
            smooth_scroll_duration: options.smooth_scroll_duration,
            state: ScrollState::new(options.force_integer_values, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0),
            smooth_scrolling: None,
            on_scroll: Emitter::new(),
        }
    }

    /// Fires when the scroll state changes.
    pub fn on_scroll(&self) -> Event<ScrollEvent> {
        self.on_scroll.event()
    }

    /// Changes the duration of smooth scroll animations.
    pub fn set_smooth_scroll_duration(&mut self, smooth_scroll_duration: f64) {
        self.smooth_scroll_duration = smooth_scroll_duration;
    }

    /// The position `scroll_position` would have after clamping.
    pub fn validate_scroll_position(&self, scroll_position: NewScrollPosition) -> ScrollPosition {
        let state = self.state.with_scroll_position(scroll_position);
        ScrollPosition {
            scroll_left: state.scroll_left,
            scroll_top: state.scroll_top,
        }
    }

    /// The current scroll state.
    pub fn scroll_dimensions(&self) -> ScrollState {
        self.state
    }

    /// Updates the dimensions and re-clamps the position. A running animation is adjusted to the
    /// new limits.
    pub fn set_scroll_dimensions(
        &mut self,
        dimensions: NewScrollDimensions,
        use_raw_scroll_positions: bool,
    ) {
        let new_state = self
            .state
            .with_scroll_dimensions(dimensions, use_raw_scroll_positions);
        let in_smooth_scrolling = self.smooth_scrolling.is_some();
        self.set_state(new_state, in_smooth_scrolling);
        if let Some(operation) = self.smooth_scrolling.as_mut() {
            operation.accept_scroll_dimensions(&self.state);
        }
    }

    /// The final position once any running animation ends. Without one, the current position.
    pub fn future_scroll_position(&self) -> ScrollPosition {
        match &self.smooth_scrolling {
            Some(operation) => ScrollPosition {
                scroll_left: operation.to.scroll_left,
                scroll_top: operation.to.scroll_top,
            },
            None => ScrollPosition {
                scroll_left: self.state.scroll_left,
                scroll_top: self.state.scroll_top,
            },
        }
    }

    /// The current position. During an animation this may be an intermediate frame.
    pub fn current_scroll_position(&self) -> ScrollPosition {
        ScrollPosition {
            scroll_left: self.state.scroll_left,
            scroll_top: self.state.scroll_top,
        }
    }

    /// Jumps to `update` and cancels any running animation.
    pub fn set_scroll_position_now(&mut self, update: NewScrollPosition) {
        let new_state = self.state.with_scroll_position(update);
        self.smooth_scrolling = None;
        self.set_state(new_state, false);
    }

    /// Animates to `update`. With `reuse_animation`, a running animation keeps its start time
    /// and only its target changes. Without it, the new animation starts from the current
    /// position.
    pub fn set_scroll_position_smooth(
        &mut self,
        update: NewScrollPosition,
        reuse_animation: bool,
        clock: &dyn Clock,
    ) {
        if self.smooth_scroll_duration == 0.0 {
            return self.set_scroll_position_now(update);
        }
        let now = clock.now_ms();

        if let Some(current) = self.smooth_scrolling.as_ref() {
            // Fill in the parts of `update` that are unset with the pending target.
            let target = NewScrollPosition {
                scroll_left: Some(update.scroll_left.unwrap_or(current.to.scroll_left)),
                scroll_top: Some(update.scroll_top.unwrap_or(current.to.scroll_top)),
            };
            let valid = self.state.with_scroll_position(target);
            if current.to.scroll_left == valid.scroll_left
                && current.to.scroll_top == valid.scroll_top
            {
                // Already heading there.
                return;
            }
            let valid_target = SmoothScrollPosition::from_state(&valid);
            let next = if reuse_animation {
                SmoothScrollingOperation::new(
                    current.from,
                    valid_target,
                    current.start_time,
                    current.duration,
                )
            } else {
                SmoothScrollingOperation::start(
                    SmoothScrollPosition::from_state(&self.state),
                    valid_target,
                    self.smooth_scroll_duration,
                    now,
                )
            };
            self.smooth_scrolling = Some(next);
        } else {
            let valid = self.state.with_scroll_position(update);
            self.smooth_scrolling = Some(SmoothScrollingOperation::start(
                SmoothScrollPosition::from_state(&self.state),
                SmoothScrollPosition::from_state(&valid),
                self.smooth_scroll_duration,
                now,
            ));
        }
    }

    /// Whether an animation is running.
    pub fn has_pending_scroll_animation(&self) -> bool {
        self.smooth_scrolling.is_some()
    }

    /// Advances the running animation to the current clock time. Returns `true` while the
    /// animation still has frames to run, so the caller should schedule the next one.
    pub fn perform_smooth_scrolling(&mut self, clock: &dyn Clock) -> bool {
        let update = match self.smooth_scrolling.as_ref() {
            Some(operation) => operation.tick_at(clock.now_ms()),
            None => return false,
        };
        let new_state = self.state.with_scroll_position(NewScrollPosition {
            scroll_left: Some(update.scroll_left),
            scroll_top: Some(update.scroll_top),
        });
        self.set_state(new_state, true);

        // A listener may have cancelled the animation while it was firing.
        if self.smooth_scrolling.is_none() {
            return false;
        }
        if update.is_done {
            self.smooth_scrolling = None;
            return false;
        }
        true
    }

    fn set_state(&mut self, new_state: ScrollState, in_smooth_scrolling: bool) {
        let old_state = self.state;
        if old_state.equals(&new_state) {
            return;
        }
        self.state = new_state;
        self.on_scroll.fire(
            self.state
                .create_scroll_event(&old_state, in_smooth_scrolling),
        );
    }
}

/// Eased position at a given completion. `Composed` joins two eases at `cut`.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Animation {
    Ease {
        from: f64,
        to: f64,
    },
    Composed {
        first: (f64, f64),
        second: (f64, f64),
        cut: f64,
    },
}

fn ease_out_cubic(t: f64) -> f64 {
    1.0 - (1.0 - t).powf(3.0)
}

fn ease(endpoints: (f64, f64), completion: f64) -> f64 {
    let (from, to) = endpoints;
    from + (to - from) * ease_out_cubic(completion)
}

impl Animation {
    fn at(self, completion: f64) -> f64 {
        match self {
            Animation::Ease { from, to } => ease((from, to), completion),
            Animation::Composed { first, second, cut } => {
                if completion < cut {
                    ease(first, completion / cut)
                } else {
                    ease(second, (completion - cut) / (1.0 - cut))
                }
            }
        }
    }
}

fn init_animation(from: f64, to: f64, viewport_size: f64) -> Animation {
    let delta = (from - to).abs();
    if delta > 2.5 * viewport_size {
        // Scroll to 75% of the viewport, jump, then ease into the target.
        let (stop1, stop2) = if from < to {
            (from + 0.75 * viewport_size, to - 0.75 * viewport_size)
        } else {
            (from - 0.75 * viewport_size, to + 0.75 * viewport_size)
        };
        Animation::Composed {
            first: (from, stop1),
            second: (stop2, to),
            cut: 0.33,
        }
    } else {
        Animation::Ease { from, to }
    }
}

/// One smooth scroll from `from` to `to` (`SmoothScrollingOperation`).
#[derive(Debug, Clone, PartialEq)]
pub struct SmoothScrollingOperation {
    /// Where the animation started.
    pub from: SmoothScrollPosition,
    /// Where the animation ends. Updated if the scroll dimensions change.
    pub to: SmoothScrollPosition,
    /// Duration in milliseconds.
    pub duration: f64,
    /// Start time in milliseconds, from the clock.
    pub start_time: f64,
    scroll_left: Animation,
    scroll_top: Animation,
}

impl SmoothScrollingOperation {
    /// Creates an operation with the given start time and duration. See [`Self::start`] for the
    /// version that adds the lead-in used by scrolls.
    pub fn new(
        from: SmoothScrollPosition,
        to: SmoothScrollPosition,
        start_time: f64,
        duration: f64,
    ) -> Self {
        let (scroll_left, scroll_top) = Self::animations(from, to);
        Self {
            from,
            to,
            duration,
            start_time,
            scroll_left,
            scroll_top,
        }
    }

    /// Starts an operation at `now`. The duration is extended by 10 ms and the start is moved
    /// back 10 ms, so the first frame already shows some motion.
    pub fn start(
        from: SmoothScrollPosition,
        to: SmoothScrollPosition,
        duration: f64,
        now: f64,
    ) -> Self {
        Self::new(from, to, now - 10.0, duration + 10.0)
    }

    fn animations(from: SmoothScrollPosition, to: SmoothScrollPosition) -> (Animation, Animation) {
        (
            init_animation(from.scroll_left, to.scroll_left, to.width),
            init_animation(from.scroll_top, to.scroll_top, to.height),
        )
    }

    /// Re-targets to the clamped `to` for new dimensions and rebuilds the animations.
    pub fn accept_scroll_dimensions(&mut self, state: &ScrollState) {
        let valid = state.with_scroll_position(NewScrollPosition {
            scroll_left: Some(self.to.scroll_left),
            scroll_top: Some(self.to.scroll_top),
        });
        self.to = SmoothScrollPosition::from_state(&valid);
        let (scroll_left, scroll_top) = Self::animations(self.from, self.to);
        self.scroll_left = scroll_left;
        self.scroll_top = scroll_top;
    }

    /// The position at `now`, in milliseconds. Done once `now` reaches the end time.
    pub fn tick_at(&self, now: f64) -> SmoothScrollingUpdate {
        let completion = (now - self.start_time) / self.duration;
        if completion < 1.0 {
            SmoothScrollingUpdate {
                scroll_left: self.scroll_left.at(completion),
                scroll_top: self.scroll_top.at(completion),
                is_done: false,
            }
        } else {
            SmoothScrollingUpdate {
                scroll_left: self.to.scroll_left,
                scroll_top: self.to.scroll_top,
                is_done: true,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    //! Ports of `src/vs/base/test/common/scrollable.test.ts`, plus clamping checks for the
    //! `ScrollState` behavior of `scrollable.ts`.

    use super::*;

    const VIEWPORT_HEIGHT: f64 = 800.0;
    const ANIMATION_DURATION: f64 = 125.0;
    const LINE_HEIGHT: f64 = 20.0;

    /// Mirrors the test subclass: the lead-in is applied to the constructor arguments.
    fn operation(from: f64, to: f64) -> SmoothScrollingOperation {
        let start = SmoothScrollPosition {
            scroll_left: 0.0,
            scroll_top: from,
            width: 0.0,
            height: VIEWPORT_HEIGHT,
        };
        let target = SmoothScrollPosition {
            scroll_top: to,
            ..start
        };
        SmoothScrollingOperation::new(start, target, 0.0 - 10.0, ANIMATION_DURATION + 10.0)
    }

    fn extract_lines(operation: &SmoothScrollingOperation, now: f64) -> (i64, i64) {
        let scroll_top = operation.tick_at(now).scroll_top;
        let scroll_bottom = scroll_top + VIEWPORT_HEIGHT;
        let start = (scroll_top / LINE_HEIGHT).floor() as i64;
        let end = (scroll_bottom / LINE_HEIGHT).ceil() as i64;
        (start, end)
    }

    fn simulate(from: f64, to: f64) -> Vec<(i64, i64)> {
        let op = operation(from, to);
        [0.0, 25.0, 50.0, 75.0, 100.0, 125.0]
            .iter()
            .map(|&now| extract_lines(&op, now))
            .collect()
    }

    #[test]
    fn scroll_25_lines_40_fit() {
        assert_eq!(
            simulate(0.0, 500.0),
            [(5, 46), (14, 55), (20, 61), (23, 64), (24, 65), (25, 65)]
        );
    }

    #[test]
    fn scroll_75_lines_40_fit() {
        assert_eq!(
            simulate(0.0, 1500.0),
            [
                (15, 56),
                (44, 85),
                (62, 103),
                (71, 112),
                (74, 115),
                (75, 115)
            ]
        );
    }

    #[test]
    fn scroll_100_lines_40_fit() {
        assert_eq!(
            simulate(0.0, 2000.0),
            [
                (20, 61),
                (59, 100),
                (82, 123),
                (94, 135),
                (99, 140),
                (100, 140)
            ]
        );
    }

    #[test]
    fn scroll_125_lines_40_fit() {
        assert_eq!(
            simulate(0.0, 2500.0),
            [
                (16, 57),
                (29, 70),
                (107, 148),
                (119, 160),
                (124, 165),
                (125, 165)
            ]
        );
    }

    #[test]
    fn scroll_500_lines_40_fit() {
        assert_eq!(
            simulate(0.0, 10000.0),
            [
                (16, 57),
                (29, 70),
                (482, 523),
                (494, 535),
                (499, 540),
                (500, 540)
            ]
        );
    }

    #[test]
    fn scroll_state_clamps_position_into_the_scrollable_area() {
        // Position past the end is pulled back so the viewport ends at scroll_height.
        let state = ScrollState::new(false, 100.0, 1000.0, 2000.0, 100.0, 500.0, 450.0);
        assert_eq!((state.scroll_left, state.scroll_top), (900.0, 400.0));
        assert_eq!(
            (state.raw_scroll_left, state.raw_scroll_top),
            (2000.0, 450.0)
        );
        // Negative size and position are clamped to 0.
        let state = ScrollState::new(false, -5.0, 1000.0, -20.0, -1.0, 500.0, -3.0);
        assert_eq!((state.width, state.height), (0.0, 0.0));
        assert_eq!((state.scroll_left, state.scroll_top), (0.0, 0.0));
    }

    #[test]
    fn scroll_state_forces_integers_when_asked() {
        let state = ScrollState::new(true, 100.7, 1000.2, 10.9, 80.5, 500.0, 20.4);
        assert_eq!((state.width, state.height), (100.0, 80.0));
        assert_eq!((state.scroll_left, state.scroll_top), (10.0, 20.0));
    }

    #[test]
    fn set_scroll_position_now_emits_one_event_and_clamps() {
        let mut scrollable = Scrollable::new(ScrollableOptions {
            force_integer_values: false,
            smooth_scroll_duration: 0.0,
        });
        scrollable.set_scroll_dimensions(
            NewScrollDimensions {
                width: Some(100.0),
                scroll_width: Some(100.0),
                height: Some(100.0),
                scroll_height: Some(500.0),
            },
            false,
        );
        let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let sink = seen.clone();
        let _subscription = scrollable
            .on_scroll()
            .subscribe(move |e| sink.lock().unwrap().push(*e));
        scrollable.set_scroll_position_now(NewScrollPosition {
            scroll_left: None,
            scroll_top: Some(450.0),
        });
        assert_eq!(scrollable.current_scroll_position().scroll_top, 400.0);
        let events = seen.lock().unwrap();
        assert_eq!(events.len(), 1);
        assert!(events[0].scroll_top_changed);
        assert!(!events[0].in_smooth_scrolling);
    }

    #[test]
    fn smooth_scroll_runs_to_the_target_through_the_clock() {
        struct FixedClock(std::cell::Cell<f64>);
        impl Clock for FixedClock {
            fn now_ms(&self) -> f64 {
                self.0.get()
            }
        }
        let clock = FixedClock(std::cell::Cell::new(0.0));
        let mut scrollable = Scrollable::new(ScrollableOptions {
            force_integer_values: false,
            smooth_scroll_duration: ANIMATION_DURATION,
        });
        scrollable.set_scroll_dimensions(
            NewScrollDimensions {
                width: Some(100.0),
                scroll_width: Some(100.0),
                height: Some(VIEWPORT_HEIGHT),
                scroll_height: Some(10000.0),
            },
            false,
        );
        scrollable.set_scroll_position_smooth(
            NewScrollPosition {
                scroll_left: None,
                scroll_top: Some(500.0),
            },
            false,
            &clock,
        );
        assert!(scrollable.has_pending_scroll_animation());
        assert_eq!(scrollable.future_scroll_position().scroll_top, 500.0);

        for frame in 0..100 {
            clock.0.set(f64::from(frame) * 25.0);
            if !scrollable.perform_smooth_scrolling(&clock) {
                break;
            }
            assert!(frame < 99, "animation did not finish");
        }
        assert!(!scrollable.has_pending_scroll_animation());
        assert_eq!(scrollable.current_scroll_position().scroll_top, 500.0);
    }
}
