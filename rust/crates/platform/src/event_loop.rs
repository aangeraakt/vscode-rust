//! Window and event loop runner on winit 0.30.
//!
//! ADR 0002 makes winit responsible for windows and input. ADR 0005 runs the event loop on the
//! UI thread and returns I/O results as user events. Nothing above this module names a winit
//! type: winit events are translated into [`PlatformEvent`], and [`EventSender`] is the only
//! way for other threads to wake the loop.
//!
//! The loop must be created and run on the main thread, which winit requires on some platforms.
//! Tests in this crate never create an event loop or a window, because CI is headless. They test
//! the translation in [`map_window_event`] and the pure window state model. The runner is
//! checked by running the `lathe` binary.

use std::fmt;

use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop as WinitEventLoop, EventLoopProxy};
use winit::window::{Window as WinitWindow, WindowId as WinitWindowId};

use crate::window::WindowConfig;
use crate::window_state::{Display, Rect};

/// Errors from the platform layer.
#[derive(Debug)]
pub enum PlatformError {
    /// The event loop could not be created or failed while running.
    EventLoop(winit::error::EventLoopError),
    /// A native window could not be created.
    Window(winit::error::OsError),
}

impl fmt::Display for PlatformError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EventLoop(error) => write!(f, "event loop error: {error}"),
            Self::Window(error) => write!(f, "window error: {error}"),
        }
    }
}

impl std::error::Error for PlatformError {}

/// Lathe's id for a native window, opaque to callers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WindowId(u64);

impl From<WinitWindowId> for WindowId {
    fn from(id: WinitWindowId) -> Self {
        Self(u64::from(id))
    }
}

/// Events the platform layer forwards to the application. Coordinates are physical pixels,
/// as winit reports them. Window state (`window_state`) uses logical pixels.
#[derive(Debug, Clone, PartialEq)]
pub enum PlatformEvent<U> {
    /// The event loop started or resumed. Windows are created from here.
    Resumed,
    /// The user asked to close a window. The application decides whether it closes.
    CloseRequested(WindowId),
    /// The content area changed size.
    Resized {
        /// The window that changed.
        window: WindowId,
        /// New content width in physical pixels.
        width: u32,
        /// New content height in physical pixels.
        height: u32,
    },
    /// The window moved.
    Moved {
        /// The window that moved.
        window: WindowId,
        /// New left edge in physical pixels.
        x: i32,
        /// New top edge in physical pixels.
        y: i32,
    },
    /// The window gained or lost focus.
    Focused {
        /// The window whose focus changed.
        window: WindowId,
        /// Whether the window now has focus.
        focused: bool,
    },
    /// A result from I/O running on another thread (ADR 0005).
    User(U),
}

/// Translates a winit window event. Returns `None` for events that have no `PlatformEvent`
/// yet (`WindowEvent` is non-exhaustive, so unknown variants are ignored here).
pub fn map_window_event<U>(window: WinitWindowId, event: &WindowEvent) -> Option<PlatformEvent<U>> {
    let window = WindowId::from(window);
    match event {
        WindowEvent::CloseRequested => Some(PlatformEvent::CloseRequested(window)),
        WindowEvent::Resized(size) => Some(PlatformEvent::Resized {
            window,
            width: size.width,
            height: size.height,
        }),
        WindowEvent::Moved(position) => Some(PlatformEvent::Moved {
            window,
            x: position.x,
            y: position.y,
        }),
        WindowEvent::Focused(focused) => Some(PlatformEvent::Focused {
            window,
            focused: *focused,
        }),
        _ => None,
    }
}

/// A native window. Created by [`LoopControl::create_window`] on the UI thread.
#[derive(Debug)]
pub struct Window {
    inner: WinitWindow,
}

impl Window {
    /// The id that [`PlatformEvent`]s for this window carry.
    pub fn id(&self) -> WindowId {
        WindowId::from(self.inner.id())
    }

    /// Asks the platform to send a redraw event for this window.
    pub fn request_redraw(&self) {
        self.inner.request_redraw();
    }
}

/// Handle used inside the event loop callback: exit, create windows, list displays.
#[derive(Debug)]
pub struct LoopControl<'a> {
    active: &'a ActiveEventLoop,
}

impl LoopControl<'_> {
    /// Stops the event loop after the current callback returns.
    pub fn exit(&self) {
        self.active.exit();
    }

    /// Creates a native window described by `config`.
    pub fn create_window(&self, config: &WindowConfig) -> Result<Window, PlatformError> {
        self.active
            .create_window(config.attributes())
            .map(|inner| Window { inner })
            .map_err(PlatformError::Window)
    }

    /// The connected displays in logical pixels, for
    /// [`crate::window_state::validate_window_state`].
    ///
    /// Two limits follow from winit 0.30. The work area is not available, so it equals the
    /// display bounds, and taskbars are not excluded. Display ids are the position in winit's
    /// monitor list, which changes when monitors are connected or disconnected.
    pub fn displays(&self) -> Vec<Display> {
        self.active
            .available_monitors()
            .enumerate()
            .map(|(index, monitor)| {
                let scale = monitor.scale_factor();
                let position = monitor.position().to_logical::<f64>(scale);
                let size = monitor.size().to_logical::<f64>(scale);
                let bounds = Rect {
                    x: position.x.round() as i32,
                    y: position.y.round() as i32,
                    width: size.width.round() as u32,
                    height: size.height.round() as u32,
                };
                Display {
                    id: index as i64,
                    bounds,
                    work_area: bounds,
                }
            })
            .collect()
    }
}

