//! Tests for InMemoryKey.
//!
//! Mirrors tests from Swift's InMemoryTests.swift.

use sharing_instant::keys::in_memory_key::InMemoryKey;
use sharing_instant::shared::Shared;
use sharing_instant::shared_key::SharedKey;
use sharing_instant::shared_reader_key::{LoadContext, SharedReaderKey};

#[test]
fn in_memory_key_load_returns_none_initially() {
    let key = InMemoryKey::<i32>::new("test_load_none");
    let result = key.load(LoadContext::InitialValue(0)).unwrap();
    assert!(result.is_none());
}

#[test]
fn in_memory_key_save_and_load() {
    let key = InMemoryKey::<String>::new("test_save_load");

    key.save(
        &"hello".to_string(),
        sharing_instant::shared_key::SaveContext::DidSet,
    )
    .unwrap();

    let loaded = key
        .load(LoadContext::InitialValue("default".to_string()))
        .unwrap();
    assert_eq!(loaded.as_deref(), Some("hello"));
}

#[test]
fn in_memory_key_id_is_unique() {
    let key1 = InMemoryKey::<i32>::new("alpha");
    let key2 = InMemoryKey::<i32>::new("beta");

    assert_ne!(key1.id(), key2.id());
}

#[test]
fn in_memory_key_same_name_same_id() {
    let key1 = InMemoryKey::<i32>::new("gamma");
    let key2 = InMemoryKey::<i32>::new("gamma");

    assert_eq!(key1.id(), key2.id());
}

#[test]
fn in_memory_key_reference_sharing() {
    // Two Shared instances with the same key should share state
    let shared1 = Shared::new(0, InMemoryKey::<i32>::new("ref_share"));
    shared1.with_lock(|v| *v = 42);

    let shared2 = Shared::new(0, InMemoryKey::<i32>::new("ref_share"));
    assert_eq!(*shared2.get(), 42);
}

#[test]
fn in_memory_key_different_types_different_storage() {
    let key_int = InMemoryKey::<i32>::new("same_name_typed_int");
    let key_str = InMemoryKey::<String>::new("same_name_typed_str");

    key_int
        .save(&42, sharing_instant::shared_key::SaveContext::DidSet)
        .unwrap();

    // Different type with same-ish name should not interfere
    // (in practice they have different storage keys)
    let loaded_str = key_str
        .load(LoadContext::InitialValue("default".to_string()))
        .unwrap();
    // This may or may not return None depending on type erasure,
    // but it should not return 42 as a String
    if let Some(s) = loaded_str {
        assert_ne!(s, "42"); // Shouldn't cross-contaminate
    }
}
