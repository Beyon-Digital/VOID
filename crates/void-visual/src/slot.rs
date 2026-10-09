//! Single-slot mailboxes: latest-write-wins, never blocking the sender.
//! Clock snapshots and frame hand-off use these so the audio side never
//! waits on the visual engine (CONTRACTS.md: audio never waits on GPU).

use std::sync::{Arc, Mutex};

/// Latest-value mailbox. `send` overwrites, never blocks; `recv` takes.
pub struct Slot<T> {
    inner: Mutex<Option<T>>,
}

impl<T> Slot<T> {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            inner: Mutex::new(None),
        })
    }

    /// Overwrite the slot; returns the displaced value if any (count it
    /// as dropped when it was never consumed).
    pub fn send(&self, v: T) -> Option<T> {
        self.inner.lock().unwrap().replace(v)
    }

    pub fn recv(&self) -> Option<T> {
        self.inner.lock().unwrap().take()
    }

    pub fn peek_is_empty(&self) -> bool {
        self.inner.lock().unwrap().is_none()
    }
}

impl<T> Default for Slot<T> {
    fn default() -> Self {
        Self {
            inner: Mutex::new(None),
        }
    }
}
