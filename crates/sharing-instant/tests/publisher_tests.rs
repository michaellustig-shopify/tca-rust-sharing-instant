//! Tests for watch channel reactivity (maps to Swift's Combine Publisher).
//!
//! Maps Swift's PublisherTests.swift to Rust's watch::Receiver.
//!
//! Swift test mapping:
//! - Combine Publisher → tokio watch::Receiver
//! - reassign key → watch reflects new key's value
//! - reload → explicit load updates watch
//! - retain → watch receiver outlives Shared

use sharing_instant::keys::in_memory_key::InMemoryKey;
use sharing_instant::shared::Shared;

#[test]
fn watch_receives_initial_value() {
    let shared = Shared::new(42, InMemoryKey::<i32>::new("pub_test_initial"));
    let rx = shared.watch();
    assert_eq!(*rx.borrow(), 42);
}

#[test]
fn watch_receives_mutations() {
    let shared = Shared::new(0, InMemoryKey::<i32>::new("pub_test_mutations"));
    let rx = shared.watch();

    shared.with_lock(|v| *v = 1);
    assert_eq!(*rx.borrow(), 1);

    shared.with_lock(|v| *v = 2);
    assert_eq!(*rx.borrow(), 2);

    shared.with_lock(|v| *v = 3);
    assert_eq!(*rx.borrow(), 3);
}

#[test]
fn watch_from_reader() {
    let shared = Shared::new(
        "hello".to_string(),
        InMemoryKey::<String>::new("pub_test_reader"),
    );
    let reader = shared.reader();
    let rx = reader.watch();

    assert_eq!(&*rx.borrow(), "hello");

    shared.with_lock(|v| *v = "world".to_string());
    assert_eq!(&*rx.borrow(), "world");
}

#[test]
fn watch_multiple_receivers() {
    let shared = Shared::new(0, InMemoryKey::<i32>::new("pub_test_multi_rx"));
    let rx1 = shared.watch();
    let rx2 = shared.watch();
    let rx3 = shared.watch();

    shared.with_lock(|v| *v = 42);

    assert_eq!(*rx1.borrow(), 42);
    assert_eq!(*rx2.borrow(), 42);
    assert_eq!(*rx3.borrow(), 42);
}

#[test]
fn watch_after_explicit_load() {
    let shared = Shared::new(0, InMemoryKey::<i32>::new("pub_test_reload"));
    let rx = shared.watch();

    shared.with_lock(|v| *v = 42);
    assert_eq!(*rx.borrow(), 42);

    // Explicit load reloads from persisted storage (same key, same value)
    shared.load().expect("load should succeed");
    assert_eq!(*rx.borrow(), 42);
}

#[test]
fn watch_receiver_outlives_mutation() {
    let rx;
    {
        let shared = Shared::new(0, InMemoryKey::<i32>::new("pub_test_outlive"));
        rx = shared.watch();
        shared.with_lock(|v| *v = 99);
    }
    // Shared is dropped, but rx still holds the last value
    assert_eq!(*rx.borrow(), 99);
}

#[test]
fn watch_struct_value() {
    #[derive(Debug, Clone, PartialEq)]
    struct State {
        count: i32,
        label: String,
    }

    let shared = Shared::new(
        State {
            count: 0,
            label: "start".to_string(),
        },
        InMemoryKey::<State>::new("pub_test_struct"),
    );
    let rx = shared.watch();

    shared.with_lock(|s| {
        s.count = 5;
        s.label = "middle".to_string();
    });

    let val = rx.borrow().clone();
    assert_eq!(val.count, 5);
    assert_eq!(val.label, "middle");
}

#[test]
fn watch_vec_mutations() {
    let shared = Shared::new(
        Vec::<i32>::new(),
        InMemoryKey::<Vec<i32>>::new("pub_test_vec"),
    );
    let rx = shared.watch();

    for i in 1..=5 {
        shared.with_lock(|v| v.push(i));
    }

    assert_eq!(&*rx.borrow(), &[1, 2, 3, 4, 5]);
}

#[test]
fn watch_clone_shares_channel() {
    let shared = Shared::new(0, InMemoryKey::<i32>::new("pub_test_clone_chan"));
    let cloned = shared.clone();
    let rx = cloned.watch();

    shared.with_lock(|v| *v = 42);
    assert_eq!(*rx.borrow(), 42);
}

#[test]
fn watch_rapid_mutations() {
    let shared = Shared::new(0, InMemoryKey::<i32>::new("pub_test_rapid"));
    let rx = shared.watch();

    for i in 0..1000 {
        shared.with_lock(|v| *v = i);
    }

    // Watch should have the final value
    assert_eq!(*rx.borrow(), 999);
}
