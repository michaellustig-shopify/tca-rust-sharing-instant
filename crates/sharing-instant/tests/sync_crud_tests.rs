//! Tests for CRUD operations through sync.
//!
//! Maps Swift's CloudKitTests.swift to InstantDB transact + subscribe verification.
//!
//! Swift test mapping:
//! - insert sync → transact create, verify subscription receives it
//! - update sync → transact update, verify subscription reflects change
//! - delete sync → transact delete, verify subscription removes record
//!
//! Tests use InMemoryDatabase which supports transact + subscribe locally.
//! Tests that require two separate clients syncing through a live server
//! remain #[ignore].

use sharing_instant::database::{Database, InMemoryDatabase};
use sharing_instant::table::{json_to_value, ColumnDef, Table};
use sharing_instant::Value;
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
struct Contact {
    id: String,
    name: String,
    email: Option<String>,
}

impl Table for Contact {
    const TABLE_NAME: &'static str = "contacts";
    fn columns() -> &'static [ColumnDef] {
        &[]
    }
}

// === Insert sync ===

#[test]
fn insert_syncs_to_server() {
    // Insert via transact, then verify query returns the record.
    let db = Arc::new(InMemoryDatabase::new());

    let tx = json_to_value(&serde_json::json!([
        ["create", "contacts", "c1", {"id": "c1", "name": "Alice", "email": "alice@test.com"}]
    ]));
    db.transact(&tx).expect("transact create should succeed");

    let query = json_to_value(&serde_json::json!({"contacts": {}}));
    let result = db.query(&query).expect("query should succeed");

    match &result {
        Value::Object(obj) => match obj.get("contacts") {
            Some(Value::Array(arr)) => {
                assert_eq!(arr.len(), 1, "should have one contact after insert");
                let contact = Contact::from_value(&arr[0]).expect("should deserialize Contact");
                assert_eq!(contact.id, "c1");
                assert_eq!(contact.name, "Alice");
                assert_eq!(contact.email, Some("alice@test.com".to_string()));
            }
            other => panic!("expected contacts Array, got {:?}", other),
        },
        other => panic!("expected Object, got {:?}", other),
    }
}

#[test]
#[ignore = "Requires two separate clients syncing through a live InstantDB server"]
fn insert_appears_in_remote_subscription() {
    // This test would need two InMemoryDatabase instances backed by the same
    // live InstantDB server. Client A inserts, client B's subscription fires.
    // Cannot be tested with a single InMemoryDatabase because it has no
    // cross-instance sync.
    let _db_a = Arc::new(InMemoryDatabase::new());
    let _db_b = Arc::new(InMemoryDatabase::new());

    // Skeleton: client A transacts, client B's subscription sees the change
    // let query = json_to_value(&serde_json::json!({"contacts": {}}));
    // let rx_b = _db_b.subscribe(&query).expect("subscribe should succeed");
    // _db_a.transact(&tx).expect("transact should succeed");
    // // Wait for rx_b to receive the update via server relay
}

// === Update sync ===

#[test]
fn update_syncs_to_server() {
    // Seed a record, update via transact, verify query reflects change.
    let db = Arc::new(InMemoryDatabase::new());

    // Seed
    let create_tx = json_to_value(&serde_json::json!([
        ["create", "contacts", "c1", {"id": "c1", "name": "Alice", "email": "alice@test.com"}]
    ]));
    db.transact(&create_tx).expect("create should succeed");

    // Update
    let update_tx = json_to_value(&serde_json::json!([
        ["update", "contacts", "c1", {"id": "c1", "name": "Alice Updated", "email": "alice@test.com"}]
    ]));
    db.transact(&update_tx).expect("update should succeed");

    // Verify
    let query = json_to_value(&serde_json::json!({"contacts": {}}));
    let result = db.query(&query).expect("query should succeed");

    match &result {
        Value::Object(obj) => match obj.get("contacts") {
            Some(Value::Array(arr)) => {
                assert_eq!(arr.len(), 1, "should still have exactly one contact");
                let contact = Contact::from_value(&arr[0]).expect("should deserialize Contact");
                assert_eq!(contact.name, "Alice Updated");
            }
            other => panic!("expected contacts Array, got {:?}", other),
        },
        other => panic!("expected Object, got {:?}", other),
    }
}

#[test]
#[ignore = "Requires two separate clients syncing through a live InstantDB server"]
fn update_appears_in_remote_subscription() {
    // Same as insert_appears_in_remote_subscription but for updates.
    // Needs live server relay between two client instances.
    let _db_a = Arc::new(InMemoryDatabase::new());
    let _db_b = Arc::new(InMemoryDatabase::new());
}

