//! ┌─────────────────────────────────────────────────────┐
//! │  IN-MEMORY KEY                                       │
//! │  Simple in-process sharing without persistence        │
//! ├─────────────────────────────────────────────────────┤
//! │                                                      │
//! │  InMemoryKey<V>                                      │
//! │    └── Global HashMap<String, Any>                   │
//! │         ├── load()  → read from map                  │
//! │         └── save()  → write to map                   │
//! │                                                      │
//! ├─────────────────────────────────────────────────────┤
//! │  WHY: Mirrors Swift's InMemoryKey. Useful for        │
//! │  sharing state across components within a single     │
//! │  process without external persistence.               │
//! │                                                      │
//! │  TESTED BY: tests/in_memory_key_tests.rs             │
//! │                                                      │
//! │  CHANGELOG:                                          │
//! │  • v0.1.0 — Initial InMemoryKey                      │
//! │                                                      │
//! │  HISTORY: git log --oneline --follow -- crates/sharing-instant/src/keys/in_memory_key.rs │
//! └─────────────────────────────────────────────────────┘

use crate::error::Result;
use crate::shared_key::{SaveContext, SharedKey};
use crate::shared_reader_key::{LoadContext, SharedReaderKey, SharedSubscriber};
use crate::subscription::SharedSubscription;
use dashmap::DashMap;
use std::any::Any;
use std::sync::Arc;

/// Global in-memory storage shared across all `InMemoryKey` instances.
static STORAGE: std::sync::LazyLock<DashMap<String, Arc<dyn Any + Send + Sync>>> =
    std::sync::LazyLock::new(DashMap::new);

/// In-memory persistence key.
///
/// Stores values in a global concurrent hash map. Values survive
/// across `Shared` instances with the same key name but are lost
/// when the process exits.
///
/// Mirrors Swift's `InMemoryKey`.
///
/// # Example
///
/// ```
/// use sharing_instant::keys::in_memory_key::InMemoryKey;
/// use sharing_instant::shared::Shared;
///
/// let shared1 = Shared::new(42, InMemoryKey::<i32>::new("counter"));
/// shared1.with_lock(|v| *v = 100);
///
/// // Another Shared with the same key sees the value
/// let shared2 = Shared::new(0, InMemoryKey::<i32>::new("counter"));
/// assert_eq!(*shared2.get(), 100);
/// ```
pub struct InMemoryKey<V: Send + Sync + Clone + 'static> {
    name: String,
    _phantom: std::marker::PhantomData<V>,
}

impl<V: Send + Sync + Clone + 'static> InMemoryKey<V> {
    /// Create a new in-memory key with the given name.
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<V: Send + Sync + Clone + 'static> SharedReaderKey for InMemoryKey<V> {
    type Value = V;
    type Id = String;

    fn id(&self) -> String {
        format!("in_memory:{}", self.name)
    }

    fn load(&self, context: LoadContext<V>) -> Result<Option<V>> {
        if let Some(entry) = STORAGE.get(&self.name) {
            if let Some(value) = entry.value().downcast_ref::<V>() {
                return Ok(Some(value.clone()));
            }
        }
        Ok(None)
    }

    fn subscribe(
        &self,
        _context: LoadContext<V>,
        _subscriber: SharedSubscriber<V>,
    ) -> SharedSubscription {
        // In-memory keys don't have external change sources
        SharedSubscription::empty()
    }
}

impl<V: Send + Sync + Clone + 'static> SharedKey for InMemoryKey<V> {
    fn save(&self, value: &V, _context: SaveContext) -> Result<()> {
        STORAGE.insert(self.name.clone(), Arc::new(value.clone()));
        Ok(())
    }
}
