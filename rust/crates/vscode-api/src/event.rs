//! Event emitter.
//!
//! Ported from `src/vs/base/common/event.ts` (`Emitter`, `Event`).
//!
//! Matched behavior (each has a test below):
//! - listeners run synchronously in subscription order;
//! - a listener added while an event is being delivered does not receive that event;
//! - a listener removed while an event is being delivered does not receive it if it has not run;
//! - a panicking listener does not stop delivery to the remaining listeners (TypeScript reports
//!   the error through `onUnexpectedError`; here the first panic resumes after delivery ends);
//! - after `dispose`, firing is a no-op and new subscriptions are inert.
//!
//! Known divergence: when `fire` is called again from inside a listener, TypeScript finishes
//! delivering the outer event to the remaining listeners first. Here the nested event is delivered
//! immediately, before the outer delivery continues.

use crate::lifecycle::Disposable;
use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};
use std::sync::{Arc, Mutex, MutexGuard, Weak};

type Listener<T> = Arc<dyn Fn(&T) + Send + Sync>;

struct Inner<T> {
    next_id: u64,
    listeners: Vec<(u64, Listener<T>)>,
    disposed: bool,
}

fn lock<T>(m: &Mutex<Inner<T>>) -> MutexGuard<'_, Inner<T>> {
    // Listeners never run while the lock is held, so a poisoned lock holds consistent data.
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// The owner side of an event: fires values to subscribers.
pub struct Emitter<T: 'static> {
    inner: Arc<Mutex<Inner<T>>>,
}

/// The subscriber side of an event. Cheap to clone; hand it out instead of the [`Emitter`].
pub struct Event<T: 'static> {
    inner: Arc<Mutex<Inner<T>>>,
}

impl<T: 'static> Clone for Event<T> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl<T: 'static> Default for Emitter<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: 'static> Emitter<T> {
    /// Creates an emitter with no listeners.
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(Inner {
                next_id: 0,
                listeners: Vec::new(),
                disposed: false,
            })),
        }
    }

    /// The subscriber handle for this emitter.
    pub fn event(&self) -> Event<T> {
        Event {
            inner: self.inner.clone(),
        }
    }

    /// Subscribes directly on the emitter. Same as `self.event().subscribe(listener)`.
    pub fn subscribe(&self, listener: impl Fn(&T) + Send + Sync + 'static) -> Disposable {
        subscribe(&self.inner, Arc::new(listener))
    }

    /// Delivers `event` to every listener subscribed at the time of the call.
    pub fn fire(&self, event: T) {
        let snapshot: Vec<(u64, Listener<T>)> = {
            let inner = lock(&self.inner);
            if inner.disposed {
                return;
            }
            inner.listeners.clone()
        };
        let mut first_panic = None;
        for (id, listener) in snapshot {
            let still_subscribed = lock(&self.inner).listeners.iter().any(|(i, _)| *i == id);
            if !still_subscribed {
                continue;
            }
            if let Err(payload) = catch_unwind(AssertUnwindSafe(|| listener(&event))) {
                first_panic.get_or_insert(payload);
            }
        }
        if let Some(payload) = first_panic {
            resume_unwind(payload);
        }
    }

    /// Whether any listener is subscribed.
    pub fn has_listeners(&self) -> bool {
        !lock(&self.inner).listeners.is_empty()
    }

    /// Removes all listeners; later `fire` calls and subscriptions do nothing.
    pub fn dispose(&self) {
        let removed = {
            let mut inner = lock(&self.inner);
            inner.disposed = true;
            std::mem::take(&mut inner.listeners)
        };
        drop(removed); // listener captures are dropped outside the lock
    }
}

impl<T: 'static> Event<T> {
    /// Subscribes `listener`. The returned [`Disposable`] unsubscribes it.
    pub fn subscribe(&self, listener: impl Fn(&T) + Send + Sync + 'static) -> Disposable {
        subscribe(&self.inner, Arc::new(listener))
    }
}