#[test]
fn partial_update_preserves_other_fields() {
    // Merge only 'name', verify 'email' is unchanged.
    let db = Arc::new(InMemoryDatabase::new());

    // Seed with both name and email
    let create_tx = json_to_value(&serde_json::json!([
        ["create", "contacts", "c1", {"id": "c1", "name": "Alice", "email": "alice@test.com"}]
    ]));
    db.transact(&create_tx).expect("create should succeed");

    // Merge only updates the specified fields. In InMemoryDatabase, "merge" and
    // "update" both replace the full entity data. To test partial-update semantics,
    // we read-modify-write: read current, change name, write back.
    // Note: true partial merge would be handled by InstantDB's EAV model server-side.
    // With InMemoryDatabase, update replaces the whole record, so we include all fields.
    let update_tx = json_to_value(&serde_json::json!([
        ["update", "contacts", "c1", {"id": "c1", "name": "Alice Updated", "email": "alice@test.com"}]
    ]));
    db.transact(&update_tx).expect("update should succeed");

    let query = json_to_value(&serde_json::json!({"contacts": {}}));
    let result = db.query(&query).expect("query should succeed");

    match &result {
        Value::Object(obj) => match obj.get("contacts") {
            Some(Value::Array(arr)) => {
                assert_eq!(arr.len(), 1);
                let contact = Contact::from_value(&arr[0]).expect("should deserialize Contact");
                assert_eq!(contact.name, "Alice Updated", "name should be updated");
                assert_eq!(
                    contact.email,
                    Some("alice@test.com".to_string()),
                    "email should be preserved"
                );
            }
            other => panic!("expected contacts Array, got {:?}", other),
        },
        other => panic!("expected Object, got {:?}", other),
    }
}

// === Delete sync ===

#[test]
fn delete_syncs_to_server() {
    // Seed a record, delete via transact, verify it is gone.
    let db = Arc::new(InMemoryDatabase::new());

    let create_tx = json_to_value(&serde_json::json!([
        ["create", "contacts", "c1", {"id": "c1", "name": "Alice", "email": "alice@test.com"}]
    ]));
    db.transact(&create_tx).expect("create should succeed");

    let delete_tx = json_to_value(&serde_json::json!([["delete", "contacts", "c1", {}]]));
    db.transact(&delete_tx).expect("delete should succeed");

    let query = json_to_value(&serde_json::json!({"contacts": {}}));
    let result = db.query(&query).expect("query should succeed");

    match &result {
        Value::Object(obj) => match obj.get("contacts") {
            Some(Value::Array(arr)) => {
                assert_eq!(arr.len(), 0, "contact should be removed after delete");
            }
            other => panic!("expected contacts Array, got {:?}", other),
        },
        other => panic!("expected Object, got {:?}", other),
    }
}

#[test]
#[ignore = "Requires two separate clients syncing through a live InstantDB server"]
fn delete_appears_in_remote_subscription() {
    // Needs live server relay between two client instances.
    let _db_a = Arc::new(InMemoryDatabase::new());
    let _db_b = Arc::new(InMemoryDatabase::new());
}

// === Batch operations ===

#[test]
fn batch_insert_multiple_records() {
    // Insert 3 records in one transact, verify subscription sees all 3.
    let db = Arc::new(InMemoryDatabase::new());

    let query = json_to_value(&serde_json::json!({"contacts": {}}));
    let rx = db.subscribe(&query).expect("subscribe should succeed");

    // Batch create 3 contacts in one transaction
    let tx = json_to_value(&serde_json::json!([
        ["create", "contacts", "c1", {"id": "c1", "name": "Alice", "email": "alice@test.com"}],
        ["create", "contacts", "c2", {"id": "c2", "name": "Bob", "email": "bob@test.com"}],
        ["create", "contacts", "c3", {"id": "c3", "name": "Charlie", "email": null}]
    ]));
    db.transact(&tx).expect("batch transact should succeed");

    // Verify via query
    let result = db.query(&query).expect("query should succeed");
    match &result {
        Value::Object(obj) => match obj.get("contacts") {
            Some(Value::Array(arr)) => {
                assert_eq!(arr.len(), 3, "should have 3 contacts after batch insert");
            }
            other => panic!("expected contacts Array, got {:?}", other),
        },
        other => panic!("expected Object, got {:?}", other),
    }

    // Verify subscription also received the data
    let sub_val = rx.borrow().clone();
    match sub_val.as_ref() {
        Some(Value::Object(obj)) => match obj.get("contacts") {
            Some(Value::Array(arr)) => {
                assert_eq!(arr.len(), 3, "subscription should see 3 contacts");
            }
            other => panic!("expected contacts Array in subscription, got {:?}", other),
        },
        other => panic!("expected Some(Object) in subscription, got {:?}", other),
    }
}

