//! Tests for subscription update ordering and change processing.
//!
//! Maps Swift's FetchRecordZoneChangesTests.swift to InstantDB subscription updates.
//!
//! Swift test mapping:
//! - zone changes → subscription updates
//! - child-before-parent → link resolution order
//! - batch changes → ordered notification delivery
//!
//! Tests use InMemoryDatabase which supports transact + subscribe locally.
//! Tests that require reconnection or server-side features remain #[ignore].

use sharing_instant::database::{Database, InMemoryDatabase};
use sharing_instant::table::{json_to_value, ColumnDef, Table};
use sharing_instant::Value;
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
struct Folder {
    id: String,
    name: String,
}

impl Table for Folder {
    const TABLE_NAME: &'static str = "folders";
    fn columns() -> &'static [ColumnDef] {
        &[]
    }
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
struct Document {
    id: String,
    title: String,
    folder_id: Option<String>,
}

impl Table for Document {
    const TABLE_NAME: &'static str = "documents";
    fn columns() -> &'static [ColumnDef] {
        &[]
    }
}

#[test]
fn subscription_receives_insert_update() {
    // Subscribe to folders, insert a folder, verify subscription fires with the new data.
    let db = Arc::new(InMemoryDatabase::new());

    let query = json_to_value(&serde_json::json!({"folders": {}}));
    let rx = db.subscribe(&query).expect("subscribe should succeed");

    // Initial state: empty
    {
        let val = rx.borrow().clone();
        match val.as_ref() {
            Some(Value::Object(obj)) => match obj.get("folders") {
                Some(Value::Array(arr)) => assert_eq!(arr.len(), 0, "should start empty"),
                other => panic!("expected folders Array, got {:?}", other),
            },
            other => panic!("expected Some(Object), got {:?}", other),
        }
    }

    // Insert a folder via transact
    let tx = json_to_value(&serde_json::json!([
        ["create", "folders", "f1", {"id": "f1", "name": "Work"}]
    ]));
    db.transact(&tx).expect("transact should succeed");

    // Subscription should now reflect the insert
    {
        let val = rx.borrow().clone();
        match val.as_ref() {
            Some(Value::Object(obj)) => match obj.get("folders") {
                Some(Value::Array(arr)) => {
                    assert_eq!(
                        arr.len(),
                        1,
                        "subscription should see 1 folder after insert"
                    );
                    let folder = Folder::from_value(&arr[0]).expect("should deserialize Folder");
                    assert_eq!(folder.name, "Work");
                }
                other => panic!("expected folders Array, got {:?}", other),
            },
            other => panic!("expected Some(Object), got {:?}", other),
        }
    }
}

#[test]
fn subscription_receives_delete_update() {
    // Subscribe to folders, insert then delete, verify subscription reflects removal.
    let db = Arc::new(InMemoryDatabase::new());

    // Seed a folder
    let create_tx = json_to_value(&serde_json::json!([
        ["create", "folders", "f1", {"id": "f1", "name": "Work"}]
    ]));
    db.transact(&create_tx).expect("create should succeed");

    let query = json_to_value(&serde_json::json!({"folders": {}}));
    let rx = db.subscribe(&query).expect("subscribe should succeed");

    // Subscription should see the existing folder
    {
        let val = rx.borrow().clone();
        match val.as_ref() {
            Some(Value::Object(obj)) => match obj.get("folders") {
                Some(Value::Array(arr)) => assert_eq!(arr.len(), 1, "should see 1 folder"),
                other => panic!("expected folders Array, got {:?}", other),
            },
            other => panic!("expected Some(Object), got {:?}", other),
        }
    }

    // Delete the folder
    let delete_tx = json_to_value(&serde_json::json!([["delete", "folders", "f1", {}]]));
    db.transact(&delete_tx).expect("delete should succeed");

    // Subscription should now be empty
    {
        let val = rx.borrow().clone();
        match val.as_ref() {
            Some(Value::Object(obj)) => match obj.get("folders") {
                Some(Value::Array(arr)) => {
                    assert_eq!(
                        arr.len(),
                        0,
                        "subscription should see 0 folders after delete"
                    );
                }
                other => panic!("expected folders Array, got {:?}", other),
            },
            other => panic!("expected Some(Object), got {:?}", other),
        }
    }
}

