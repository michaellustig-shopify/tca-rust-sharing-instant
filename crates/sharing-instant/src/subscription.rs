//! ┌─────────────────────────────────────────────────────┐
//! │  SUBSCRIPTION                                        │
//! │  Cancellable handle for reactive streams              │
//! ├─────────────────────────────────────────────────────┤
//! │                                                      │
//! │  SharedSubscription ─── Drop ──► cancel()            │
//! │                                                      │
//! │  Analogous to Swift's SharedSubscription / Combine's │
//! │  AnyCancellable. Dropping cancels the subscription.  │
//! │                                                      │
//! ├─────────────────────────────────────────────────────┤
//! │  WHY: RAII-based subscription lifecycle. No manual   │
//! │  cancel calls needed — just let it go out of scope.  │
//! │                                                      │
//! │  ALTERNATIVES: Explicit cancel (error-prone),        │
//! │  weak references (unpredictable lifetime).           │
//! │                                                      │
//! │  TESTED BY: tests/subscription_tests.rs              │
//! │                                                      │
//! │  CHANGELOG:                                          │
//! │  • v0.1.0 — Initial SharedSubscription               │
//! │                                                      │
//! │  HISTORY: git log --oneline --follow -- crates/sharing-instant/src/subscription.rs │
//! └─────────────────────────────────────────────────────┘

/// A cancellable subscription handle.
///
/// When dropped, the subscription is automatically cancelled.
/// This mirrors Swift's `SharedSubscription` and follows Rust's
/// RAII pattern for resource management.
///
/// # Example
///
/// ```
/// use sharing_instant::subscription::SharedSubscription;
/// use std::sync::Arc;
/// use std::sync::atomic::{AtomicBool, Ordering};
///
/// let cancelled = Arc::new(AtomicBool::new(false));
/// let cancelled_clone = cancelled.clone();
///
/// let sub = SharedSubscription::new(move || {
///     cancelled_clone.store(true, Ordering::SeqCst);
/// });
///
/// assert!(!cancelled.load(Ordering::SeqCst));
/// drop(sub);
/// assert!(cancelled.load(Ordering::SeqCst));
/// ```
pub struct SharedSubscription {
    cancel_fn: Option<Box<dyn FnOnce() + Send + Sync>>,
}

impl SharedSubscription {
    /// Create a new subscription with the given cancellation callback.
    pub fn new(cancel: impl FnOnce() + Send + Sync + 'static) -> Self {
        Self {
            cancel_fn: Some(Box::new(cancel)),
        }
    }

    /// Create an empty (no-op) subscription.
    ///
    /// Useful for persistence keys that don't support subscriptions.
    pub fn empty() -> Self {
        Self { cancel_fn: None }
    }

    /// Explicitly cancel the subscription.
    ///
    /// This is also called automatically on `Drop`.
    pub fn cancel(&mut self) {
        if let Some(f) = self.cancel_fn.take() {
            f();
        }
    }
}

impl Drop for SharedSubscription {
    fn drop(&mut self) {
        self.cancel();
    }
}

// SharedSubscription is Send + Sync because the cancel_fn is Send + Sync
unsafe impl Send for SharedSubscription {}
unsafe impl Sync for SharedSubscription {}
