//! Tests for PartialEq behavior of persisted values.
//!
//! Maps Swift's EquatableTests.swift to Rust PartialEq.

use sharing_instant::keys::in_memory_key::InMemoryKey;
use sharing_instant::shared::Shared;

#[test]
fn values_from_same_key_are_equal() {
    let shared1 = Shared::new(42, InMemoryKey::<i32>::new("eq_same_key_1"));
    let shared2 = Shared::new(42, InMemoryKey::<i32>::new("eq_same_key_1"));

    assert_eq!(*shared1.get(), *shared2.get());
}

#[test]
fn values_from_different_keys_can_be_equal() {
    let shared1 = Shared::new(42, InMemoryKey::<i32>::new("eq_diff_key_a"));
    let shared2 = Shared::new(42, InMemoryKey::<i32>::new("eq_diff_key_b"));

    // Same value, different keys — values are still equal
    assert_eq!(*shared1.get(), *shared2.get());
}

#[test]
fn mutated_values_diverge() {
    let shared1 = Shared::new(42, InMemoryKey::<i32>::new("eq_diverge_a"));
    let shared2 = Shared::new(42, InMemoryKey::<i32>::new("eq_diverge_b"));

    shared1.with_lock(|v| *v = 100);

    assert_ne!(*shared1.get(), *shared2.get());
}
