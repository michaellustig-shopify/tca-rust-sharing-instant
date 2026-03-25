//! Tests for FileStorageKey persistence.
//!
//! Maps Swift's FileStorageTests.swift to Rust's FileStorageKey.
//!
//! Swift test mapping:
//! - basics → file_storage_roundtrip
//! - throttle → file_storage_write_idempotent (no throttle impl yet)
//! - multipleFiles → file_storage_multiple_files
//! - initialValue → file_storage_loads_existing
//! - decodeFailure → file_storage_decode_failure
//! - deleteFile → file_storage_file_deleted
//! - multipleMutations → file_storage_concurrent_mutations

use sharing_instant::keys::file_storage_key::FileStorageKey;
use sharing_instant::shared::Shared;
use std::path::PathBuf;

fn temp_path(name: &str) -> PathBuf {
    let mut path = std::env::temp_dir();
    path.push(format!("sharing_instant_test_{name}.json"));
    path
}

fn cleanup(path: &PathBuf) {
    let _ = std::fs::remove_file(path);
}

#[test]
fn file_storage_roundtrip() {
    let path = temp_path("roundtrip");
    cleanup(&path);

    let shared = Shared::new(vec![1, 2, 3], FileStorageKey::new(&path));
    shared.with_lock(|v| v.push(4));

    // Verify file was written
    assert!(path.exists());

    // New Shared loads from file
    let shared2 = Shared::new(Vec::<i32>::new(), FileStorageKey::new(&path));
    assert_eq!(&*shared2.get(), &[1, 2, 3, 4]);

    cleanup(&path);
}

#[test]
fn file_storage_initial_value_from_file() {
    let path = temp_path("initial_value");
    cleanup(&path);

    // Pre-write a file
    std::fs::write(&path, "[10, 20, 30]").expect("write should succeed");

    let shared = Shared::new(Vec::<i32>::new(), FileStorageKey::new(&path));
    assert_eq!(&*shared.get(), &[10, 20, 30]);

    cleanup(&path);
}

#[test]
fn file_storage_default_when_no_file() {
    let path = temp_path("no_file_default");
    cleanup(&path);

    let shared = Shared::new(42, FileStorageKey::new(&path));
    assert_eq!(*shared.get(), 42);

    cleanup(&path);
}

#[test]
fn file_storage_multiple_files() {
    let path1 = temp_path("multi_1");
    let path2 = temp_path("multi_2");
    cleanup(&path1);
    cleanup(&path2);

    let shared1 = Shared::new("file1".to_string(), FileStorageKey::new(&path1));
    let shared2 = Shared::new("file2".to_string(), FileStorageKey::new(&path2));

    shared1.with_lock(|v| *v = "updated1".to_string());
    shared2.with_lock(|v| *v = "updated2".to_string());

    // Re-load from disk
    let reload1 = Shared::new(String::new(), FileStorageKey::new(&path1));
    let reload2 = Shared::new(String::new(), FileStorageKey::new(&path2));

    assert_eq!(&*reload1.get(), "updated1");
    assert_eq!(&*reload2.get(), "updated2");

    cleanup(&path1);
    cleanup(&path2);
}

#[test]
fn file_storage_decode_failure_uses_default() {
    let path = temp_path("decode_fail");
    cleanup(&path);

    // Write invalid JSON for the expected type
    std::fs::write(&path, "not valid json").expect("write should succeed");

    // FileStorageKey load fails, Shared falls back to default
    let shared = Shared::new(42, FileStorageKey::new(&path));
    assert_eq!(*shared.get(), 42);

    cleanup(&path);
}

#[test]
fn file_storage_file_deleted_uses_default() {
    let path = temp_path("deleted");
    cleanup(&path);

    // Write a valid file first
    std::fs::write(&path, "99").expect("write should succeed");

    let shared = Shared::new(0, FileStorageKey::new(&path));
    assert_eq!(*shared.get(), 99);

    // Delete the file
    std::fs::remove_file(&path).expect("delete should succeed");

    // New Shared falls back to default
    let shared2 = Shared::new(0, FileStorageKey::new(&path));
    assert_eq!(*shared2.get(), 0);

    cleanup(&path);
}

#[test]
fn file_storage_struct_roundtrip() {
    #[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
    struct Config {
        name: String,
        count: i32,
        enabled: bool,
    }

    let path = temp_path("struct_roundtrip");
    cleanup(&path);

    let shared = Shared::new(
        Config {
            name: "test".to_string(),
            count: 5,
            enabled: true,
        },
        FileStorageKey::new(&path),
    );

    shared.with_lock(|c| {
        c.count = 10;
        c.enabled = false;
    });

    let reload = Shared::new(
        Config {
            name: String::new(),
            count: 0,
            enabled: false,
        },
        FileStorageKey::new(&path),
    );

    let val = reload.get();
    assert_eq!(val.name, "test");
    assert_eq!(val.count, 10);
    assert!(!val.enabled);

    cleanup(&path);
}

#[test]
fn file_storage_concurrent_writes() {
    let path = temp_path("concurrent_writes");
    cleanup(&path);

    let shared = Shared::new(0, FileStorageKey::new(&path));

    // Multiple sequential mutations
    for i in 1..=100 {
        shared.with_lock(|v| *v = i);
    }

    let reload = Shared::new(0, FileStorageKey::new(&path));
    assert_eq!(*reload.get(), 100);

    cleanup(&path);
}

#[test]
fn file_storage_creates_parent_directory() {
    let mut path = std::env::temp_dir();
    path.push("sharing_instant_test_subdir");
    path.push("nested_file.json");

    // Ensure directory doesn't exist
    let _ = std::fs::remove_dir_all(path.parent().expect("has parent"));

    let shared = Shared::new(42, FileStorageKey::new(&path));
    shared.with_lock(|v| *v = 99);

    assert!(path.exists());

    let reload = Shared::new(0, FileStorageKey::new(&path));
    assert_eq!(*reload.get(), 99);

    // Cleanup
    let _ = std::fs::remove_dir_all(path.parent().expect("has parent"));
}

#[test]
fn file_storage_empty_json_file() {
    let path = temp_path("empty_json");
    cleanup(&path);

    std::fs::write(&path, "").expect("write should succeed");

    // Empty file can't decode, falls back to default
    let shared = Shared::new(vec![1, 2, 3], FileStorageKey::new(&path));
    assert_eq!(&*shared.get(), &[1, 2, 3]);

    cleanup(&path);
}