fn subscribe<T: 'static>(inner: &Arc<Mutex<Inner<T>>>, listener: Listener<T>) -> Disposable {
    let id = {
        let mut guard = lock(inner);
        if guard.disposed {
            return Disposable::none();
        }
        let id = guard.next_id;
        guard.next_id += 1;
        guard.listeners.push((id, listener));
        id
    };
    let weak: Weak<Mutex<Inner<T>>> = Arc::downgrade(inner);
    Disposable::new(move || {
        if let Some(inner) = weak.upgrade() {
            let removed = {
                let mut guard = lock(&inner);
                let pos = guard.listeners.iter().position(|(i, _)| *i == id);
                pos.map(|p| guard.listeners.remove(p))
            };
            drop(removed);
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn log() -> Arc<Mutex<Vec<String>>> {
        Arc::new(Mutex::new(Vec::new()))
    }

    #[test]
    fn delivers_in_subscription_order_until_disposed() {
        let emitter = Emitter::<i32>::new();
        let seen = log();
        let (a, b) = (seen.clone(), seen.clone());
        let da = emitter
            .event()
            .subscribe(move |e| a.lock().unwrap().push(format!("a{e}")));
        let _db = emitter.subscribe(move |e| b.lock().unwrap().push(format!("b{e}")));
        emitter.fire(1);
        drop(da);
        emitter.fire(2);
        assert_eq!(*seen.lock().unwrap(), ["a1", "b1", "b2"]);
    }

    #[test]
    fn listener_added_during_fire_misses_current_event() {
        let emitter = Arc::new(Emitter::<i32>::new());
        let seen = log();
        let late = Arc::new(Mutex::new(Vec::<Disposable>::new()));
        let (em, s, l) = (emitter.clone(), seen.clone(), late.clone());
        let _d = emitter.subscribe(move |e| {
            let s = s.clone();
            l.lock()
                .unwrap()
                .push(em.subscribe(move |e2| s.lock().unwrap().push(format!("late{e2}"))));
            let _ = e;
        });
        emitter.fire(1);
        assert!(seen.lock().unwrap().is_empty());
        emitter.fire(2);
        assert_eq!(*seen.lock().unwrap(), ["late2"]);
    }

    #[test]
    fn listener_removed_during_fire_is_skipped() {
        let emitter = Emitter::<i32>::new();
        let seen = log();
        let second: Arc<Mutex<Option<Disposable>>> = Arc::new(Mutex::new(None));
        let s2 = second.clone();
        let _first = emitter.subscribe(move |_| {
            if let Some(mut d) = s2.lock().unwrap().take() {
                d.dispose();
            }
        });
        let s = seen.clone();
        *second.lock().unwrap() =
            Some(emitter.subscribe(move |e| s.lock().unwrap().push(e.to_string())));
        emitter.fire(1);
        assert!(seen.lock().unwrap().is_empty());
    }

    #[test]
    fn panicking_listener_does_not_block_others() {
        let emitter = Emitter::<i32>::new();
        let seen = log();
        let _a = emitter.subscribe(|_| panic!("boom"));
        let s = seen.clone();
        let _b = emitter.subscribe(move |e| s.lock().unwrap().push(e.to_string()));
        let result = catch_unwind(AssertUnwindSafe(|| emitter.fire(5)));
        assert!(result.is_err());
        assert_eq!(*seen.lock().unwrap(), ["5"]);
    }

    #[test]
    fn dispose_stops_delivery_and_new_subscriptions() {
        let emitter = Emitter::<i32>::new();
        let seen = log();
        let s = seen.clone();
        let _d = emitter.subscribe(move |e| s.lock().unwrap().push(e.to_string()));
        emitter.dispose();
        emitter.fire(1);
        let s = seen.clone();
        let d2 = emitter.subscribe(move |e| s.lock().unwrap().push(e.to_string()));
        assert!(d2.is_disposed());
        emitter.fire(2);
        assert!(seen.lock().unwrap().is_empty());
        assert!(!emitter.has_listeners());
    }

    #[test]
    fn unsubscribe_after_emitter_dropped_is_safe() {
        let emitter = Emitter::<i32>::new();
        let d = emitter.subscribe(|_| {});
        drop(emitter);
        drop(d);
    }
}
