//! Tests for SharedSubscription RAII lifecycle.

use sharing_instant::subscription::SharedSubscription;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

#[test]
fn subscription_cancel_on_drop() {
    let cancelled = Arc::new(AtomicBool::new(false));
    let cancelled_clone = cancelled.clone();

    let sub = SharedSubscription::new(move || {
        cancelled_clone.store(true, Ordering::SeqCst);
    });

    assert!(!cancelled.load(Ordering::SeqCst));
    drop(sub);
    assert!(cancelled.load(Ordering::SeqCst));
}

#[test]
fn subscription_explicit_cancel() {
    let cancelled = Arc::new(AtomicBool::new(false));
    let cancelled_clone = cancelled.clone();

    let mut sub = SharedSubscription::new(move || {
        cancelled_clone.store(true, Ordering::SeqCst);
    });

    sub.cancel();
    assert!(cancelled.load(Ordering::SeqCst));
}

#[test]
fn subscription_double_cancel_safe() {
    let count = Arc::new(std::sync::atomic::AtomicU32::new(0));
    let count_clone = count.clone();

    let mut sub = SharedSubscription::new(move || {
        count_clone.fetch_add(1, Ordering::SeqCst);
    });

    sub.cancel();
    sub.cancel(); // Second cancel should be no-op

    assert_eq!(count.load(Ordering::SeqCst), 1);
}

#[test]
fn subscription_empty_is_safe() {
    let sub = SharedSubscription::empty();
    drop(sub); // Should not panic
}
