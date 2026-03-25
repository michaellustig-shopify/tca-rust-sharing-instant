//! Tests for change tracking in tests.
//!
//! Maps Swift's SharedChangeTrackerTests.swift to Rust test patterns.
//! Verifies that test assertions can track changes to Shared values.

use sharing_instant::keys::in_memory_key::InMemoryKey;
use sharing_instant::shared::Shared;

#[test]
fn track_single_mutation() {
    let shared = Shared::new(0, InMemoryKey::<i32>::new("tracker_single"));
    let mut rx = shared.watch();

    // Initial value
    assert_eq!(*rx.borrow(), 0);

    // Track mutation
    shared.with_lock(|v| *v = 42);
    assert_eq!(*rx.borrow(), 42);
}

#[test]
fn track_multiple_mutations() {
    let shared = Shared::new(
        Vec::<String>::new(),
        InMemoryKey::<Vec<String>>::new("tracker_multi"),
    );
    let rx = shared.watch();

    shared.with_lock(|v| v.push("first".to_string()));
    assert_eq!(rx.borrow().len(), 1);

    shared.with_lock(|v| v.push("second".to_string()));
    assert_eq!(rx.borrow().len(), 2);

    shared.with_lock(|v| v.push("third".to_string()));
    assert_eq!(rx.borrow().len(), 3);
}

#[test]
fn track_no_mutation() {
    let shared = Shared::new(42, InMemoryKey::<i32>::new("tracker_no_change"));
    let rx = shared.watch();

    // No mutations — value stays the same
    assert_eq!(*rx.borrow(), 42);
    assert_eq!(*rx.borrow(), 42);
}

#[test]
fn track_mutation_across_clones() {
    let shared1 = Shared::new(0, InMemoryKey::<i32>::new("tracker_clones"));
    let shared2 = shared1.clone();
    let rx = shared1.watch();

    shared2.with_lock(|v| *v = 100);
    assert_eq!(*rx.borrow(), 100);
}

#[test]
fn track_revert_mutation() {
    let shared = Shared::new(42, InMemoryKey::<i32>::new("tracker_revert"));
    let rx = shared.watch();

    shared.with_lock(|v| *v = 100);
    assert_eq!(*rx.borrow(), 100);

    shared.with_lock(|v| *v = 42);
    assert_eq!(*rx.borrow(), 42);
}

#[test]
fn track_struct_field_mutations() {
    #[derive(Debug, Clone, PartialEq)]
    struct State {
        count: i32,
        active: bool,
    }

    let shared = Shared::new(
        State {
            count: 0,
            active: false,
        },
        InMemoryKey::<State>::new("tracker_struct"),
    );
    let rx = shared.watch();

    shared.with_lock(|s| s.count = 5);
    assert_eq!(rx.borrow().count, 5);
    assert!(!rx.borrow().active);

    shared.with_lock(|s| s.active = true);
    assert!(rx.borrow().active);
}

#[test]
fn track_nested_collection_mutations() {
    let shared = Shared::new(
        vec![vec![1, 2], vec![3, 4]],
        InMemoryKey::<Vec<Vec<i32>>>::new("tracker_nested"),
    );
    let rx = shared.watch();

    shared.with_lock(|v| v[0].push(99));
    assert_eq!(rx.borrow()[0], vec![1, 2, 99]);
    assert_eq!(rx.borrow()[1], vec![3, 4]); // unchanged
}

#[test]
fn track_option_transitions() {
    let shared = Shared::new(
        None::<String>,
        InMemoryKey::<Option<String>>::new("tracker_option"),
    );
    let rx = shared.watch();

    assert_eq!(*rx.borrow(), None);

    shared.with_lock(|v| *v = Some("hello".to_string()));
    assert_eq!(*rx.borrow(), Some("hello".to_string()));

    shared.with_lock(|v| *v = None);
    assert_eq!(*rx.borrow(), None);
}
