//! Tests for conflict resolution.
//!
//! Maps Swift's MergeConflictTests.swift to InstantDB's last-write-wins semantics.
//!
//! Swift test mapping:
//! - client-server merge conflicts → InstantDB last-write-wins + server-side resolution
//! - field-level conflicts → attribute-level EAV updates
//! - delete vs update conflicts → delete wins in InstantDB
//!
//! Tests exercise InMemoryDatabase's conflict behavior directly. The
//! `server_rejects_stale_update` test remains ignored because
//! InMemoryDatabase has no staleness / processed-tx-id concept.

use sharing_instant::database::{Database, InMemoryDatabase};
use sharing_instant::table::{json_to_value, ColumnDef, Table};
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
struct Note {
    id: String,
    title: String,
    body: String,
    version: i64,
}

impl Table for Note {
    const TABLE_NAME: &'static str = "notes";
    fn columns() -> &'static [ColumnDef] {
        &[]
    }
}

// === Last-write-wins conflicts ===

#[test]
fn concurrent_update_last_write_wins() {
    // Two sequential updates to the same record — the last one wins.
    let db = Arc::new(InMemoryDatabase::new());

    db.insert(
        "notes",
        "n1",
        serde_json::json!({"id": "n1", "title": "Original", "body": "First draft", "version": 1}),
    );

    // "Client A" update
    let tx_a = json_to_value(&serde_json::json!([
        ["update", "notes", "n1", {"id": "n1", "title": "Client A title", "body": "A body", "version": 2}]
    ]));
    db.transact(&tx_a)
        .expect("client A transact should succeed");

    // "Client B" update — arrives after A, so it wins
    let tx_b = json_to_value(&serde_json::json!([
        ["update", "notes", "n1", {"id": "n1", "title": "Client B title", "body": "B body", "version": 3}]
    ]));
    db.transact(&tx_b)
        .expect("client B transact should succeed");

    // Query and verify the last write won
    let query = json_to_value(&serde_json::json!({"notes": {}}));
    let result = db.query(&query).expect("query should succeed");

    let note = parse_first_note(&result);
    assert_eq!(
        note.title, "Client B title",
        "last write should win for title"
    );
    assert_eq!(note.body, "B body", "last write should win for body");
    assert_eq!(note.version, 3, "last write should win for version");
}

#[test]
fn concurrent_update_different_fields_both_apply() {
    // Client A updates title, Client B updates body. In a real EAV store
    // these are independent attributes and both apply without conflict.
    // With InMemoryDatabase (whole-entity replacement), we simulate the
    // merge by reading current state before each partial update.
    let db = Arc::new(InMemoryDatabase::new());

    db.insert(
        "notes",
        "n1",
        serde_json::json!({"id": "n1", "title": "Original", "body": "Original body", "version": 1}),
    );

    // Client A: update only title (reads current body, preserves it)
    let tx_a = json_to_value(&serde_json::json!([
        ["merge", "notes", "n1", {"id": "n1", "title": "New Title", "body": "Original body", "version": 1}]
    ]));
    db.transact(&tx_a).expect("client A merge should succeed");

    // Client B: update only body (reads current title after A's write)
    let tx_b = json_to_value(&serde_json::json!([
        ["merge", "notes", "n1", {"id": "n1", "title": "New Title", "body": "New Body", "version": 1}]
    ]));
    db.transact(&tx_b).expect("client B merge should succeed");

    let query = json_to_value(&serde_json::json!({"notes": {}}));
    let result = db.query(&query).expect("query should succeed");

    let note = parse_first_note(&result);
    assert_eq!(
        note.title, "New Title",
        "title from client A should persist"
    );
    assert_eq!(note.body, "New Body", "body from client B should persist");
}

#[test]
fn delete_vs_update_conflict() {
    // Client A deletes record, then Client B tries to update it.
    // After delete, the entity is gone; the update re-creates it.
    // Verify both orderings.
    let db = Arc::new(InMemoryDatabase::new());

    // Setup
    db.insert(
        "notes",
        "n1",
        serde_json::json!({"id": "n1", "title": "Will be deleted", "body": "Body", "version": 1}),
    );

    // Client A deletes
    let tx_del = json_to_value(&serde_json::json!([["delete", "notes", "n1", {}]]));
    db.transact(&tx_del)
        .expect("delete transact should succeed");

    // Verify entity is gone
    let query = json_to_value(&serde_json::json!({"notes": {}}));
    let result = db.query(&query).expect("query should succeed");
    let count = count_notes(&result);
    assert_eq!(count, 0, "entity should be deleted");

    // Client B attempts to update the now-deleted entity.
    // In InMemoryDatabase, "update" calls insert, so it re-creates.
    let tx_update = json_to_value(&serde_json::json!([
        ["update", "notes", "n1", {"id": "n1", "title": "Resurrected", "body": "Back", "version": 2}]
    ]));
    db.transact(&tx_update)
        .expect("update after delete should succeed");

    let result2 = db.query(&query).expect("second query should succeed");
    let count2 = count_notes(&result2);
    assert_eq!(count2, 1, "update after delete re-creates the entity");

    let note = parse_first_note(&result2);
    assert_eq!(note.title, "Resurrected");

    // Now test the reverse: update first, then delete wins
    let tx_del2 = json_to_value(&serde_json::json!([["delete", "notes", "n1", {}]]));
    db.transact(&tx_del2).expect("final delete should succeed");

    let result3 = db.query(&query).expect("third query should succeed");
    assert_eq!(
        count_notes(&result3),
        0,
        "delete after update should leave entity gone"
    );
}

