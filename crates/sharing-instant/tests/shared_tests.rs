//! Tests for Shared<V, K> wrapper.
//!
//! Mirrors tests from Swift's SharedTests.swift.
//!
//! Swift test mapping:
//! - projectedValue → shared_clone_shares_state (projected value = clone in Rust)
//! - nesting → shared_nested_values
//! - mutation (BoxReference) → shared_multiple_references_mutation
//! - appendWritableKeyPath → shared_reader_map_projection
//! - optional → shared_optional_wrapping
//! - collection → shared_collection_element
//! - task → shared_across_threads
//! - reader → shared_reader_reflects_mutations, shared_reader_creation
//! - valueDescription → shared_debug_representation
//! - save/load → shared_explicit_save_load

use sharing_instant::keys::in_memory_key::InMemoryKey;
use sharing_instant::shared::Shared;
use std::sync::Arc;
use std::thread;

// === Existing tests ===

#[test]
fn shared_initial_value() {
    let shared = Shared::new(42, InMemoryKey::<i32>::new("test_initial"));
    assert_eq!(*shared.get(), 42);
}

#[test]
fn shared_with_lock_mutates() {
    let shared = Shared::new(0, InMemoryKey::<i32>::new("test_mutate"));
    shared.with_lock(|v| *v = 100);
    assert_eq!(*shared.get(), 100);
}

#[test]
fn shared_with_lock_returns_value() {
    let shared = Shared::new(vec![1, 2, 3], InMemoryKey::<Vec<i32>>::new("test_return"));
    let len = shared.with_lock(|v| {
        v.push(4);
        v.len()
    });
    assert_eq!(len, 4);
    assert_eq!(&*shared.get(), &[1, 2, 3, 4]);
}

#[test]
fn shared_clone_shares_state() {
    let shared1 = Shared::new(0, InMemoryKey::<i32>::new("test_clone"));
    let shared2 = shared1.clone();

    shared1.with_lock(|v| *v = 42);
    assert_eq!(*shared2.get(), 42);
}

#[test]
fn shared_persists_to_in_memory_key() {
    let shared1 = Shared::new(0, InMemoryKey::<i32>::new("test_persist"));
    shared1.with_lock(|v| *v = 99);

    let shared2 = Shared::new(0, InMemoryKey::<i32>::new("test_persist"));
    assert_eq!(*shared2.get(), 99);
}

#[test]
fn shared_default_when_no_persisted_value() {
    let shared = Shared::new(42, InMemoryKey::<i32>::new("test_default_unique_key_12345"));
    assert_eq!(*shared.get(), 42);
}

#[test]
fn shared_with_string_value() {
    let shared = Shared::new(
        "hello".to_string(),
        InMemoryKey::<String>::new("test_string"),
    );
    shared.with_lock(|v| *v = "world".to_string());
    assert_eq!(&*shared.get(), "world");
}

#[test]
fn shared_with_struct_value() {
    #[derive(Debug, Clone, PartialEq)]
    struct Settings {
        volume: f64,
        muted: bool,
    }

    let shared = Shared::new(
        Settings {
            volume: 0.5,
            muted: false,
        },
        InMemoryKey::<Settings>::new("test_struct"),
    );

    shared.with_lock(|s| {
        s.volume = 0.8;
        s.muted = true;
    });

    let val = shared.get();
    assert_eq!(val.volume, 0.8);
    assert!(val.muted);
}

#[test]
fn shared_reader_reflects_mutations() {
    let shared = Shared::new(0, InMemoryKey::<i32>::new("test_reader"));
    let reader = shared.reader();

    assert_eq!(*reader.get(), 0);

    shared.with_lock(|v| *v = 42);

    let rx = reader.watch();
    assert_eq!(*rx.borrow(), 42);
}

// === New tests ported from Swift's SharedTests.swift ===

/// Maps to Swift's nesting test: nested Shared values within structs.
#[test]
fn shared_nested_values() {
    #[derive(Debug, Clone, PartialEq)]
    struct Inner {
        value: i32,
    }

    #[derive(Debug, Clone, PartialEq)]
    struct Outer {
        inner: Inner,
        label: String,
    }

    let shared = Shared::new(
        Outer {
            inner: Inner { value: 10 },
            label: "hello".to_string(),
        },
        InMemoryKey::<Outer>::new("test_nested"),
    );

    // Mutate the nested value
    shared.with_lock(|outer| {
        outer.inner.value = 20;
        outer.label = "world".to_string();
    });

    let val = shared.get();
    assert_eq!(val.inner.value, 20);
    assert_eq!(val.label, "world");
}

/// Maps to Swift's mutation (BoxReference) test: multiple references see mutations.
#[test]
fn shared_multiple_references_mutation() {
    let shared1 = Shared::new(0, InMemoryKey::<i32>::new("test_multi_ref"));
    let shared2 = shared1.clone();
    let shared3 = shared1.clone();

    shared1.with_lock(|v| *v = 10);
    assert_eq!(*shared2.get(), 10);
    assert_eq!(*shared3.get(), 10);

    shared2.with_lock(|v| *v = 20);
    assert_eq!(*shared1.get(), 20);
    assert_eq!(*shared3.get(), 20);

    shared3.with_lock(|v| *v = 30);
    assert_eq!(*shared1.get(), 30);
    assert_eq!(*shared2.get(), 30);
}

