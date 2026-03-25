//! Tests for MutationCallbacks<T>.

use sharing_instant::MutationCallbacks;
use sharing_instant::SharingInstantError;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

#[test]
fn empty_callbacks() {
    let cb = MutationCallbacks::<String>::new();
    assert!(cb.is_empty());
}

#[test]
fn builder_pattern() {
    let cb = MutationCallbacks::<String>::new()
        .on_mutate(|| {})
        .on_success(|_| {})
        .on_error(|_| {})
        .on_settled(|| {});
    assert!(!cb.is_empty());
}

#[test]
fn fire_success_calls_mutate_success_settled() {
    let order = Arc::new(AtomicUsize::new(0));

    let o1 = order.clone();
    let o2 = order.clone();
    let o3 = order.clone();

    let cb = MutationCallbacks::<i32>::new()
        .on_mutate(move || {
            assert_eq!(o1.fetch_add(1, Ordering::SeqCst), 0);
        })
        .on_success(move |val| {
            assert_eq!(val, 42);
            assert_eq!(o2.fetch_add(1, Ordering::SeqCst), 1);
        })
        .on_settled(move || {
            assert_eq!(o3.fetch_add(1, Ordering::SeqCst), 2);
        });

    cb.fire_success(42);
    assert_eq!(order.load(Ordering::SeqCst), 3);
}

#[test]
fn fire_error_calls_mutate_error_settled() {
    let order = Arc::new(AtomicUsize::new(0));

    let o1 = order.clone();
    let o2 = order.clone();
    let o3 = order.clone();

    let cb = MutationCallbacks::<String>::new()
        .on_mutate(move || {
            assert_eq!(o1.fetch_add(1, Ordering::SeqCst), 0);
        })
        .on_error(move |err| {
            assert!(err.to_string().contains("query failed"));
            assert_eq!(o2.fetch_add(1, Ordering::SeqCst), 1);
        })
        .on_settled(move || {
            assert_eq!(o3.fetch_add(1, Ordering::SeqCst), 2);
        });

    cb.fire_error(SharingInstantError::QueryFailed("query failed".to_string()));
    assert_eq!(order.load(Ordering::SeqCst), 3);
}

#[test]
fn fire_success_without_callbacks_is_noop() {
    let cb = MutationCallbacks::<i32>::new();
    cb.fire_success(0); // should not panic
}

#[test]
fn fire_error_without_callbacks_is_noop() {
    let cb = MutationCallbacks::<i32>::new();
    cb.fire_error(SharingInstantError::QueryFailed("err".to_string()));
}

#[test]
fn debug_shows_callback_presence() {
    let cb = MutationCallbacks::<i32>::new().on_success(|_| {});
    let debug = format!("{cb:?}");
    assert!(debug.contains("on_success: true"));
    assert!(debug.contains("on_mutate: false"));
}

#[test]
fn error_only_sets_only_on_error() {
    let called = Arc::new(AtomicUsize::new(0));
    let c = called.clone();

    let cb = MutationCallbacks::<String>::error_only(move |_err| {
        c.fetch_add(1, Ordering::SeqCst);
    });

    assert!(cb.on_error.is_some());
    assert!(cb.on_success.is_none());
    assert!(cb.on_mutate.is_none());
    assert!(cb.on_settled.is_none());

    cb.fire_error(SharingInstantError::QueryFailed("boom".into()));
    assert_eq!(called.load(Ordering::SeqCst), 1);
}

#[test]
fn success_only_sets_only_on_success() {
    let called = Arc::new(AtomicUsize::new(0));
    let c = called.clone();

    let cb = MutationCallbacks::<i32>::success_only(move |val| {
        assert_eq!(val, 99);
        c.fetch_add(1, Ordering::SeqCst);
    });

    assert!(cb.on_success.is_some());
    assert!(cb.on_error.is_none());
    assert!(cb.on_mutate.is_none());
    assert!(cb.on_settled.is_none());

    cb.fire_success(99);
    assert_eq!(called.load(Ordering::SeqCst), 1);
}

#[test]
fn settled_only_sets_only_on_settled() {
    let called = Arc::new(AtomicUsize::new(0));
    let c = called.clone();

    let cb = MutationCallbacks::<()>::settled_only(move || {
        c.fetch_add(1, Ordering::SeqCst);
    });

    assert!(cb.on_settled.is_some());
    assert!(cb.on_success.is_none());
    assert!(cb.on_error.is_none());
    assert!(cb.on_mutate.is_none());

    cb.fire_success(());
    assert_eq!(called.load(Ordering::SeqCst), 1);
}