/// Sends events to the event loop from any thread. Clones are cheap.
pub struct EventSender<U: 'static>(EventLoopProxy<U>);

impl<U: 'static> Clone for EventSender<U> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<U: 'static> fmt::Debug for EventSender<U> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EventSender").finish_non_exhaustive()
    }
}

impl<U: Send + 'static> EventSender<U> {
    /// Delivers `event` as [`PlatformEvent::User`]. Fails once the loop has exited, and the
    /// event is dropped then.
    pub fn send(&self, event: U) -> Result<(), EventLoopClosed> {
        self.0.send_event(event).map_err(|_| EventLoopClosed)
    }
}

/// The event loop has exited, so the event could not be delivered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EventLoopClosed;

impl fmt::Display for EventLoopClosed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("the event loop has exited")
    }
}

impl std::error::Error for EventLoopClosed {}

/// The event loop with user events of type `U`.
pub struct EventLoop<U: 'static> {
    inner: WinitEventLoop<U>,
}

impl<U: 'static> EventLoop<U> {
    /// Creates the event loop. Call on the main thread.
    pub fn new() -> Result<Self, PlatformError> {
        WinitEventLoop::<U>::with_user_event()
            .build()
            .map(|inner| Self { inner })
            .map_err(PlatformError::EventLoop)
    }

    /// A sender that other threads use to deliver user events to this loop.
    pub fn sender(&self) -> EventSender<U> {
        EventSender(self.inner.create_proxy())
    }

    /// Runs the loop until it exits. `handler` receives every [`PlatformEvent`] on the calling
    /// thread.
    pub fn run<F>(self, handler: F) -> Result<(), PlatformError>
    where
        F: FnMut(PlatformEvent<U>, LoopControl<'_>),
    {
        let mut runner = Runner { handler };
        self.inner
            .run_app(&mut runner)
            .map_err(PlatformError::EventLoop)
    }
}

/// Adapts a closure to winit's `ApplicationHandler`.
struct Runner<F> {
    handler: F,
}

impl<U: 'static, F> ApplicationHandler<U> for Runner<F>
where
    F: FnMut(PlatformEvent<U>, LoopControl<'_>),
{
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        (self.handler)(PlatformEvent::Resumed, LoopControl { active: event_loop });
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WinitWindowId,
        event: WindowEvent,
    ) {
        if let Some(event) = map_window_event(window_id, &event) {
            (self.handler)(event, LoopControl { active: event_loop });
        }
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: U) {
        (self.handler)(
            PlatformEvent::User(event),
            LoopControl { active: event_loop },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use winit::dpi::{PhysicalPosition, PhysicalSize};

    fn window() -> WinitWindowId {
        WinitWindowId::dummy()
    }

    #[test]
    fn close_request_maps_to_close_event() {
        let event: Option<PlatformEvent<()>> =
            map_window_event(window(), &WindowEvent::CloseRequested);
        assert_eq!(
            event,
            Some(PlatformEvent::CloseRequested(WindowId::from(window())))
        );
    }

    #[test]
    fn resize_and_move_keep_physical_values() {
        let resized: Option<PlatformEvent<()>> = map_window_event(
            window(),
            &WindowEvent::Resized(PhysicalSize::new(1920, 1080)),
        );
        assert_eq!(
            resized,
            Some(PlatformEvent::Resized {
                window: WindowId::from(window()),
                width: 1920,
                height: 1080,
            })
        );

        let moved: Option<PlatformEvent<()>> = map_window_event(
            window(),
            &WindowEvent::Moved(PhysicalPosition::new(-10, 20)),
        );
        assert_eq!(
            moved,
            Some(PlatformEvent::Moved {
                window: WindowId::from(window()),
                x: -10,
                y: 20,
            })
        );
    }

    #[test]
    fn focus_maps_both_directions() {
        for focused in [true, false] {
            let event: Option<PlatformEvent<()>> =
                map_window_event(window(), &WindowEvent::Focused(focused));
            assert_eq!(
                event,
                Some(PlatformEvent::Focused {
                    window: WindowId::from(window()),
                    focused,
                })
            );
        }
    }

    #[test]
    fn unhandled_window_events_are_dropped() {
        let event: Option<PlatformEvent<()>> =
            map_window_event(window(), &WindowEvent::RedrawRequested);
        assert_eq!(event, None);
    }

    #[test]
    fn sender_is_send_sync_and_clone_for_send_user_events() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<EventSender<String>>();
        fn assert_clone<T: Clone>() {}
        assert_clone::<EventSender<String>>();
    }
}
