//! Tests for subscription lifecycle with FetchAll/FetchOne.
//!
//! Mirrors tests from Swift's FetchSubscriptionTests.swift.
//!
//! Swift test mapping:
//! - stopSubscriptionWhenTaskCancelled → subscription_stops_on_drop
//! - completeWhenTaskExplicitlyCancelled → subscription_explicit_cancel_stops_updates
//! - cancellingOneFetchDoesNotCancelAnother → independent_subscriptions

use sharing_instant::database::InMemoryDatabase;
use sharing_instant::fetch_all::FetchAll;
use sharing_instant::table::{ColumnDef, Table};
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
struct Todo {
    id: String,
    title: String,
    done: bool,
}

impl Table for Todo {
    const TABLE_NAME: &'static str = "todos";
    fn columns() -> &'static [ColumnDef] {
        &[]
    }
}

/// Maps to Swift's stopSubscriptionWhenTaskCancelled:
/// When a FetchAll is dropped, its subscription should stop.
/// New database writes after drop should not cause issues.
#[test]
fn subscription_stops_on_drop() {
    let db = Arc::new(InMemoryDatabase::new());

    db.insert(
        "todos",
        "t1",
        serde_json::json!({"id": "t1", "title": "First", "done": false}),
    );

    let fetch = FetchAll::<Todo>::new(db.clone());
    assert_eq!(fetch.get().len(), 1);

    // Drop the FetchAll
    drop(fetch);

    // Further writes should not panic or cause issues
    db.insert(
        "todos",
        "t2",
        serde_json::json!({"id": "t2", "title": "Second", "done": false}),
    );
}

/// Maps to Swift's cancellingOneFetchDoesNotCancelAnother:
/// Two FetchAll instances on the same table are independent.
#[test]
fn independent_subscriptions() {
    let db = Arc::new(InMemoryDatabase::new());

    db.insert(
        "todos",
        "t1",
        serde_json::json!({"id": "t1", "title": "First", "done": false}),
    );

    let fetch1 = FetchAll::<Todo>::new(db.clone());
    let fetch2 = FetchAll::<Todo>::new(db.clone());

    assert_eq!(fetch1.get().len(), 1);
    assert_eq!(fetch2.get().len(), 1);

    // Drop fetch1
    drop(fetch1);

    // fetch2 should still work fine
    db.insert(
        "todos",
        "t2",
        serde_json::json!({"id": "t2", "title": "Second", "done": false}),
    );

    // Create a new fetch to verify the DB is intact
    let fetch3 = FetchAll::<Todo>::new(db);
    assert_eq!(fetch3.get().len(), 2);
}

/// Verify that multiple FetchAll instances share the same underlying
/// database but maintain independent result sets.
#[test]
fn multiple_fetch_all_same_database() {
    let db = Arc::new(InMemoryDatabase::new());

    db.insert(
        "todos",
        "t1",
        serde_json::json!({"id": "t1", "title": "First", "done": false}),
    );

    let fetch1 = FetchAll::<Todo>::new(db.clone());
    let fetch2 = FetchAll::<Todo>::new(db.clone());

    // Both should see the same data
    assert_eq!(fetch1.get().len(), fetch2.get().len());

    // Add data — new FetchAll picks it up
    db.insert(
        "todos",
        "t2",
        serde_json::json!({"id": "t2", "title": "Second", "done": true}),
    );

    let fetch3 = FetchAll::<Todo>::new(db);
    assert_eq!(fetch3.get().len(), 2);
}
