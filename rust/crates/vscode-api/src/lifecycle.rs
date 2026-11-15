//! Disposables and lifecycle phases.
//!
//! Ported from:
//! - `src/vs/base/common/lifecycle.ts` (`IDisposable`, `toDisposable`, `DisposableStore`)
//! - `src/vs/workbench/services/lifecycle/common/lifecycle.ts` (`LifecyclePhase`, `ShutdownReason`)
//!
//! Divergences from the TypeScript, by design:
//! - [`Disposable`] is RAII: dropping it disposes it. In TypeScript an ignored disposable keeps
//!   its registration forever; in Rust call [`Disposable::leak`] to get that behavior.
//! - Adding to an already disposed [`DisposableStore`] disposes the item at once. TypeScript
//!   leaks it and warns.

use std::fmt;

/// A handle to a registration (listener, command, provider, ...) that is undone when the handle
/// is disposed or dropped. Disposing twice is a no-op.
#[must_use = "dropping a Disposable disposes it immediately; call `leak()` to keep the registration"]
pub struct Disposable {
    on_dispose: Option<Box<dyn FnOnce() + Send>>,
}

impl Disposable {
    /// Creates a disposable that runs `f` once when disposed (`toDisposable`).
    pub fn new(f: impl FnOnce() + Send + 'static) -> Self {
        Self {
            on_dispose: Some(Box::new(f)),
        }
    }

    /// A disposable that does nothing (`Disposable.None`).
    pub fn none() -> Self {
        Self { on_dispose: None }
    }

    /// Undoes the registration now. Later calls and the eventual drop do nothing.
    pub fn dispose(&mut self) {
        if let Some(f) = self.on_dispose.take() {
            f();
        }
    }

    /// Whether this disposable has already run (or never had anything to run).
    pub fn is_disposed(&self) -> bool {
        self.on_dispose.is_none()
    }

    /// Keeps the registration for the life of the process.
    pub fn leak(mut self) {
        self.on_dispose = None;
    }
}

impl Drop for Disposable {
    fn drop(&mut self) {
        self.dispose();
    }
}

impl fmt::Debug for Disposable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Disposable")
            .field("disposed", &self.is_disposed())
            .finish()
    }
}

/// The single-threaded counterpart of [`Disposable`]: same semantics, but its closure need not be
/// `Send`, so it can capture `Rc` state. Returned by [`crate::event::LocalEvent::subscribe`] for
/// UI-thread listeners (ADR 0007). It cannot be converted into a [`Disposable`].
#[must_use = "dropping a LocalDisposable disposes it immediately; call `leak()` to keep the registration"]
pub struct LocalDisposable {
    on_dispose: Option<Box<dyn FnOnce()>>,
}

impl LocalDisposable {
    /// Creates a disposable that runs `f` once when disposed.
    pub fn new(f: impl FnOnce() + 'static) -> Self {
        Self {
            on_dispose: Some(Box::new(f)),
        }
    }

    /// A disposable that does nothing.
    pub fn none() -> Self {
        Self { on_dispose: None }
    }

    /// Undoes the registration now. Later calls and the eventual drop do nothing.
    pub fn dispose(&mut self) {
        if let Some(f) = self.on_dispose.take() {
            f();
        }
    }

    /// Whether this disposable has already run (or never had anything to run).
    pub fn is_disposed(&self) -> bool {
        self.on_dispose.is_none()
    }

    /// Keeps the registration for the life of the process.
    pub fn leak(mut self) {
        self.on_dispose = None;
    }
}

impl Drop for LocalDisposable {
    fn drop(&mut self) {
        self.dispose();
    }
}

impl fmt::Debug for LocalDisposable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LocalDisposable")
            .field("disposed", &self.is_disposed())
            .finish()
    }
}

/// A collection of disposables that are disposed together, in insertion order
/// (`DisposableStore`). Dropping the store disposes it.
#[derive(Debug, Default)]
pub struct DisposableStore {
    items: Vec<Disposable>,
    disposed: bool,
}

impl DisposableStore {
    /// Creates an empty store.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a disposable. If the store is already disposed, `item` is disposed immediately.
    pub fn add(&mut self, mut item: Disposable) {
        if self.disposed {
            item.dispose();
        } else {
            self.items.push(item);
        }
    }

    /// Disposes all items, in insertion order, and marks the store as disposed.
    pub fn dispose(&mut self) {
        self.disposed = true;
        self.clear();
    }

    /// Disposes all items, in insertion order, but keeps the store usable.
    pub fn clear(&mut self) {
        for mut item in std::mem::take(&mut self.items) {
            item.dispose();
        }
    }

    /// Whether [`DisposableStore::dispose`] has been called.
    pub fn is_disposed(&self) -> bool {
        self.disposed
    }

    /// Number of items currently held.
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Whether the store holds no items.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

impl Drop for DisposableStore {
    fn drop(&mut self) {
        self.dispose();
    }
}

/// Startup phases of a window, in order. Numeric values match VS Code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum LifecyclePhase {
    /// Services are being constructed; nothing is rendered yet.
    Starting = 1,
    /// Services are ready and the window is about to restore UI state.
    Ready = 2,
    /// Views, panels and editors have restored.
    Restored = 3,
    /// A few seconds after `Restored`, for non-urgent work.
    Eventually = 4,
}

/// Why a window shuts down. Numeric values match VS Code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ShutdownReason {
    /// The window is closed.
    Close = 1,
    /// The application quits.
    Quit = 2,
    /// The window reloads.
    Reload = 3,
    /// The window loads a different workspace.
    Load = 4,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    fn recorder() -> (Arc<Mutex<Vec<u32>>>, impl Fn(u32) -> Disposable) {
        let log = Arc::new(Mutex::new(Vec::new()));
        let log2 = log.clone();
        (log, move |n| {
            let log = log2.clone();
            Disposable::new(move || log.lock().unwrap().push(n))
        })
    }

    #[test]
    fn disposable_runs_once_and_on_drop() {
        let (log, make) = recorder();
        let mut d = make(1);
        d.dispose();
        d.dispose();
        drop(d);
        drop(make(2));
        assert_eq!(*log.lock().unwrap(), vec![1, 2]);
    }

    #[test]
    fn leak_keeps_registration() {
        let (log, make) = recorder();
        make(1).leak();
        assert!(log.lock().unwrap().is_empty());
    }

    #[test]
    fn store_disposes_in_insertion_order() {
        let (log, make) = recorder();
        let mut store = DisposableStore::new();
        store.add(make(1));
        store.add(make(2));
        store.add(make(3));
        store.dispose();
        assert_eq!(*log.lock().unwrap(), vec![1, 2, 3]);
        assert!(store.is_disposed());
    }

    #[test]
    fn adding_to_disposed_store_disposes_immediately() {
        let (log, make) = recorder();
        let mut store = DisposableStore::new();
        store.dispose();
        store.add(make(7));
        assert_eq!(*log.lock().unwrap(), vec![7]);
        assert!(store.is_empty());
    }

    #[test]
    fn clear_keeps_store_usable() {
        let (log, make) = recorder();
        let mut store = DisposableStore::new();
        store.add(make(1));
        store.clear();
        store.add(make(2));
        assert_eq!(store.len(), 1);
        drop(store);
        assert_eq!(*log.lock().unwrap(), vec![1, 2]);
    }

    #[test]
    fn phases_are_ordered_like_vscode() {
        assert!(LifecyclePhase::Starting < LifecyclePhase::Eventually);
        assert_eq!(LifecyclePhase::Restored as i32, 3);
        assert_eq!(ShutdownReason::Load as i32, 4);
    }
}
