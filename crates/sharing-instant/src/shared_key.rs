//! ┌─────────────────────────────────────────────────────┐
//! │  SHARED KEY                                          │
//! │  Mutable persistence abstraction                     │
//! ├─────────────────────────────────────────────────────┤
//! │                                                      │
//! │  SharedKey (extends SharedReaderKey)                  │
//! │    └── save() → persist value to external store      │
//! │                                                      │
//! │  Use SharedReaderKey for read-only derived state.     │
//! │  Use SharedKey when the Rust code needs to write.    │
//! │                                                      │
//! ├─────────────────────────────────────────────────────┤
//! │  WHY: Mirrors Swift's SharedKey protocol. Separating │
//! │  read from write enables safe derived state and      │
//! │  prevents accidental mutations.                      │
//! │                                                      │
//! │  TESTED BY: tests/shared_key_tests.rs                │
//! │                                                      │
//! │  CHANGELOG:                                          │
//! │  • v0.1.0 — Initial trait definition                 │
//! │                                                      │
//! │  HISTORY: git log --oneline --follow -- crates/sharing-instant/src/shared_key.rs │
//! └─────────────────────────────────────────────────────┘

use crate::error::Result;
use crate::shared_reader_key::SharedReaderKey;

/// Context for save operations.
///
/// Mirrors Swift's `SaveContext` enum — distinguishes between
/// implicit saves (after mutation) and explicit user-initiated saves.
///
/// # Example
///
/// ```
/// use sharing_instant::shared_key::SaveContext;
///
/// let ctx = SaveContext::DidSet;
/// assert!(matches!(ctx, SaveContext::DidSet));
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SaveContext {
    /// The value was mutated via `with_lock()` or assignment.
    DidSet,
    /// The user explicitly called `save()`.
    UserInitiated,
}

/// Mutable persistence abstraction.
///
/// Extends `SharedReaderKey` with write capability. Implementations
/// persist values to an external store when `save()` is called.
///
/// # Design
///
/// Like Swift's `SharedKey`, the save operation uses synchronous
/// return with `Result` for simplicity. Async persistence backends
/// should queue the write internally.
///
/// # Example
///
/// ```
/// use sharing_instant::shared_key::{SharedKey, SaveContext};
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
///     fn id(&self) -> String { self.name.clone() }
///
///     fn load(&self, _ctx: LoadContext<i32>) -> sharing_instant::error::Result<Option<i32>> {
///         Ok(Some(0))
///     }
///
///     fn subscribe(
///         &self,
///         _ctx: LoadContext<i32>,
///         _sub: SharedSubscriber<i32>,
///     ) -> SharedSubscription {
///         SharedSubscription::empty()
///     }
/// }
///
/// impl SharedKey for CounterKey {
///     fn save(&self, value: &i32, context: SaveContext) -> sharing_instant::error::Result<()> {
///         // Persist to external store
///         Ok(())
///     }
/// }
/// ```
pub trait SharedKey: SharedReaderKey {
    /// Persist the given value to the external store.
    ///
    /// Called automatically after `Shared::with_lock()` mutations
    /// (with `SaveContext::DidSet`) or explicitly via `Shared::save()`
    /// (with `SaveContext::UserInitiated`).
    fn save(&self, value: &Self::Value, context: SaveContext) -> Result<()>;
}