#[test]
fn child_before_parent_in_update_batch() {
    // In a single transaction, create a child document before its parent folder.
    // The InMemoryDatabase should handle this because it stores entities by type
    // independently. Link resolution is logical, not enforced at DB level.
    let db = Arc::new(InMemoryDatabase::new());

    // Create child (document) before parent (folder) in the same transaction
    let tx = json_to_value(&serde_json::json!([
        ["create", "documents", "d1", {"id": "d1", "title": "Report", "folder_id": "f1"}],
        ["create", "folders", "f1", {"id": "f1", "name": "Work"}]
    ]));
    db.transact(&tx)
        .expect("child-before-parent transact should succeed");

    // Verify both exist
    let folder_query = json_to_value(&serde_json::json!({"folders": {}}));
    let doc_query = json_to_value(&serde_json::json!({"documents": {}}));

    let folder_result = db
        .query(&folder_query)
        .expect("folder query should succeed");
    let doc_result = db.query(&doc_query).expect("doc query should succeed");

    match &folder_result {
        Value::Object(obj) => match obj.get("folders") {
            Some(Value::Array(arr)) => assert_eq!(arr.len(), 1, "folder should exist"),
            other => panic!("expected folders Array, got {:?}", other),
        },
        other => panic!("expected Object, got {:?}", other),
    }

    match &doc_result {
        Value::Object(obj) => match obj.get("documents") {
            Some(Value::Array(arr)) => {
                assert_eq!(arr.len(), 1, "document should exist");
                let doc = Document::from_value(&arr[0]).expect("should deserialize Document");
                assert_eq!(
                    doc.folder_id,
                    Some("f1".to_string()),
                    "link should be intact"
                );
            }
            other => panic!("expected documents Array, got {:?}", other),
        },
        other => panic!("expected Object, got {:?}", other),
    }
}

#[test]
fn multiple_entity_types_in_single_update() {
    // A single transaction updates both folders and documents.
    // Subscriptions to each entity type should fire.
    let db = Arc::new(InMemoryDatabase::new());

    let folder_query = json_to_value(&serde_json::json!({"folders": {}}));
    let doc_query = json_to_value(&serde_json::json!({"documents": {}}));

    let folder_rx = db
        .subscribe(&folder_query)
        .expect("folder subscribe should succeed");
    let doc_rx = db
        .subscribe(&doc_query)
        .expect("doc subscribe should succeed");

    // One transact that creates both a folder and a document
    let tx = json_to_value(&serde_json::json!([
        ["create", "folders", "f1", {"id": "f1", "name": "Work"}],
        ["create", "documents", "d1", {"id": "d1", "title": "Report", "folder_id": "f1"}]
    ]));
    db.transact(&tx)
        .expect("multi-type transact should succeed");

    // Folder subscription should see the folder
    {
        let val = folder_rx.borrow().clone();
        match val.as_ref() {
            Some(Value::Object(obj)) => match obj.get("folders") {
                Some(Value::Array(arr)) => {
                    assert_eq!(arr.len(), 1, "folder subscription should see 1 folder");
                }
                other => panic!("expected folders Array, got {:?}", other),
            },
            other => panic!("expected Some(Object), got {:?}", other),
        }
    }

    // Document subscription should see the document
    {
        let val = doc_rx.borrow().clone();
        match val.as_ref() {
            Some(Value::Object(obj)) => match obj.get("documents") {
                Some(Value::Array(arr)) => {
                    assert_eq!(arr.len(), 1, "doc subscription should see 1 document");
                }
                other => panic!("expected documents Array, got {:?}", other),
            },
            other => panic!("expected Some(Object), got {:?}", other),
        }
    }
}

