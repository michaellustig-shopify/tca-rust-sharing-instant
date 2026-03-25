//! ┌─────────────────────────────────────────────────────┐
//! │  SHARED READER KEY                                   │
//! │  Read-only persistence abstraction                    │
//! ├─────────────────────────────────────────────────────┤
//! │                                                      │
//! │  SharedReaderKey                                     │
//! │    ├── load()      → initial value from store        │
//! │    └── subscribe() → stream of updates               │
//! │                                                      │
//! │  Implementations:                                    │
//! │    ├── InstantDbKey   (InstantDB queries)            │
//! │    ├── InMemoryKey    (in-process sharing)           │
//! │    └── FileStorageKey (file system)                  │
//! │                                                      │
//! ├─────────────────────────────────────────────────────┤
//! │  WHY: Mirrors Swift's SharedReaderKey protocol.      │
//! │  Separates read-only access from mutation, enabling  │
//! │  derived/computed shared state.                       │
//! │                                                      │
//! │  ALTERNATIVES: Single ReadWrite trait (less          │
//! │  composable), direct store access (no abstraction).  │
//! │                                                      │
//! │  TESTED BY: tests/shared_reader_key_tests.rs         │
//! │                                                      │
//! │  CHANGELOG:                                          │
//! │  • v0.1.0 — Initial trait definition                 │
//! │                                                      │
//! │  HISTORY: git log --oneline --follow -- crates/sharing-instant/src/shared_reader_key.rs │
//! └─────────────────────────────────────────────────────┘

use crate::error::Result;
use crate::subscription::SharedSubscription;
use std::fmt::Debug;
use std::hash::Hash;

/// Context provided when loading a value from a persistence key.
///
/// Mirrors Swift's `LoadContext<Value>` enum, distinguishing between
/// initial load (with a default value) and explicit reload.
///
/// # Example
///
/// ```
/// use sharing_instant::shared_reader_key::LoadContext;
///
/// let ctx: LoadContext<String> = LoadContext::InitialValue("default".to_string());
/// assert!(matches!(ctx, LoadContext::InitialValue(_)));
/// ```
#[derive(Debug, Clone)]
pub enum LoadContext<V> {
    /// Loading during initial creation — the provided value is the default.
    InitialValue(V),
    /// Loading on explicit user request (e.g., `load()` call).
    UserInitiated,
}

/// A subscriber that receives updates from a persistence key.
///
/// Mirrors Swift's `SharedSubscriber<Value>` — a callback-based
/// push mechanism for external change notifications.
///
/// # Example
///
/// ```
/// use sharing_instant::shared_reader_key::SharedSubscriber;
///
/// let subscriber = SharedSubscriber::new(|result| {
///     match result {
///         Ok(Some(value)) => println!("got update: {value:?}"),
///         Ok(None) => println!("value cleared"),
///         Err(e) => eprintln!("error: {e}"),
///     }
/// });
/// subscriber.yield_value(42);
/// ```
pub struct SharedSubscriber<V: Send + 'static> {
    callback: Box<
        dyn Fn(std::result::Result<Option<V>, crate::error::SharingInstantError>) + Send + Sync,
    >,
}

impl<V: Send + 'static> SharedSubscriber<V> {
    /// Create a new subscriber with the given callback.
    pub fn new(
        callback: impl Fn(std::result::Result<Option<V>, crate::error::SharingInstantError>)
            + Send
            + Sync
            + 'static,
    ) -> Self {
        Self {
            callback: Box::new(callback),
        }
    }

    /// Push an updated value to the subscriber.
    pub fn yield_value(&self, value: V) {
        (self.callback)(Ok(Some(value)));
    }

    /// Signal that the value should revert to its initial/default value.
    pub fn yield_returning_initial_value(&self) {
        (self.callback)(Ok(None));
    }

    /// Push an error to the subscriber.
    pub fn yield_error(&self, error: crate::error::SharingInstantError) {
        (self.callback)(Err(error));
    }
}

/// Read-only persistence abstraction.
///
/// Mirrors Swift's `SharedReaderKey` protocol. Implementations provide:
/// - A hashable identity for reference deduplication
/// - A `load` method for initial/on-demand value fetching
/// - A `subscribe` method for reactive change notifications
///
/// # Design
///
/// Uses continuation-passing style (callbacks) rather than async/await
/// to support integration with non-async systems (file watchers,
/// InstantDB's watch channels, etc.).
///
/// # Example
///
/// ```
/// use sharing_instant::shared_reader_key::{SharedReaderKey, LoadContext, SharedSubscriber};
/// use sharing_instant::subscription::SharedSubscription;
///
/// struct CounterKey {
///     name: String,
/// }
///
/// impl SharedReaderKey for CounterKey {
///     type Value = i32;
///     type Id = String;
///
///     fn id(&self) -> String {
///         self.name.clone()
///     }
///
///     fn load(&self, context: LoadContext<i32>) -> sharing_instant::error::Result<Option<i32>> {
///         Ok(Some(0))
///     }
///
///     fn subscribe(
///         &self,
///         _context: LoadContext<i32>,
///         _subscriber: SharedSubscriber<i32>,
///     ) -> SharedSubscription {
///         SharedSubscription::empty()
///     }
/// }
/// ```
pub trait SharedReaderKey: Send + Sync + 'static {
    /// The type of value this key loads and subscribes to.
    type Value: Send + Sync + Clone + 'static;

    /// Hashable identity for reference deduplication.
    ///
    /// Multiple `Shared` instances with the same `id()` will share
    /// a single underlying reference, ensuring mutations are visible
    /// across all instances.
    type Id: Hash + Eq + Send + Sync + Clone + Debug + 'static;

    /// Returns the identity of this key for deduplication.
    fn id(&self) -> Self::Id;

    /// Load the current value synchronously.
    ///
    /// Returns `Ok(Some(value))` if a value exists, `Ok(None)` if
    /// the caller should use the default, or `Err` on failure.
    fn load(&self, context: LoadContext<Self::Value>) -> Result<Option<Self::Value>>;

    /// Subscribe to changes from the external store.
    ///
    /// The returned `SharedSubscription` must be held alive —
    /// dropping it cancels the subscription.
    fn subscribe(
        &self,
        context: LoadContext<Self::Value>,
        subscriber: SharedSubscriber<Self::Value>,
    ) -> SharedSubscription;
}