/// Maps to Swift's optional test: Shared with Option values.
#[test]
fn shared_optional_wrapping() {
    let shared = Shared::new(Some(42), InMemoryKey::<Option<i32>>::new("test_optional"));
    assert_eq!(*shared.get(), Some(42));

    // Set to None
    shared.with_lock(|v| *v = None);
    assert_eq!(*shared.get(), None);

    // Set back to Some
    shared.with_lock(|v| *v = Some(99));
    assert_eq!(*shared.get(), Some(99));
}

/// Maps to Swift's collection test: Shared collections with element-level work.
#[test]
fn shared_collection_element() {
    let shared = Shared::new(
        vec![1, 2, 3, 4, 5],
        InMemoryKey::<Vec<i32>>::new("test_collection"),
    );

    // Mutate individual elements
    shared.with_lock(|v| {
        v[0] = 10;
        v[4] = 50;
    });

    assert_eq!(&*shared.get(), &[10, 2, 3, 4, 50]);

    // Remove an element
    shared.with_lock(|v| {
        v.remove(2);
    });
    assert_eq!(&*shared.get(), &[10, 2, 4, 50]);
}

/// Maps to Swift's task test: Shared captured across threads.
#[test]
fn shared_across_threads() {
    let shared = Shared::new(0, InMemoryKey::<i32>::new("test_threads"));
    let shared_clone = shared.clone();

    let handle = thread::spawn(move || {
        shared_clone.with_lock(|v| *v = 42);
    });

    handle.join().expect("thread should not panic");
    assert_eq!(*shared.get(), 42);
}

/// Maps to Swift's task test variant: multiple threads writing concurrently.
#[test]
fn shared_concurrent_mutations() {
    let shared = Arc::new(Shared::new(
        0,
        InMemoryKey::<i32>::new("test_concurrent_mut"),
    ));

    let handles: Vec<_> = (0..10)
        .map(|_| {
            let s = shared.clone();
            thread::spawn(move || {
                for _ in 0..100 {
                    s.with_lock(|v| *v += 1);
                }
            })
        })
        .collect();

    for h in handles {
        h.join().expect("thread should not panic");
    }

    assert_eq!(*shared.get(), 1000);
}

/// Maps to Swift's reader test: SharedReader creation from Shared.
#[test]
fn shared_reader_creation() {
    let shared = Shared::new(
        "initial".to_string(),
        InMemoryKey::<String>::new("test_reader_create"),
    );

    let reader = shared.reader();
    assert_eq!(&*reader.get(), "initial");

    // Mutation on shared propagates to reader's watch channel
    shared.with_lock(|v| *v = "updated".to_string());
    let rx = reader.watch();
    assert_eq!(&*rx.borrow(), "updated");
}

/// Maps to Swift's explicit save/load operations.
#[test]
fn shared_explicit_save_load() {
    let shared = Shared::new(0, InMemoryKey::<i32>::new("test_save_load"));
    shared.with_lock(|v| *v = 42);

    // Explicit save (should succeed even though with_lock already saves)
    shared.save().expect("save should succeed");

    // Explicit load (reloads from persisted storage)
    shared.load().expect("load should succeed");
    assert_eq!(*shared.get(), 42);
}

/// Maps to Swift's watch channel reactivity.
#[test]
fn shared_watch_channel_receives_updates() {
    let shared = Shared::new(0, InMemoryKey::<i32>::new("test_watch_channel"));
    let rx = shared.watch();

    // Initial value
    assert_eq!(*rx.borrow(), 0);

    // After mutation, watch should have the new value
    shared.with_lock(|v| *v = 42);
    assert_eq!(*rx.borrow(), 42);

    shared.with_lock(|v| *v = 100);
    assert_eq!(*rx.borrow(), 100);
}

/// Maps to Swift's competing defaults: two Shared instances with same key, different defaults.
#[test]
fn shared_competing_defaults() {
    // First shared establishes the value
    let shared1 = Shared::new(10, InMemoryKey::<i32>::new("test_competing_defaults"));
    shared1.with_lock(|v| *v = 42);

    // Second shared with different default should load the persisted value, not the default
    let shared2 = Shared::new(999, InMemoryKey::<i32>::new("test_competing_defaults"));
    assert_eq!(*shared2.get(), 42);
}

/// Maps to Swift's equatable test: PartialEq for persisted values.
#[test]
fn shared_partial_eq_via_get() {
    let shared1 = Shared::new(42, InMemoryKey::<i32>::new("test_eq_1"));
    let shared2 = Shared::new(42, InMemoryKey::<i32>::new("test_eq_2"));

    // Values are equal even though keys differ
    assert_eq!(*shared1.get(), *shared2.get());

    shared1.with_lock(|v| *v = 100);
    assert_ne!(*shared1.get(), *shared2.get());
}
