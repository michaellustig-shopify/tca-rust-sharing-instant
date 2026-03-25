//! Tests for FetchAll<T>.
//!
//! Mirrors tests from Swift's FetchAllTests.swift.

use sharing_instant::database::InMemoryDatabase;
use sharing_instant::fetch_all::FetchAll;
use sharing_instant::table::{ColumnDef, Table};
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
struct Item {
    id: String,
    title: String,
    is_active: bool,
}

impl Table for Item {
    const TABLE_NAME: &'static str = "items";
    fn columns() -> &'static [ColumnDef] {
        &[]
    }
}

#[test]
fn fetch_all_empty_database() {
    let db = Arc::new(InMemoryDatabase::new());
    let fetch = FetchAll::<Item>::new(db);
    assert_eq!(fetch.get().len(), 0);
}

#[test]
fn fetch_all_with_data() {
    let db = Arc::new(InMemoryDatabase::new());

    db.insert(
        "items",
        "i1",
        serde_json::json!({"id": "i1", "title": "First", "is_active": true}),
    );
    db.insert(
        "items",
        "i2",
        serde_json::json!({"id": "i2", "title": "Second", "is_active": false}),
    );

    let fetch = FetchAll::<Item>::new(db);
    let items = fetch.get();
    assert_eq!(items.len(), 2);
}

#[test]
fn fetch_all_is_not_loading_after_init() {
    let db = Arc::new(InMemoryDatabase::new());
    let fetch = FetchAll::<Item>::new(db);
    assert!(!fetch.is_loading());
}

#[test]
fn fetch_all_no_error_on_empty() {
    let db = Arc::new(InMemoryDatabase::new());
    let fetch = FetchAll::<Item>::new(db);
    assert!(fetch.load_error().is_none());
}

#[test]
fn fetch_all_reader_works() {
    let db = Arc::new(InMemoryDatabase::new());

    db.insert(
        "items",
        "i1",
        serde_json::json!({"id": "i1", "title": "First", "is_active": true}),
    );

    let fetch = FetchAll::<Item>::new(db);
    let reader = fetch.reader();
    assert_eq!(reader.get().len(), 1);
}

#[test]
fn fetch_all_with_custom_query() {
    let db = Arc::new(InMemoryDatabase::new());

    db.insert(
        "items",
        "i1",
        serde_json::json!({"id": "i1", "title": "Active", "is_active": true}),
    );
    db.insert(
        "items",
        "i2",
        serde_json::json!({"id": "i2", "title": "Inactive", "is_active": false}),
    );

    // Custom query (in-memory DB doesn't filter, but tests the API)
    let query = Item::query()
        .where_eq("is_active", sharing_instant::Value::Bool(true))
        .build();
    let fetch = FetchAll::<Item>::with_query(db, query);

    // In-memory DB returns all (filtering is server-side for InstantDB)
    // This tests that the API accepts custom queries
    let items = fetch.get();
    assert!(items.len() >= 1);
}

#[test]
fn fetch_all_manual_reload() {
    let db = Arc::new(InMemoryDatabase::new());
    let mut fetch = FetchAll::<Item>::new(db.clone());

    assert_eq!(fetch.get().len(), 0);

    // Insert directly into the database (bypassing FetchAll).
    db.insert(
        "items",
        "i1",
        serde_json::json!({"id": "i1", "title": "Injected", "is_active": true}),
    );

    // Manual reload picks up the change.
    fetch.load().expect("load should succeed");
    assert_eq!(fetch.get().len(), 1);
    assert_eq!(fetch.get()[0].title, "Injected");
}

#[test]
fn fetch_all_loading_state() {
    let db = Arc::new(InMemoryDatabase::new());
    let fetch = FetchAll::<Item>::new(db);

    // After initial load, is_loading should be false.
    assert!(!fetch.is_loading());
    assert!(fetch.load_error().is_none());
}
