//! ┌─────────────────────────────────────────────────────┐
//! │  SHARED                                              │
//! │  Mutable shared state with persistence               │
//! ├─────────────────────────────────────────────────────┤
//! │                                                      │
//! │  Shared<V, K: SharedKey>                             │
//! │    ├── value: V          (current value)             │
//! │    ├── key: K            (persistence strategy)      │
//! │    ├── reference: Arc    (deduplication)             │
//! │    └── subscription      (external change tracking)  │
//! │                                                      │
//! │  Mutation: with_lock(|v| { *v = new }) → auto-save  │
//! │  Observation: watch() → Stream<V>                    │
//! │                                                      │
//! ├─────────────────────────────────────────────────────┤
//! │  WHY: Mirrors Swift's @Shared property wrapper.      │
//! │  Provides thread-safe mutable access with automatic  │
//! │  persistence and cross-component sharing.            │
//! │                                                      │
//! │  ALTERNATIVES: Arc<Mutex<V>> (no persistence),       │
//! │  message passing (no shared state semantics).        │
//! │                                                      │
//! │  TESTED BY: tests/shared_tests.rs                    │
//! │                                                      │
//! │  CHANGELOG:                                          │
//! │  • v0.1.0 — Initial Shared<V, K> wrapper             │
//! │                                                      │
//! │  HISTORY: git log --oneline --follow -- crates/sharing-instant/src/shared.rs │
//! └─────────────────────────────────────────────────────┘

use crate::error::Result;
use crate::shared_key::{SaveContext, SharedKey};
use crate::shared_reader::SharedReader;
use crate::shared_reader_key::{LoadContext, SharedSubscriber};
use crate::subscription::SharedSubscription;
use parking_lot::RwLock;
use std::sync::Arc;
use tokio::sync::watch;

/// Mutable shared state wrapper with automatic persistence.
///
/// Mirrors Swift's `@Shared` property wrapper. Wraps a value with:
/// - Thread-safe read/write access via `with_lock()`
/// - Automatic persistence via `SharedKey` on mutation
/// - Cross-component sharing via reference deduplication
/// - Reactive updates via `watch()` stream
///
/// # Example
///
/// ```
/// use sharing_instant::shared::Shared;
/// use sharing_instant::keys::in_memory_key::InMemoryKey;
///
/// let shared = Shared::new(
///     42,
///     InMemoryKey::new("counter"),
/// );
///
/// assert_eq!(*shared.get(), 42);
///
/// shared.with_lock(|v| *v = 100);
/// assert_eq!(*shared.get(), 100);
/// ```
pub struct Shared<V, K>
where
    V: Send + Sync + Clone + 'static,
    K: SharedKey<Value = V>,
{
    inner: Arc<SharedInner<V>>,
    key: Arc<K>,
    _subscription: Arc<RwLock<Option<SharedSubscription>>>,
}

struct SharedInner<V: Send + Sync + Clone + 'static> {
    value: RwLock<V>,
    sender: watch::Sender<V>,
}

