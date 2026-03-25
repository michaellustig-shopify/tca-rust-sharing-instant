//! Tests for reactive triggers (subscription notifications on data change).
//!
//! Maps Swift's TriggerTests.swift to InstantDB subscription notifications.
//!
//! Uses InMemoryDatabase which fires subscriptions on any insert/remove/transact.

use sharing_instant::database::{Database, InMemoryDatabase};
use sharing_instant::table::{json_to_value, ColumnDef, Table};
use sharing_instant::Value;
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
struct Note {
    id: String,
    body: String,
}

impl Table for Note {
    const TABLE_NAME: &'static str = "notes";
    fn columns() -> &'static [ColumnDef] {
        &[]
    }
}

#[test]
fn insert_triggers_subscription_notification() {
    // Subscribe to notes, insert data, verify the watch channel receives an update.
    let db = Arc::new(InMemoryDatabase::new());

    let query = json_to_value(&serde_json::json!({"notes": {}}));
    let rx = db.subscribe(&query).expect("subscribe should succeed");

    // Initial state: empty
    {
        let val = rx.borrow().clone();
        match val.as_ref() {
            Some(Value::Object(obj)) => match obj.get("notes") {
                Some(Value::Array(arr)) => assert_eq!(arr.len(), 0, "should start empty"),
                other => panic!("expected notes Array, got {:?}", other),
            },
            other => panic!("expected Some(Object), got {:?}", other),
        }
    }

    // Insert via transact
    let tx = json_to_value(&serde_json::json!([
        ["create", "notes", "n1", {"id": "n1", "body": "Hello world"}]
    ]));
    db.transact(&tx).expect("transact should succeed");

    // Verify watch channel received the update
    {
        let val = rx.borrow().clone();
        match val.as_ref() {
            Some(Value::Object(obj)) => match obj.get("notes") {
                Some(Value::Array(arr)) => {
                    assert_eq!(arr.len(), 1, "subscription should see 1 note after insert");
                    let note = Note::from_value(&arr[0]).expect("should deserialize Note");
                    assert_eq!(note.id, "n1");
                    assert_eq!(note.body, "Hello world");
                }
                other => panic!("expected notes Array, got {:?}", other),
            },
            other => panic!("expected Some(Object), got {:?}", other),
        }
    }
}