#[test]
#[ignore = "Requires live WebSocket connection with disconnect/reconnect capability"]
fn subscription_survives_reconnection() {
    // This test requires a live InstantDB WebSocket connection that can be
    // disconnected and reconnected. InMemoryDatabase has no connection state.
    let _db = Arc::new(InMemoryDatabase::new());

    // Skeleton:
    // 1. Subscribe to a query
    // 2. Insert data, verify subscription fires
    // 3. Simulate disconnect (drop WebSocket)
    // 4. Insert more data while disconnected
    // 5. Reconnect
    // 6. Verify subscription catches up with missed changes
}

#[test]
fn subscription_deduplicates_updates() {
    // Same record updated twice in quick succession. The watch::Receiver only
    // holds the latest value, so the subscription should reflect the final state.
    let db = Arc::new(InMemoryDatabase::new());

    let query = json_to_value(&serde_json::json!({"folders": {}}));
    let rx = db.subscribe(&query).expect("subscribe should succeed");

    // Two rapid updates to the same record
    let tx1 = json_to_value(&serde_json::json!([
        ["create", "folders", "f1", {"id": "f1", "name": "Draft"}]
    ]));
    db.transact(&tx1).expect("first update should succeed");

    let tx2 = json_to_value(&serde_json::json!([
        ["update", "folders", "f1", {"id": "f1", "name": "Final"}]
    ]));
    db.transact(&tx2).expect("second update should succeed");

    // The subscription's borrow() should show the final state
    let val = rx.borrow().clone();
    match val.as_ref() {
        Some(Value::Object(obj)) => match obj.get("folders") {
            Some(Value::Array(arr)) => {
                assert_eq!(arr.len(), 1, "should have exactly 1 folder");
                let folder = Folder::from_value(&arr[0]).expect("should deserialize Folder");
                assert_eq!(folder.name, "Final", "should reflect the latest update");
            }
            other => panic!("expected folders Array, got {:?}", other),
        },
        other => panic!("expected Some(Object), got {:?}", other),
    }
}

#[test]
fn subscription_handles_empty_batch() {
    // Transact with an empty array of steps should not error and should not
    // spuriously trigger subscriptions. Note: tokio watch::Receiver only notifies
    // on actual sends. An empty transact with no actual mutations should be fine.
    let db = Arc::new(InMemoryDatabase::new());

    // Seed one folder so subscription has something
    let seed_tx = json_to_value(&serde_json::json!([
        ["create", "folders", "f1", {"id": "f1", "name": "Work"}]
    ]));
    db.transact(&seed_tx).expect("seed should succeed");

    let query = json_to_value(&serde_json::json!({"folders": {}}));
    let rx = db.subscribe(&query).expect("subscribe should succeed");

    // Empty transact
    let empty_tx = json_to_value(&serde_json::json!([]));
    db.transact(&empty_tx)
        .expect("empty transact should not error");

    // Subscription should still show the original folder, unchanged
    let val = rx.borrow().clone();
    match val.as_ref() {
        Some(Value::Object(obj)) => match obj.get("folders") {
            Some(Value::Array(arr)) => {
                assert_eq!(arr.len(), 1, "should still have 1 folder");
                let folder = Folder::from_value(&arr[0]).expect("should deserialize Folder");
                assert_eq!(folder.name, "Work", "data should be unchanged");
            }
            other => panic!("expected folders Array, got {:?}", other),
        },
        other => panic!("expected Some(Object), got {:?}", other),
    }
}

