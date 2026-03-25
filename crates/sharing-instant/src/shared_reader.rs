//! ┌─────────────────────────────────────────────────────┐
//! │  SHARED READER                                       │
//! │  Read-only shared state                              │
//! ├─────────────────────────────────────────────────────┤
//! │                                                      │
//! │  SharedReader<V>                                     │
//! │    ├── get()   → &V  (current value)                 │
//! │    ├── watch() → Receiver<V>  (reactive stream)      │
//! │    └── map()   → SharedReader<U>  (derived state)    │
//! │                                                      │
//! ├─────────────────────────────────────────────────────┤
//! │  WHY: Mirrors Swift's SharedReader. Provides safe    │
//! │  read-only access for components that shouldn't      │
//! │  mutate shared state.                                │
//! │                                                      │
//! │  TESTED BY: tests/shared_reader_tests.rs             │
//! │                                                      │
//! │  CHANGELOG:                                          │
//! │  • v0.1.0 — Initial SharedReader<V>                  │
//! │                                                      │
//! │  HISTORY: git log --oneline --follow -- crates/sharing-instant/src/shared_reader.rs │
//! └─────────────────────────────────────────────────────┘

use parking_lot::RwLock;
use std::sync::Arc;
use tokio::sync::watch;

/// Read-only view of shared state.
///
/// Mirrors Swift's `SharedReader<Value>`. Cannot mutate the value
/// but receives reactive updates when the source changes.
///
/// Created from a `Shared` via `.reader()`, or from `FetchAll`/`FetchOne`.
///
/// # Example
///
/// ```
/// use sharing_instant::shared_reader::SharedReader;
///
/// let (tx, rx) = tokio::sync::watch::channel(42);
/// let reader = SharedReader::from_watch(42, rx);
///
/// assert_eq!(*reader.get(), 42);
///
/// tx.send(100).unwrap();
/// // reader.get() will eventually reflect 100
/// ```
pub struct SharedReader<V: Send + Sync + Clone + 'static> {
    value: Arc<RwLock<V>>,
    receiver: watch::Receiver<V>,
}

impl<V: Send + Sync + Clone + 'static> SharedReader<V> {
    /// Create a SharedReader from a watch channel.
    pub fn from_watch(initial: V, receiver: watch::Receiver<V>) -> Self {
        Self {
            value: Arc::new(RwLock::new(initial)),
            receiver,
        }
    }

    /// Get the current value.
    ///
    /// Returns the most recently received value from the underlying
    /// watch channel.
    pub fn get(&self) -> parking_lot::RwLockReadGuard<'_, V> {
        // Update from receiver if changed
        if let Ok(new_val) = self.receiver.has_changed() {
            if new_val {
                let current = self.receiver.borrow().clone();
                *self.value.write() = current;
            }
        }
        self.value.read()
    }

    /// Get a reactive watch receiver.
    pub fn watch(&self) -> watch::Receiver<V> {
        self.receiver.clone()
    }

    /// Create a derived read-only view by mapping the value.
    ///
    /// # Example
    ///
    /// ```
    /// use sharing_instant::shared_reader::SharedReader;
    ///
    /// let (tx, rx) = tokio::sync::watch::channel(vec![1, 2, 3]);
    /// let reader = SharedReader::from_watch(vec![1, 2, 3], rx);
    /// let count_reader = reader.map(|v| v.len());
    ///
    /// assert_eq!(*count_reader.get(), 3);
    /// ```
    pub fn map<U: Send + Sync + Clone + 'static>(
        &self,
        f: impl Fn(&V) -> U + Send + Sync + 'static,
    ) -> SharedReader<U> {
        let f = Arc::new(f);
        let initial = f(&*self.get());
        let (tx, rx) = watch::channel(initial.clone());

        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            let mut source_rx = self.receiver.clone();
            let f_clone = f.clone();

            handle.spawn(async move {
                while source_rx.changed().await.is_ok() {
                    let new_val = f_clone(&*source_rx.borrow());
                    if tx.send(new_val).is_err() {
                        break;
                    }
                }
            });
        }

        SharedReader::from_watch(initial, rx)
    }
}

impl<V: Send + Sync + Clone + 'static> Clone for SharedReader<V> {
    fn clone(&self) -> Self {
        Self {
            value: Arc::new(RwLock::new(self.value.read().clone())),
            receiver: self.receiver.clone(),
        }
    }
}
