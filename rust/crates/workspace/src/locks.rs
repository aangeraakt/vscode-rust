//! Per-resource async locks, the counterpart of `createResourceLock` in
//! `src/vs/platform/files/node/diskFileSystemProvider.ts`.
//!
//! A slot exists only while someone holds or waits for it. Slots are kept as `Weak` references
//! and pruned on each acquisition, so the map does not grow with every path ever written.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, PoisonError, Weak};

use tokio::sync::{Mutex as AsyncMutex, OwnedMutexGuard};

/// Serializes operations on the same resource key.
#[derive(Default)]
pub(crate) struct ResourceLocks {
    slots: Mutex<HashMap<String, Weak<AsyncMutex<()>>>>,
}

impl ResourceLocks {
    /// Waits until no other holder has `key`, then returns a guard that releases it on drop.
    pub(crate) async fn lock(&self, key: String) -> OwnedMutexGuard<()> {
        let slot = {
            let mut slots = self.slots.lock().unwrap_or_else(PoisonError::into_inner);
            slots.retain(|_, slot| slot.strong_count() > 0);
            match slots.get(&key).and_then(Weak::upgrade) {
                Some(slot) => slot,
                None => {
                    let slot = Arc::new(AsyncMutex::new(()));
                    slots.insert(key, Arc::downgrade(&slot));
                    slot
                }
            }
        };
        slot.lock_owned().await
    }
}