impl<V, K> Shared<V, K>
where
    V: Send + Sync + Clone + 'static,
    K: SharedKey<Value = V>,
{
    /// Create a new shared value with the given persistence key.
    ///
    /// Attempts to load the initial value from the key. If loading
    /// fails or returns None, uses the provided default value.
    ///
    /// # Example
    ///
    /// ```
    /// use sharing_instant::shared::Shared;
    /// use sharing_instant::keys::in_memory_key::InMemoryKey;
    ///
    /// let shared = Shared::new(0, InMemoryKey::new("count"));
    /// assert_eq!(*shared.get(), 0);
    /// ```
    pub fn new(default: V, key: K) -> Self {
        // Try to load from the key
        let initial = key
            .load(LoadContext::InitialValue(default.clone()))
            .ok()
            .flatten()
            .unwrap_or(default);

        let (sender, _) = watch::channel(initial.clone());
        let inner = Arc::new(SharedInner {
            value: RwLock::new(initial),
            sender,
        });

        let key = Arc::new(key);

        // Set up subscription for external changes
        let inner_clone = inner.clone();
        let subscriber = SharedSubscriber::<V>::new(move |result| {
            if let Ok(Some(value)) = result {
                let mut guard = inner_clone.value.write();
                *guard = value.clone();
                let _ = inner_clone.sender.send(value);
            }
        });

        let subscription = key.subscribe(
            LoadContext::InitialValue(inner.value.read().clone()),
            subscriber,
        );

        Self {
            inner,
            key,
            _subscription: Arc::new(RwLock::new(Some(subscription))),
        }
    }

    /// Get a read-only reference to the current value.
    ///
    /// # Example
    ///
    /// ```
    /// use sharing_instant::shared::Shared;
    /// use sharing_instant::keys::in_memory_key::InMemoryKey;
    ///
    /// let shared = Shared::new("hello".to_string(), InMemoryKey::new("msg"));
    /// assert_eq!(&*shared.get(), "hello");
    /// ```
    pub fn get(&self) -> parking_lot::RwLockReadGuard<'_, V> {
        self.inner.value.read()
    }

    /// Mutate the value under a lock, then auto-persist.
    ///
    /// The closure receives a mutable reference to the current value.
    /// After the closure returns, the new value is automatically saved
    /// via the `SharedKey::save()` method and broadcast to all watchers.
    ///
    /// # Example
    ///
    /// ```
    /// use sharing_instant::shared::Shared;
    /// use sharing_instant::keys::in_memory_key::InMemoryKey;
    ///
    /// let shared = Shared::new(vec![1, 2, 3], InMemoryKey::new("list"));
    /// shared.with_lock(|v| v.push(4));
    /// assert_eq!(&*shared.get(), &[1, 2, 3, 4]);
    /// ```
    pub fn with_lock<R>(&self, f: impl FnOnce(&mut V) -> R) -> R {
        let result;
        let new_value;
        {
            let mut guard = self.inner.value.write();
            result = f(&mut guard);
            new_value = guard.clone();
        }

        // Auto-persist
        let _ = self.key.save(&new_value, SaveContext::DidSet);

        // Broadcast to watchers
        let _ = self.inner.sender.send(new_value);

        result
    }

    /// Explicitly save the current value.
    ///
    /// Usually not needed — `with_lock()` saves automatically.
    /// Use this when the value was modified externally.
    pub fn save(&self) -> Result<()> {
        let value = self.inner.value.read().clone();
        self.key.save(&value, SaveContext::UserInitiated)
    }

    /// Explicitly reload the value from the persistence key.
    pub fn load(&self) -> Result<()> {
        let loaded = self.key.load(LoadContext::UserInitiated)?;
        if let Some(value) = loaded {
            let mut guard = self.inner.value.write();
            *guard = value.clone();
            let _ = self.inner.sender.send(value);
        }
        Ok(())
    }

    /// Get a reactive watch receiver that yields on every change.
    ///
    /// # Example
    ///
    /// ```
    /// use sharing_instant::shared::Shared;
    /// use sharing_instant::keys::in_memory_key::InMemoryKey;
    ///
    /// let shared = Shared::new(0, InMemoryKey::new("counter"));
    /// let mut rx = shared.watch();
    ///
    /// shared.with_lock(|v| *v = 42);
    /// // rx.changed().await would yield here
    /// ```
    pub fn watch(&self) -> watch::Receiver<V> {
        self.inner.sender.subscribe()
    }

    /// Create a read-only view of this shared value.
    pub fn reader(&self) -> SharedReader<V> {
        SharedReader::from_watch(
            self.inner.value.read().clone(),
            self.inner.sender.subscribe(),
        )
    }
}

impl<V, K> Clone for Shared<V, K>
where
    V: Send + Sync + Clone + 'static,
    K: SharedKey<Value = V>,
{
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            key: self.key.clone(),
            _subscription: self._subscription.clone(),
        }
    }
}
