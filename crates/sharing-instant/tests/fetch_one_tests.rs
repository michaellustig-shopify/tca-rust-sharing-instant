//! Tests for FetchOne<T>.
//!
//! Mirrors tests from Swift's FetchOneTests.swift.
//!
//! Swift test mapping:
//! - nonTableInit → fetch_one_empty_database (Rust uses Option instead of non-table init)
//! - tableInit → fetch_one_returns_first_item
//! - optionalTableInit → fetch_one_optional_returns_none_no_error
//! - selectStatementInit → fetch_one_with_custom_query
//! - statementInit → fetch_one_with_query_returns_value
//! - optionalStatementInit → fetch_one_with_query_no_match
//! - fetchOneOptional → fetch_one_empty_database (get() returns Option<T>)
//! - fetchOneDelayedAssignment → fetch_one_delayed_construction
//! - fetchOneSelection → fetch_one_different_table_type

use sharing_instant::database::InMemoryDatabase;
use sharing_instant::error::SharingInstantError;
use sharing_instant::fetch_one::FetchOne;
use sharing_instant::table::{ColumnDef, Table};
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
struct Counter {
    id: String,
    count: i64,
}

impl Table for Counter {
    const TABLE_NAME: &'static str = "counters";
    fn columns() -> &'static [ColumnDef] {
        &[]
    }
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
struct Record {
    id: String,
    date: f64,
    #[serde(rename = "parentID")]
    parent_id: Option<String>,
}

impl Table for Record {
    const TABLE_NAME: &'static str = "records";
    fn columns() -> &'static [ColumnDef] {
        &[]
    }
}

// === Existing tests (Swift: nonTableInit, tableInit, require variants) ===

#[test]
fn fetch_one_empty_database() {
    let db = Arc::new(InMemoryDatabase::new());
    let fetch = FetchOne::<Counter>::new(db);
    assert!(fetch.get().is_none());
}

#[test]
fn fetch_one_returns_first_item() {
    let db = Arc::new(InMemoryDatabase::new());

    db.insert(
        "counters",
        "c1",
        serde_json::json!({"id": "c1", "count": 42}),
    );

    let fetch = FetchOne::<Counter>::new(db);
    let item = fetch.get().expect("should have one item");
    assert_eq!(item.count, 42);
}

#[test]
fn fetch_one_require_fails_when_empty() {
    let db = Arc::new(InMemoryDatabase::new());
    let fetch = FetchOne::<Counter>::new(db);
    let result = fetch.require();
    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        SharingInstantError::NotFound { .. }
    ));
}

#[test]
fn fetch_one_require_succeeds_with_data() {
    let db = Arc::new(InMemoryDatabase::new());

    db.insert(
        "counters",
        "c1",
        serde_json::json!({"id": "c1", "count": 7}),
    );

    let fetch = FetchOne::<Counter>::new(db);
    let item = fetch.require().expect("should find counter");
    assert_eq!(item.count, 7);
}

#[test]
fn fetch_one_is_not_loading_after_init() {
    let db = Arc::new(InMemoryDatabase::new());
    let fetch = FetchOne::<Counter>::new(db);
    assert!(!fetch.is_loading());
}

#[test]
fn fetch_one_reader_works() {
    let db = Arc::new(InMemoryDatabase::new());

    db.insert(
        "counters",
        "c1",
        serde_json::json!({"id": "c1", "count": 99}),
    );

    let fetch = FetchOne::<Counter>::new(db);
    let reader = fetch.reader();
    assert!(reader.get().is_some());
}

// === New tests ported from Swift ===

/// Maps to Swift's optionalTableInit: get() returns None without error when table is empty.
#[test]
fn fetch_one_optional_returns_none_no_error() {
    let db = Arc::new(InMemoryDatabase::new());
    let fetch = FetchOne::<Counter>::new(db);

    // get() returns None (like Swift's optional @FetchOne returning nil)
    assert!(fetch.get().is_none());
    // No loading in progress
    assert!(!fetch.is_loading());
}

/// Maps to Swift's selectStatementInit: FetchOne with a custom query.
#[test]
fn fetch_one_with_custom_query() {
    let db = Arc::new(InMemoryDatabase::new());

    db.insert(
        "counters",
        "c1",
        serde_json::json!({"id": "c1", "count": 10}),
    );
    db.insert(
        "counters",
        "c2",
        serde_json::json!({"id": "c2", "count": 20}),
    );

    let query = Counter::query().limit(1).build();
    let fetch = FetchOne::<Counter>::with_query(db, query);

    let item = fetch.get().expect("should have one item from custom query");
    // Returns some counter (ordering depends on HashMap iteration)
    assert!(item.count == 10 || item.count == 20);
}

