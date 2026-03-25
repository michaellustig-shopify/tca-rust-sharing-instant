//! Tests for default value behavior across SharedKey implementations.
//!
//! Maps Swift's DefaultTests.swift to Rust's InMemoryKey/FileStorageKey defaults.
//!
//! Swift test mapping:
//! - competing defaults → default_competing_defaults
//! - persistence after release → default_persistence_survives_drop
//! - writer/reader interplay → default_writer_reader_interplay

use sharing_instant::keys::in_memory_key::InMemoryKey;
use sharing_instant::shared::Shared;

#[test]
fn default_used_when_no_persisted_value() {
    let shared = Shared::new(42, InMemoryKey::<i32>::new("default_test_no_persist_1"));
    assert_eq!(*shared.get(), 42);
}

#[test]
fn default_competing_defaults_first_write_wins() {
    // First writer establishes the persisted value
    let shared1 = Shared::new(10, InMemoryKey::<i32>::new("default_test_competing_1"));
    shared1.with_lock(|v| *v = 42);

    // Second shared with different default loads persisted value
    let shared2 = Shared::new(999, InMemoryKey::<i32>::new("default_test_competing_1"));
    assert_eq!(*shared2.get(), 42);
}

#[test]
fn default_persistence_survives_drop() {
    // Write and drop
    {
        let shared = Shared::new(0, InMemoryKey::<i32>::new("default_test_survives_drop"));
        shared.with_lock(|v| *v = 77);
    }

    // Value persists in global storage after drop
    let shared = Shared::new(0, InMemoryKey::<i32>::new("default_test_survives_drop"));
    assert_eq!(*shared.get(), 77);
}

#[test]
fn default_writer_reader_interplay() {
    let writer = Shared::new(
        "initial".to_string(),
        InMemoryKey::<String>::new("default_test_writer_reader"),
    );
    let reader = writer.reader();

    assert_eq!(&*reader.get(), "initial");

    writer.with_lock(|v| *v = "updated".to_string());
    let rx = reader.watch();
    assert_eq!(&*rx.borrow(), "updated");
}

#[test]
fn default_multiple_keys_independent() {
    let shared_a = Shared::new(1, InMemoryKey::<i32>::new("default_test_independent_a"));
    let shared_b = Shared::new(2, InMemoryKey::<i32>::new("default_test_independent_b"));

    shared_a.with_lock(|v| *v = 100);
    assert_eq!(*shared_b.get(), 2); // B unaffected
}

#[test]
fn default_overwrite_persisted_value() {
    let shared1 = Shared::new(0, InMemoryKey::<i32>::new("default_test_overwrite"));
    shared1.with_lock(|v| *v = 50);

    // New shared loads 50, then overwrites
    let shared2 = Shared::new(0, InMemoryKey::<i32>::new("default_test_overwrite"));
    assert_eq!(*shared2.get(), 50);
    shared2.with_lock(|v| *v = 100);

    // Third shared sees the latest value
    let shared3 = Shared::new(0, InMemoryKey::<i32>::new("default_test_overwrite"));
    assert_eq!(*shared3.get(), 100);
}

#[test]
fn default_with_option_type() {
    let shared = Shared::new(
        None::<i32>,
        InMemoryKey::<Option<i32>>::new("default_test_option"),
    );
    assert_eq!(*shared.get(), None);

    shared.with_lock(|v| *v = Some(42));

    let reload = Shared::new(None, InMemoryKey::<Option<i32>>::new("default_test_option"));
    assert_eq!(*reload.get(), Some(42));
}

#[test]
fn default_with_vec_type() {
    let shared = Shared::new(
        Vec::<String>::new(),
        InMemoryKey::<Vec<String>>::new("default_test_vec"),
    );
    shared.with_lock(|v| v.push("hello".to_string()));

    let reload = Shared::new(
        Vec::<String>::new(),
        InMemoryKey::<Vec<String>>::new("default_test_vec"),
    );
    assert_eq!(&*reload.get(), &["hello".to_string()]);
}

#[test]
fn default_clone_shares_persisted_state() {
    let shared = Shared::new(0, InMemoryKey::<i32>::new("default_test_clone_persist"));
    let cloned = shared.clone();

    shared.with_lock(|v| *v = 42);
    assert_eq!(*cloned.get(), 42);

    // Both should load persisted value
    let reload = Shared::new(0, InMemoryKey::<i32>::new("default_test_clone_persist"));
    assert_eq!(*reload.get(), 42);
}