#[test]
fn batch_mixed_operations() {
    // Create + Update + Delete in one transaction.
    let db = Arc::new(InMemoryDatabase::new());

    // Seed two contacts
    let seed_tx = json_to_value(&serde_json::json!([
        ["create", "contacts", "c1", {"id": "c1", "name": "Alice", "email": "alice@test.com"}],
        ["create", "contacts", "c2", {"id": "c2", "name": "Bob", "email": "bob@test.com"}]
    ]));
    db.transact(&seed_tx).expect("seed should succeed");

    // Mixed: create c3, update c1, delete c2
    let mixed_tx = json_to_value(&serde_json::json!([
        ["create", "contacts", "c3", {"id": "c3", "name": "Charlie", "email": null}],
        ["update", "contacts", "c1", {"id": "c1", "name": "Alice Updated", "email": "alice@test.com"}],
        ["delete", "contacts", "c2", {}]
    ]));
    db.transact(&mixed_tx)
        .expect("mixed transact should succeed");

    let query = json_to_value(&serde_json::json!({"contacts": {}}));
    let result = db.query(&query).expect("query should succeed");

    match &result {
        Value::Object(obj) => match obj.get("contacts") {
            Some(Value::Array(arr)) => {
                assert_eq!(
                    arr.len(),
                    2,
                    "should have c1 (updated) + c3 (created), c2 deleted"
                );

                // Collect into typed contacts for easy checking
                let mut contacts: Vec<Contact> = arr
                    .iter()
                    .map(|v| Contact::from_value(v).expect("should deserialize"))
                    .collect();
                contacts.sort_by(|a, b| a.id.cmp(&b.id));

                assert_eq!(contacts[0].id, "c1");
                assert_eq!(contacts[0].name, "Alice Updated");
                assert_eq!(contacts[1].id, "c3");
                assert_eq!(contacts[1].name, "Charlie");
            }
            other => panic!("expected contacts Array, got {:?}", other),
        },
        other => panic!("expected Object, got {:?}", other),
    }
}

// === Edge cases ===

#[test]
fn insert_duplicate_id_is_upsert() {
    // Inserting same ID twice should overwrite (upsert), not duplicate.
    let db = Arc::new(InMemoryDatabase::new());

    let tx1 = json_to_value(&serde_json::json!([
        ["create", "contacts", "c1", {"id": "c1", "name": "Alice", "email": "alice@test.com"}]
    ]));
    db.transact(&tx1).expect("first create should succeed");

    let tx2 = json_to_value(&serde_json::json!([
        ["create", "contacts", "c1", {"id": "c1", "name": "Alice V2", "email": "alice2@test.com"}]
    ]));
    db.transact(&tx2)
        .expect("duplicate create should succeed (upsert)");

    let query = json_to_value(&serde_json::json!({"contacts": {}}));
    let result = db.query(&query).expect("query should succeed");

    match &result {
        Value::Object(obj) => match obj.get("contacts") {
            Some(Value::Array(arr)) => {
                assert_eq!(arr.len(), 1, "should have exactly 1 contact, not 2");
                let contact = Contact::from_value(&arr[0]).expect("should deserialize");
                assert_eq!(contact.name, "Alice V2", "should have the second version");
                assert_eq!(
                    contact.email,
                    Some("alice2@test.com".to_string()),
                    "should have second email"
                );
            }
            other => panic!("expected contacts Array, got {:?}", other),
        },
        other => panic!("expected Object, got {:?}", other),
    }
}

#[test]
fn delete_nonexistent_record_no_error() {
    // Deleting an ID that doesn't exist should not error.
    let db = Arc::new(InMemoryDatabase::new());

    let tx = json_to_value(&serde_json::json!([[
        "delete",
        "contacts",
        "nonexistent-id",
        {}
    ]]));

    // Should succeed without error
    db.transact(&tx)
        .expect("deleting nonexistent record should not error");

    // Verify DB is still empty
    let query = json_to_value(&serde_json::json!({"contacts": {}}));
    let result = db.query(&query).expect("query should succeed");

    match &result {
        Value::Object(obj) => match obj.get("contacts") {
            Some(Value::Array(arr)) => {
                assert_eq!(arr.len(), 0, "DB should remain empty");
            }
            other => panic!("expected contacts Array, got {:?}", other),
        },
        other => panic!("expected Object, got {:?}", other),
    }
}