#[test]
fn subscription_with_nested_link_query() {
    // Subscribe to both folders and documents. Updates to either should propagate.
    let db = Arc::new(InMemoryDatabase::new());

    // Subscribe to a combined query for both entity types
    let query = json_to_value(&serde_json::json!({
        "folders": {},
        "documents": {}
    }));
    let rx = db.subscribe(&query).expect("subscribe should succeed");

    // Insert a folder
    let tx1 = json_to_value(&serde_json::json!([
        ["create", "folders", "f1", {"id": "f1", "name": "Work"}]
    ]));
    db.transact(&tx1).expect("folder create should succeed");

    // Verify subscription sees the folder
    {
        let val = rx.borrow().clone();
        match val.as_ref() {
            Some(Value::Object(obj)) => {
                match obj.get("folders") {
                    Some(Value::Array(arr)) => assert_eq!(arr.len(), 1),
                    other => panic!("expected folders Array, got {:?}", other),
                }
                match obj.get("documents") {
                    Some(Value::Array(arr)) => assert_eq!(arr.len(), 0),
                    other => panic!("expected documents Array, got {:?}", other),
                }
            }
            other => panic!("expected Some(Object), got {:?}", other),
        }
    }

    // Insert a document linked to the folder
    let tx2 = json_to_value(&serde_json::json!([
        ["create", "documents", "d1", {"id": "d1", "title": "Report", "folder_id": "f1"}]
    ]));
    db.transact(&tx2).expect("document create should succeed");

    // Verify subscription sees both
    {
        let val = rx.borrow().clone();
        match val.as_ref() {
            Some(Value::Object(obj)) => {
                match obj.get("folders") {
                    Some(Value::Array(arr)) => assert_eq!(arr.len(), 1, "1 folder"),
                    other => panic!("expected folders Array, got {:?}", other),
                }
                match obj.get("documents") {
                    Some(Value::Array(arr)) => assert_eq!(arr.len(), 1, "1 document"),
                    other => panic!("expected documents Array, got {:?}", other),
                }
            }
            other => panic!("expected Some(Object), got {:?}", other),
        }
    }
}

#[test]
fn large_batch_update_performance() {
    // Insert 100+ records in a single transaction, verify all are processed.
    let db = Arc::new(InMemoryDatabase::new());

    let query = json_to_value(&serde_json::json!({"folders": {}}));
    let rx = db.subscribe(&query).expect("subscribe should succeed");

    // Build a transaction with 150 creates
    let steps: Vec<serde_json::Value> = (0..150)
        .map(|i| {
            serde_json::json!([
                "create",
                "folders",
                format!("f{}", i),
                {"id": format!("f{}", i), "name": format!("Folder {}", i)}
            ])
        })
        .collect();

    let tx = json_to_value(&serde_json::json!(steps));
    db.transact(&tx)
        .expect("large batch transact should succeed");

    // Verify all 150 are present via query
    let result = db.query(&query).expect("query should succeed");
    match &result {
        Value::Object(obj) => match obj.get("folders") {
            Some(Value::Array(arr)) => {
                assert_eq!(arr.len(), 150, "all 150 folders should be present");
            }
            other => panic!("expected folders Array, got {:?}", other),
        },
        other => panic!("expected Object, got {:?}", other),
    }

    // Verify subscription also sees all 150
    let val = rx.borrow().clone();
    match val.as_ref() {
        Some(Value::Object(obj)) => match obj.get("folders") {
            Some(Value::Array(arr)) => {
                assert_eq!(arr.len(), 150, "subscription should see all 150 folders");
            }
            other => panic!("expected folders Array in subscription, got {:?}", other),
        },
        other => panic!("expected Some(Object) in subscription, got {:?}", other),
    }
}

#[test]
#[ignore = "Requires live InstantDB server that provides transaction IDs in responses"]
fn subscription_receives_transaction_id() {
    // InMemoryDatabase does not track transaction IDs. A live InstantDB server
    // returns a processed-tx-id with each subscription update. This test would
    // verify that the tx-id advances monotonically.
    let _db = Arc::new(InMemoryDatabase::new());

    // Skeleton:
    // 1. Subscribe to a query
    // 2. Transact (create), capture tx-id from subscription update
    // 3. Transact (update), capture new tx-id
    // 4. Assert new tx-id > previous tx-id
}

#[test]
#[ignore = "Requires live InstantDB server with checkpoint/resume support"]
fn subscription_reprocesses_from_checkpoint() {
    // InMemoryDatabase has no concept of checkpoints or processed-tx-id.
    // A live server would resume from the last acknowledged tx-id on reconnect.
    let _db = Arc::new(InMemoryDatabase::new());

    // Skeleton:
    // 1. Subscribe, process updates up to tx-id X
    // 2. Disconnect
    // 3. Server receives more mutations (tx-id Y > X)
    // 4. Reconnect with last-tx-id = X
    // 5. Verify subscription receives all changes from X to Y
}