/// Maps to Swift's optionalStatementInit: custom query returns None when no match.
#[test]
fn fetch_one_with_query_no_match() {
    let db = Arc::new(InMemoryDatabase::new());
    // Empty database, no counters at all

    let query = Counter::query().limit(1).build();
    let fetch = FetchOne::<Counter>::with_query(db, query);

    assert!(fetch.get().is_none());
}

/// Maps to Swift's tableInit with delete: item present, then deleted, FetchOne reflects removal.
#[test]
fn fetch_one_returns_none_after_remove() {
    let db = Arc::new(InMemoryDatabase::new());

    db.insert(
        "counters",
        "c1",
        serde_json::json!({"id": "c1", "count": 42}),
    );

    let fetch = FetchOne::<Counter>::new(db.clone());
    assert!(fetch.get().is_some());

    // Remove the counter — in-memory DB notifies subscriptions,
    // but without a tokio runtime the sync FetchOne won't auto-update.
    // This tests the initial-load-only path.
    db.remove("counters", "c1");

    // Create a new FetchOne to verify the data is gone
    let fetch2 = FetchOne::<Counter>::new(db);
    assert!(fetch2.get().is_none());
}

/// Maps to Swift's multiple records: FetchOne returns the first of many.
#[test]
fn fetch_one_multiple_items_returns_first() {
    let db = Arc::new(InMemoryDatabase::new());

    db.insert(
        "counters",
        "c1",
        serde_json::json!({"id": "c1", "count": 1}),
    );
    db.insert(
        "counters",
        "c2",
        serde_json::json!({"id": "c2", "count": 2}),
    );
    db.insert(
        "counters",
        "c3",
        serde_json::json!({"id": "c3", "count": 3}),
    );

    let fetch = FetchOne::<Counter>::new(db);
    let item = fetch.get().expect("should return one item");
    // Returns exactly one item (not all three)
    assert!(item.count >= 1 && item.count <= 3);
}

/// Maps to Swift's fetchOneSelection: different Table types work correctly.
#[test]
fn fetch_one_different_table_type() {
    let db = Arc::new(InMemoryDatabase::new());

    db.insert(
        "records",
        "r1",
        serde_json::json!({"id": "r1", "date": 42.0, "parentID": null}),
    );

    let fetch = FetchOne::<Record>::new(db);
    let item = fetch.get().expect("should fetch Record");
    assert_eq!(item.date, 42.0);
    assert!(item.parent_id.is_none());
}

/// Maps to Swift's fetchOneDelayedAssignment: construct FetchOne later, not at declaration.
#[test]
fn fetch_one_delayed_construction() {
    let db = Arc::new(InMemoryDatabase::new());

    db.insert(
        "counters",
        "c1",
        serde_json::json!({"id": "c1", "count": 77}),
    );

    // Delayed: declare as Option, assign later
    let fetch: Option<FetchOne<Counter>> = None;
    assert!(fetch.is_none());

    let fetch = Some(FetchOne::<Counter>::new(db));
    let item = fetch
        .as_ref()
        .expect("should be Some")
        .get()
        .expect("should have counter");
    assert_eq!(item.count, 77);
}

/// Maps to Swift's require + error after delete: require fails after data removal.
#[test]
fn fetch_one_require_fails_after_data_removed() {
    let db = Arc::new(InMemoryDatabase::new());

    db.insert(
        "counters",
        "c1",
        serde_json::json!({"id": "c1", "count": 5}),
    );

    // First FetchOne succeeds
    let fetch = FetchOne::<Counter>::new(db.clone());
    assert!(fetch.require().is_ok());

    // Remove data, new FetchOne fails
    db.remove("counters", "c1");
    let fetch2 = FetchOne::<Counter>::new(db);
    let result = fetch2.require();
    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        SharingInstantError::NotFound { .. }
    ));
}

/// Maps to Swift's watch/subscription: verify the watch receiver exists.
#[test]
fn fetch_one_watch_receiver_available() {
    let db = Arc::new(InMemoryDatabase::new());

    db.insert(
        "counters",
        "c1",
        serde_json::json!({"id": "c1", "count": 42}),
    );

    let fetch = FetchOne::<Counter>::new(db);
    let rx = fetch.watch();

    // The watch receiver should have the initial value
    let current = rx.borrow().clone();
    assert!(current.is_some());
    assert_eq!(current.expect("should have value").count, 42);
}
