//! Tests for InMemoryDatabase.
//!
//! Mirrors patterns from Swift's IntegrationTests.swift.

use sharing_instant::database::{Database, InMemoryDatabase};
use sharing_instant::table::json_to_value;
use sharing_instant::Value;

#[test]
fn empty_database_returns_empty_results() {
    let db = InMemoryDatabase::new();
    let query = json_to_value(&serde_json::json!({ "reminders": {} }));
    let result = db.query(&query).unwrap();

    match result {
        Value::Object(obj) => {
            assert!(matches!(obj.get("reminders"), Some(Value::Array(arr)) if arr.is_empty()));
        }
        _ => panic!("expected Object"),
    }
}

#[test]
fn insert_and_query() {
    let db = InMemoryDatabase::new();

    db.insert(
        "reminders",
        "r1",
        serde_json::json!({
            "id": "r1",
            "title": "Buy milk",
            "isCompleted": false,
        }),
    );

    let query = json_to_value(&serde_json::json!({ "reminders": {} }));
    let result = db.query(&query).unwrap();

    match result {
        Value::Object(obj) => match obj.get("reminders") {
            Some(Value::Array(arr)) => {
                assert_eq!(arr.len(), 1);
            }
            _ => panic!("expected Array"),
        },
        _ => panic!("expected Object"),
    }
}

#[test]
fn insert_multiple_entities() {
    let db = InMemoryDatabase::new();

    db.insert(
        "reminders",
        "r1",
        serde_json::json!({"id": "r1", "title": "Buy milk"}),
    );
    db.insert(
        "reminders",
        "r2",
        serde_json::json!({"id": "r2", "title": "Walk dog"}),
    );
    db.insert(
        "reminders",
        "r3",
        serde_json::json!({"id": "r3", "title": "Read book"}),
    );

    let query = json_to_value(&serde_json::json!({ "reminders": {} }));
    let result = db.query(&query).unwrap();

    match result {
        Value::Object(obj) => match obj.get("reminders") {
            Some(Value::Array(arr)) => assert_eq!(arr.len(), 3),
            _ => panic!("expected Array"),
        },
        _ => panic!("expected Object"),
    }
}

#[test]
fn remove_entity() {
    let db = InMemoryDatabase::new();

    db.insert(
        "reminders",
        "r1",
        serde_json::json!({"id": "r1", "title": "Buy milk"}),
    );
    db.insert(
        "reminders",
        "r2",
        serde_json::json!({"id": "r2", "title": "Walk dog"}),
    );

    db.remove("reminders", "r1");

    let query = json_to_value(&serde_json::json!({ "reminders": {} }));
    let result = db.query(&query).unwrap();

    match result {
        Value::Object(obj) => match obj.get("reminders") {
            Some(Value::Array(arr)) => assert_eq!(arr.len(), 1),
            _ => panic!("expected Array"),
        },
        _ => panic!("expected Object"),
    }
}

#[test]
fn transact_create() {
    let db = InMemoryDatabase::new();

    let tx = json_to_value(&serde_json::json!([
        ["create", "reminders", "r1", {"id": "r1", "title": "Buy milk"}]
    ]));

    db.transact(&tx).unwrap();

    let query = json_to_value(&serde_json::json!({ "reminders": {} }));
    let result = db.query(&query).unwrap();

    match result {
        Value::Object(obj) => match obj.get("reminders") {
            Some(Value::Array(arr)) => assert_eq!(arr.len(), 1),
            _ => panic!("expected Array"),
        },
        _ => panic!("expected Object"),
    }
}

#[test]
fn transact_update() {
    let db = InMemoryDatabase::new();

    db.insert(
        "reminders",
        "r1",
        serde_json::json!({"id": "r1", "title": "Buy milk", "isCompleted": false}),
    );

    let tx = json_to_value(&serde_json::json!([
        ["update", "reminders", "r1", {"id": "r1", "title": "Buy milk", "isCompleted": true}]
    ]));

    db.transact(&tx).unwrap();

    let query = json_to_value(&serde_json::json!({ "reminders": {} }));
    let result = db.query(&query).unwrap();

    match result {
        Value::Object(obj) => {
            match obj.get("reminders") {
                Some(Value::Array(arr)) => {
                    assert_eq!(arr.len(), 1);
                    // Check the updated value
                    match &arr[0] {
                        Value::Object(r) => {
                            assert!(matches!(r.get("isCompleted"), Some(Value::Bool(true))));
                        }
                        _ => panic!("expected Object"),
                    }
                }
                _ => panic!("expected Array"),
            }
        }
        _ => panic!("expected Object"),
    }
}

#[test]
fn transact_delete() {
    let db = InMemoryDatabase::new();

    db.insert(
        "reminders",
        "r1",
        serde_json::json!({"id": "r1", "title": "Buy milk"}),
    );

    let tx = json_to_value(&serde_json::json!([["delete", "reminders", "r1", {}]]));

    db.transact(&tx).unwrap();

    let query = json_to_value(&serde_json::json!({ "reminders": {} }));
    let result = db.query(&query).unwrap();

    match result {
        Value::Object(obj) => match obj.get("reminders") {
            Some(Value::Array(arr)) => assert_eq!(arr.len(), 0),
            _ => panic!("expected Array"),
        },
        _ => panic!("expected Object"),
    }
}

#[test]
fn transact_invalid_format() {
    let db = InMemoryDatabase::new();
    let tx = Value::String("invalid".to_string());
    assert!(db.transact(&tx).is_err());
}

#[test]
fn query_multiple_tables() {
    let db = InMemoryDatabase::new();

    db.insert("reminders", "r1", serde_json::json!({"id": "r1"}));
    db.insert("lists", "l1", serde_json::json!({"id": "l1"}));

    let query = json_to_value(&serde_json::json!({
        "reminders": {},
        "lists": {}
    }));

    let result = db.query(&query).unwrap();

    match result {
        Value::Object(obj) => {
            assert!(matches!(obj.get("reminders"), Some(Value::Array(arr)) if arr.len() == 1));
            assert!(matches!(obj.get("lists"), Some(Value::Array(arr)) if arr.len() == 1));
        }
        _ => panic!("expected Object"),
    }
}

#[test]
fn subscription_notified_on_insert() {
    let db = InMemoryDatabase::new();
    let query = json_to_value(&serde_json::json!({ "reminders": {} }));

    let rx = db.subscribe(&query).unwrap();

    // Initial result should be empty
    {
        let val = rx.borrow().clone();
        match val.as_ref() {
            Some(Value::Object(obj)) => {
                assert!(matches!(obj.get("reminders"), Some(Value::Array(arr)) if arr.is_empty()));
            }
            _ => panic!("expected initial result"),
        }
    }

    // Insert should trigger notification
    db.insert(
        "reminders",
        "r1",
        serde_json::json!({"id": "r1", "title": "Buy milk"}),
    );

    {
        let val = rx.borrow().clone();
        match val.as_ref() {
            Some(Value::Object(obj)) => {
                assert!(matches!(obj.get("reminders"), Some(Value::Array(arr)) if arr.len() == 1));
            }
            _ => panic!("expected updated result"),
        }
    }
}