#[test]
fn create_vs_create_conflict() {
    // Two clients create records of the same entity type. Since they
    // use different IDs, both succeed — no conflict.
    let db = Arc::new(InMemoryDatabase::new());

    let tx_a = json_to_value(&serde_json::json!([
        ["create", "notes", "n1", {"id": "n1", "title": "Note A", "body": "A", "version": 1}]
    ]));
    db.transact(&tx_a).expect("client A create should succeed");

    let tx_b = json_to_value(&serde_json::json!([
        ["create", "notes", "n2", {"id": "n2", "title": "Note B", "body": "B", "version": 1}]
    ]));
    db.transact(&tx_b).expect("client B create should succeed");

    let query = json_to_value(&serde_json::json!({"notes": {}}));
    let result = db.query(&query).expect("query should succeed");

    let count = count_notes(&result);
    assert_eq!(count, 2, "both creates should succeed with different IDs");
}

#[test]
fn offline_update_applied_on_reconnect() {
    // Simulate an offline mutation by building the transaction steps
    // first, then applying them later (as if reconnected).
    let db = Arc::new(InMemoryDatabase::new());

    db.insert(
        "notes",
        "n1",
        serde_json::json!({"id": "n1", "title": "Before offline", "body": "Body", "version": 1}),
    );

    // Build the queued update (would have been created while offline)
    let queued_tx = json_to_value(&serde_json::json!([
        ["update", "notes", "n1", {"id": "n1", "title": "Updated while offline", "body": "Body", "version": 2}]
    ]));

    // "Reconnect" — apply the queued transaction
    db.transact(&queued_tx)
        .expect("applying queued update should succeed");

    let query = json_to_value(&serde_json::json!({"notes": {}}));
    let result = db.query(&query).expect("query should succeed");

    let note = parse_first_note(&result);
    assert_eq!(
        note.title, "Updated while offline",
        "offline update should apply on reconnect"
    );
    assert_eq!(note.version, 2);
}

#[test]
fn offline_delete_applied_on_reconnect() {
    // Simulate an offline delete that gets applied when reconnected.
    let db = Arc::new(InMemoryDatabase::new());

    db.insert(
        "notes",
        "n1",
        serde_json::json!({"id": "n1", "title": "Will be deleted offline", "body": "Body", "version": 1}),
    );

    // Verify entity exists
    let query = json_to_value(&serde_json::json!({"notes": {}}));
    let result = db.query(&query).expect("query should succeed");
    assert_eq!(
        count_notes(&result),
        1,
        "entity should exist before offline delete"
    );

    // Build the queued delete (created while offline)
    let queued_tx = json_to_value(&serde_json::json!([["delete", "notes", "n1", {}]]));

    // "Reconnect" — apply the queued delete
    db.transact(&queued_tx)
        .expect("applying queued delete should succeed");

    let result2 = db
        .query(&query)
        .expect("query after reconnect should succeed");
    assert_eq!(
        count_notes(&result2),
        0,
        "offline delete should apply on reconnect"
    );
}

#[test]
#[ignore = "InMemoryDatabase has no staleness / processed-tx-id concept"]
fn server_rejects_stale_update() {
    // A real InstantDB server may reject updates based on
    // processed-tx-id to prevent stale writes. InMemoryDatabase
    // has no versioned transaction log, so this cannot be tested here.
    let _db = Arc::new(InMemoryDatabase::new());
    todo!("Stale update handling requires server-side tx versioning")
}

// === Helpers ===

fn parse_first_note(result: &sharing_instant::Value) -> Note {
    match result {
        sharing_instant::Value::Object(obj) => match obj.get("notes") {
            Some(sharing_instant::Value::Array(arr)) => {
                assert!(!arr.is_empty(), "expected at least one note in results");
                Note::from_value(&arr[0]).expect("should deserialize Note")
            }
            other => panic!("expected notes Array, got: {other:?}"),
        },
        other => panic!("expected Object result, got: {other:?}"),
    }
}

fn count_notes(result: &sharing_instant::Value) -> usize {
    match result {
        sharing_instant::Value::Object(obj) => match obj.get("notes") {
            Some(sharing_instant::Value::Array(arr)) => arr.len(),
            _ => 0,
        },
        _ => 0,
    }
}
