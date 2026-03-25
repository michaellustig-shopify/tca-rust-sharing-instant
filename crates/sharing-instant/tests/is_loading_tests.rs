//! Tests for is_loading state tracking.
//!
//! Maps Swift's IsLoadingTests.swift to FetchAll/FetchOne loading states.

use sharing_instant::database::InMemoryDatabase;
use sharing_instant::fetch_all::FetchAll;
use sharing_instant::fetch_one::FetchOne;
use sharing_instant::table::{ColumnDef, Table};
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
struct Widget {
    id: String,
    name: String,
}

impl Table for Widget {
    const TABLE_NAME: &'static str = "widgets";
    fn columns() -> &'static [ColumnDef] {
        &[]
    }
}

#[test]
fn fetch_all_not_loading_after_sync_init() {
    let db = Arc::new(InMemoryDatabase::new());
    let fetch = FetchAll::<Widget>::new(db);
    // InMemoryDatabase loads synchronously, so is_loading should be false
    assert!(!fetch.is_loading());
}

#[test]
fn fetch_one_not_loading_after_sync_init() {
    let db = Arc::new(InMemoryDatabase::new());
    let fetch = FetchOne::<Widget>::new(db);
    assert!(!fetch.is_loading());
}

#[test]
fn fetch_all_not_loading_with_data() {
    let db = Arc::new(InMemoryDatabase::new());
    db.insert(
        "widgets",
        "w1",
        serde_json::json!({"id": "w1", "name": "Widget 1"}),
    );
    let fetch = FetchAll::<Widget>::new(db);
    assert!(!fetch.is_loading());
}

#[test]
fn fetch_one_not_loading_with_data() {
    let db = Arc::new(InMemoryDatabase::new());
    db.insert(
        "widgets",
        "w1",
        serde_json::json!({"id": "w1", "name": "Widget 1"}),
    );
    let fetch = FetchOne::<Widget>::new(db);
    assert!(!fetch.is_loading());
}

#[test]
fn fetch_all_no_load_error_on_success() {
    let db = Arc::new(InMemoryDatabase::new());
    let fetch = FetchAll::<Widget>::new(db);
    assert!(fetch.load_error().is_none());
}

#[test]
fn fetch_all_no_load_error_with_data() {
    let db = Arc::new(InMemoryDatabase::new());
    db.insert(
        "widgets",
        "w1",
        serde_json::json!({"id": "w1", "name": "Widget 1"}),
    );
    let fetch = FetchAll::<Widget>::new(db);
    assert!(fetch.load_error().is_none());
}
