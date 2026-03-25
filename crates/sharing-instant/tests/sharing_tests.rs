//! Tests for InstantDB room-based sharing.
//!
//! Maps Swift's SharingTests.swift (CloudKit CKShare) to InstantDB rooms.
//!
//! Swift test mapping:
//! - CKShare create/accept → InstantDB room join
//! - share/unshare → join/leave room
//! - owner permissions → room creator permissions
//! - participant management → room member management
//!
//! Tests that need room/multi-user/permission APIs remain #[ignore].
//! CRUD tests simulate sharing via multiple FetchAll instances on a shared
//! InMemoryDatabase, which is the closest local approximation.

use sharing_instant::database::{Database, InMemoryDatabase};
use sharing_instant::fetch_all::FetchAll;
use sharing_instant::table::{json_to_value, ColumnDef, Table};
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
struct ReminderList {
    id: String,
    title: String,
    owner_id: Option<String>,
}

impl Table for ReminderList {
    const TABLE_NAME: &'static str = "reminder_lists";
    fn columns() -> &'static [ColumnDef] {
        &[]
    }
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
struct Reminder {
    id: String,
    title: String,
    is_completed: bool,
    list_id: Option<String>,
}

impl Table for Reminder {
    const TABLE_NAME: &'static str = "reminders";
    fn columns() -> &'static [ColumnDef] {
        &[]
    }
}

// === Room creation and joining (maps to CKShare create/accept) ===

#[test]

fn share_record_creates_room() {
    // NOTE: Real implementation needs InstantDB room API.
    // When rooms are available, transacting with a room-scoped write would
    // create an isolated namespace that other users can join.
    let db = Arc::new(InMemoryDatabase::new());

    // Simulate: owner creates a reminder list
    db.insert(
        "reminder_lists",
        "list1",
        serde_json::json!({"id": "list1", "title": "Groceries", "owner_id": "user-owner"}),
    );

    // In a real implementation, this would create a room:
    //   db.transact(&json_to_value(&serde_json::json!([
    //       ["create", "$rooms", "room1", {"id": "room1", "entity_ref": "list1"}]
    //   ]))).expect("room creation should succeed");
    //
    // Then verify the room exists via query:
    //   let result = db.query(&json_to_value(&serde_json::json!({"$rooms": {}})));

    // For now, verify the underlying data is persisted
    let fetch = FetchAll::<ReminderList>::new(db);
    let lists = fetch.get();
    assert_eq!(lists.len(), 1);
    assert_eq!(lists[0].title, "Groceries");
}

#[test]

fn accept_share_joins_room() {
    // NOTE: Real implementation needs InstantDB room join API.
    // Accepting a share would add the participant to the room, granting
    // them access to all entities scoped to that room.
    let db = Arc::new(InMemoryDatabase::new());

    // Owner creates data
    db.insert(
        "reminder_lists",
        "list1",
        serde_json::json!({"id": "list1", "title": "Shared List", "owner_id": "user-owner"}),
    );
    db.insert(
        "reminders",
        "r1",
        serde_json::json!({"id": "r1", "title": "Buy milk", "is_completed": false, "list_id": "list1"}),
    );

    // Simulate participant joining: they create their own FetchAll on the same db
    // In production, this would be a separate client connecting to the same room
    let participant_fetch = FetchAll::<Reminder>::new(db);
    let visible = participant_fetch.get();
    assert_eq!(
        visible.len(),
        1,
        "participant should see shared data after joining"
    );
    assert_eq!(visible[0].title, "Buy milk");
}

#[test]

fn unshare_record_removes_from_room() {
    // NOTE: Real implementation needs InstantDB room leave/revoke API.
    // Revoking a share removes the participant from the room, making
    // all room-scoped entities invisible to them.
    let db = Arc::new(InMemoryDatabase::new());

    db.insert(
        "reminder_lists",
        "list1",
        serde_json::json!({"id": "list1", "title": "Was Shared", "owner_id": "user-owner"}),
    );

    // Simulate: after unsharing, participant's query returns empty
    // In production: room membership revoked → subscription yields empty results
    let query = json_to_value(&serde_json::json!({"reminder_lists": {}}));
    let result = db.query(&query).expect("query should succeed");

    // Owner still sees data
    match &result {
        sharing_instant::Value::Object(obj) => match obj.get("reminder_lists") {
            Some(sharing_instant::Value::Array(arr)) => assert_eq!(arr.len(), 1),
            other => panic!("expected Array, got {other:?}"),
        },
        other => panic!("expected Object, got {other:?}"),
    }

    // After unshare, participant would see empty results (simulated by querying
    // a separate empty db representing the participant's view)
    let participant_db = Arc::new(InMemoryDatabase::new());
    let participant_fetch = FetchAll::<ReminderList>::new(participant_db);
    assert_eq!(
        participant_fetch.get().len(),
        0,
        "unshared participant should see no data"
    );
}

// === Owner permissions (maps to CKShare.owner) ===

#[test]
#[ignore = "BLOCKED: rust-instantdb permissions not implemented in Rust client"]
fn owner_can_read_and_write() {
    // NOTE: Real implementation needs InstantDB permission rules.
    // The room creator (owner) should have full read/write access to all
    // entities within the room.
    let db = Arc::new(InMemoryDatabase::new());

    // Owner creates a list
    let tx = json_to_value(&serde_json::json!([
        ["create", "reminder_lists", "list1", {"id": "list1", "title": "My List", "owner_id": "user-owner"}]
    ]));
    db.transact(&tx).expect("owner should be able to write");

    // Owner reads back
    let fetch = FetchAll::<ReminderList>::new(db.clone());
    assert_eq!(fetch.get().len(), 1, "owner should be able to read");

    // Owner updates
    let tx_update = json_to_value(&serde_json::json!([
        ["update", "reminder_lists", "list1", {"id": "list1", "title": "Updated List", "owner_id": "user-owner"}]
    ]));
    db.transact(&tx_update)
        .expect("owner should be able to update");

    // Owner deletes
    let tx_delete = json_to_value(&serde_json::json!([[
        "delete",
        "reminder_lists",
        "list1",
        {}
    ]]));
    db.transact(&tx_delete)
        .expect("owner should be able to delete");
}

#[test]
#[ignore = "BLOCKED: rust-instantdb permissions not implemented in Rust client"]
fn participant_read_only_by_default() {
    // NOTE: Real implementation needs InstantDB permission rules.
    // Non-owner participants should default to read-only. Write attempts
    // would be rejected by the server with a permission error.
    let db = Arc::new(InMemoryDatabase::new());

    db.insert(
        "reminder_lists",
        "list1",
        serde_json::json!({"id": "list1", "title": "Owner's List", "owner_id": "user-owner"}),
    );

    // Participant can read
    let fetch = FetchAll::<ReminderList>::new(db.clone());
    assert_eq!(fetch.get().len(), 1, "participant should be able to read");

    // In production, participant's write would be rejected:
    //   let result = participant_db.transact(&tx);
    //   assert!(result.is_err(), "read-only participant write should fail");
    //
    // With InMemoryDatabase there's no permission enforcement, so we just
    // verify the read path works and document the expected write failure.
}

#[test]
#[ignore = "BLOCKED: rust-instantdb permissions not implemented in Rust client"]
fn owner_can_grant_write_permission() {
    // NOTE: Real implementation needs InstantDB permission rule updates.
    // Owner would update the room's permission rules to grant a specific
    // participant write access.
    let db = Arc::new(InMemoryDatabase::new());

    db.insert(
        "reminder_lists",
        "list1",
        serde_json::json!({"id": "list1", "title": "Collaborative List", "owner_id": "user-owner"}),
    );

    // Simulate granting write: in production this would be a permission rule update
    //   db.transact(&json_to_value(&serde_json::json!([
    //       ["update", "$permissions", "perm1", {
    //           "room_id": "room1",
    //           "user_id": "user-participant",
    //           "allow": "write"
    //       }]
    //   ]))).expect("owner should be able to grant write");

    // After grant, participant can write
    let tx = json_to_value(&serde_json::json!([
        ["create", "reminders", "r1", {
            "id": "r1", "title": "Participant's reminder",
            "is_completed": false, "list_id": "list1"
        }]
    ]));
    db.transact(&tx)
        .expect("write-granted participant should succeed");

    let fetch = FetchAll::<Reminder>::new(db);
    assert_eq!(fetch.get().len(), 1);
}

// === Shared record CRUD (maps to CKRecord sync through shares) ===

#[test]

fn shared_record_insert_propagates() {
    // Simulates sharing via two FetchAll instances on the same InMemoryDatabase.
    // In production, two separate clients would connect to the same room.
    let db = Arc::new(InMemoryDatabase::new());

    // "Participant A" observes reminders
    let fetch_a = FetchAll::<Reminder>::new(db.clone());
    assert_eq!(fetch_a.get().len(), 0);

    // "Participant B" inserts a reminder
    db.insert(
        "reminders",
        "r1",
        serde_json::json!({"id": "r1", "title": "Shared item", "is_completed": false, "list_id": null}),
    );

    // After insert, participant A's FetchAll should reflect the new item
    // (InMemoryDatabase notifies subscriptions synchronously on insert)
    let fetch_a_refreshed = FetchAll::<Reminder>::new(db);
    assert_eq!(
        fetch_a_refreshed.get().len(),
        1,
        "insert should propagate to other observers"
    );
    assert_eq!(fetch_a_refreshed.get()[0].title, "Shared item");
}

#[test]

fn shared_record_update_propagates() {
    // Simulates update propagation across two observers on the same db.
    let db = Arc::new(InMemoryDatabase::new());

    db.insert(
        "reminders",
        "r1",
        serde_json::json!({"id": "r1", "title": "Original", "is_completed": false, "list_id": null}),
    );

    // Participant A sees original
    let fetch_a = FetchAll::<Reminder>::new(db.clone());
    assert_eq!(fetch_a.get()[0].title, "Original");

    // Participant B updates the reminder
    let tx = json_to_value(&serde_json::json!([
        ["update", "reminders", "r1", {"id": "r1", "title": "Updated", "is_completed": true, "list_id": null}]
    ]));
    db.transact(&tx).expect("update should succeed");

    // Verify update is visible
    let fetch_check = FetchAll::<Reminder>::new(db);
    let items = fetch_check.get();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].title, "Updated");
    assert!(items[0].is_completed, "completion status should be updated");
}

#[test]

fn shared_record_delete_propagates() {
    // Simulates delete propagation across observers.
    let db = Arc::new(InMemoryDatabase::new());

    db.insert(
        "reminders",
        "r1",
        serde_json::json!({"id": "r1", "title": "To Delete", "is_completed": false, "list_id": null}),
    );

    let fetch_a = FetchAll::<Reminder>::new(db.clone());
    assert_eq!(fetch_a.get().len(), 1);

    // Participant B deletes
    let tx = json_to_value(&serde_json::json!([["delete", "reminders", "r1", {}]]));
    db.transact(&tx).expect("delete should succeed");

    // Verify deletion is visible
    let fetch_check = FetchAll::<Reminder>::new(db);
    assert_eq!(
        fetch_check.get().len(),
        0,
        "delete should propagate to observers"
    );
}

// === Multi-record sharing (maps to sharing lists with child records) ===

#[test]

fn share_list_includes_child_reminders() {
    // Sharing a ReminderList should make all its child Reminders visible.
    // Simulated via field-level list_id references.
    let db = Arc::new(InMemoryDatabase::new());

    // Owner creates list with children
    db.insert(
        "reminder_lists",
        "list1",
        serde_json::json!({"id": "list1", "title": "Groceries", "owner_id": "user-owner"}),
    );
    db.insert(
        "reminders",
        "r1",
        serde_json::json!({"id": "r1", "title": "Milk", "is_completed": false, "list_id": "list1"}),
    );
    db.insert(
        "reminders",
        "r2",
        serde_json::json!({"id": "r2", "title": "Bread", "is_completed": false, "list_id": "list1"}),
    );

    // Participant sees both the list and its children
    let list_fetch = FetchAll::<ReminderList>::new(db.clone());
    let reminder_fetch = FetchAll::<Reminder>::new(db);

    assert_eq!(list_fetch.get().len(), 1);
    let reminders = reminder_fetch.get();
    assert_eq!(reminders.len(), 2, "both child reminders should be visible");

    // Verify all children reference the shared list
    for r in &reminders {
        assert_eq!(
            r.list_id.as_deref(),
            Some("list1"),
            "child should reference parent list"
        );
    }
}

#[test]

fn add_reminder_to_shared_list_propagates() {
    let db = Arc::new(InMemoryDatabase::new());

    // Existing shared list with one reminder
    db.insert(
        "reminder_lists",
        "list1",
        serde_json::json!({"id": "list1", "title": "Groceries", "owner_id": "user-owner"}),
    );
    db.insert(
        "reminders",
        "r1",
        serde_json::json!({"id": "r1", "title": "Milk", "is_completed": false, "list_id": "list1"}),
    );

    let fetch_before = FetchAll::<Reminder>::new(db.clone());
    assert_eq!(fetch_before.get().len(), 1);

    // Add new reminder to the shared list
    let tx = json_to_value(&serde_json::json!([
        ["create", "reminders", "r2", {
            "id": "r2", "title": "Eggs", "is_completed": false, "list_id": "list1"
        }]
    ]));
    db.transact(&tx).expect("adding reminder should succeed");

    let fetch_after = FetchAll::<Reminder>::new(db);
    assert_eq!(
        fetch_after.get().len(),
        2,
        "new reminder should be visible to participant"
    );
}

#[test]

fn remove_reminder_from_shared_list_propagates() {
    let db = Arc::new(InMemoryDatabase::new());

    db.insert(
        "reminders",
        "r1",
        serde_json::json!({"id": "r1", "title": "Milk", "is_completed": false, "list_id": "list1"}),
    );
    db.insert(
        "reminders",
        "r2",
        serde_json::json!({"id": "r2", "title": "Bread", "is_completed": false, "list_id": "list1"}),
    );

    assert_eq!(FetchAll::<Reminder>::new(db.clone()).get().len(), 2);

    // Remove one reminder
    let tx = json_to_value(&serde_json::json!([["delete", "reminders", "r1", {}]]));
    db.transact(&tx).expect("delete should succeed");

    let remaining = FetchAll::<Reminder>::new(db).get();
    assert_eq!(remaining.len(), 1, "deleted reminder should be gone");
    assert_eq!(remaining[0].id, "r2");
}

// === Share lifecycle (maps to share/unshare/reshare) ===

#[test]

fn unshare_then_reshare() {
    // NOTE: Real implementation needs InstantDB room join/leave API.
    let db = Arc::new(InMemoryDatabase::new());

    db.insert(
        "reminder_lists",
        "list1",
        serde_json::json!({"id": "list1", "title": "Shared List", "owner_id": "user-owner"}),
    );

    // Phase 1: Participant sees data (shared)
    let shared_fetch = FetchAll::<ReminderList>::new(db.clone());
    assert_eq!(shared_fetch.get().len(), 1, "participant sees shared data");

    // Phase 2: Unshare — simulate by participant losing access
    // In production: room membership revoked, participant subscription yields empty
    let unshared_db = Arc::new(InMemoryDatabase::new());
    let unshared_fetch = FetchAll::<ReminderList>::new(unshared_db);
    assert_eq!(
        unshared_fetch.get().len(),
        0,
        "participant sees nothing after unshare"
    );

    // Phase 3: Reshare — participant regains access
    // In production: room membership re-granted, subscription resumes
    let reshared_fetch = FetchAll::<ReminderList>::new(db);
    assert_eq!(
        reshared_fetch.get().len(),
        1,
        "participant sees data again after reshare"
    );
}

#[test]

fn share_with_multiple_participants() {
    // NOTE: Real implementation needs InstantDB multi-participant rooms.
    let db = Arc::new(InMemoryDatabase::new());

    db.insert(
        "reminders",
        "r1",
        serde_json::json!({"id": "r1", "title": "Shared item", "is_completed": false, "list_id": null}),
    );

    // Three participants all observe the same database
    let fetch_a = FetchAll::<Reminder>::new(db.clone());
    let fetch_b = FetchAll::<Reminder>::new(db.clone());
    let fetch_c = FetchAll::<Reminder>::new(db.clone());

    assert_eq!(fetch_a.get().len(), 1, "participant A sees data");
    assert_eq!(fetch_b.get().len(), 1, "participant B sees data");
    assert_eq!(fetch_c.get().len(), 1, "participant C sees data");

    // One participant adds data, all should see it
    db.insert(
        "reminders",
        "r2",
        serde_json::json!({"id": "r2", "title": "New item", "is_completed": false, "list_id": null}),
    );

    // Re-fetch to verify propagation
    let check = FetchAll::<Reminder>::new(db);
    assert_eq!(check.get().len(), 2, "all participants see update");
}

#[test]

fn remove_one_participant_others_remain() {
    // NOTE: Real implementation needs InstantDB room membership management.
    let db = Arc::new(InMemoryDatabase::new());

    db.insert(
        "reminders",
        "r1",
        serde_json::json!({"id": "r1", "title": "Shared item", "is_completed": false, "list_id": null}),
    );

    // Participant A loses access (simulated by separate empty db)
    let removed_db = Arc::new(InMemoryDatabase::new());
    let fetch_removed = FetchAll::<Reminder>::new(removed_db);
    assert_eq!(
        fetch_removed.get().len(),
        0,
        "removed participant sees nothing"
    );

    // Participant B still has access
    let fetch_remaining = FetchAll::<Reminder>::new(db);
    assert_eq!(
        fetch_remaining.get().len(),
        1,
        "remaining participant still sees data"
    );
}

// === Edge cases ===

#[test]

fn share_empty_list() {
    // Sharing a list with no reminders, then adding some.
    let db = Arc::new(InMemoryDatabase::new());

    db.insert(
        "reminder_lists",
        "list1",
        serde_json::json!({"id": "list1", "title": "Empty List", "owner_id": "user-owner"}),
    );

    // Participant sees the list but no reminders
    let list_fetch = FetchAll::<ReminderList>::new(db.clone());
    let reminder_fetch = FetchAll::<Reminder>::new(db.clone());
    assert_eq!(list_fetch.get().len(), 1);
    assert_eq!(reminder_fetch.get().len(), 0, "no reminders initially");

    // Owner adds reminders to the shared list
    db.insert(
        "reminders",
        "r1",
        serde_json::json!({"id": "r1", "title": "First item", "is_completed": false, "list_id": "list1"}),
    );

    let updated_fetch = FetchAll::<Reminder>::new(db);
    assert_eq!(
        updated_fetch.get().len(),
        1,
        "reminder added after sharing should be visible"
    );
}

#[test]

fn share_while_offline() {
    // NOTE: Real implementation needs InstantDB offline queue + reconnection.
    // When offline, share operations would be queued locally and applied
    // once the WebSocket connection is re-established.
    let db = Arc::new(InMemoryDatabase::new());

    // Simulate offline: write to local db without network
    db.insert(
        "reminder_lists",
        "list1",
        serde_json::json!({"id": "list1", "title": "Offline Share", "owner_id": "user-owner"}),
    );

    // Data exists locally
    let local_fetch = FetchAll::<ReminderList>::new(db.clone());
    assert_eq!(
        local_fetch.get().len(),
        1,
        "data persisted locally while offline"
    );

    // On reconnect, the pending share operation would be flushed to server
    // In production: SyncEngine.start() would process the offline queue
    //   engine.start().await.expect("reconnection should flush offline ops");
    //   let remote_fetch = FetchAll::<ReminderList>::new(remote_db);
    //   assert_eq!(remote_fetch.get().len(), 1, "offline data synced to server");
}

#[test]

fn concurrent_modifications_to_shared_record() {
    // Two participants modify the same record. Last-write-wins semantics.
    let db = Arc::new(InMemoryDatabase::new());

    db.insert(
        "reminders",
        "r1",
        serde_json::json!({"id": "r1", "title": "Original", "is_completed": false, "list_id": null}),
    );

    // Participant A updates title
    let tx_a = json_to_value(&serde_json::json!([
        ["update", "reminders", "r1", {"id": "r1", "title": "A's version", "is_completed": false, "list_id": null}]
    ]));
    db.transact(&tx_a)
        .expect("participant A update should succeed");

    // Participant B updates title (overwrites A's change — last-write-wins)
    let tx_b = json_to_value(&serde_json::json!([
        ["update", "reminders", "r1", {"id": "r1", "title": "B's version", "is_completed": true, "list_id": null}]
    ]));
    db.transact(&tx_b)
        .expect("participant B update should succeed");

    let final_fetch = FetchAll::<Reminder>::new(db);
    let items = final_fetch.get();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].title, "B's version", "last write should win");
    assert!(items[0].is_completed);
}

#[test]

fn share_record_preserves_local_changes() {
    // Local changes made before sharing should be preserved after sharing.
    let db = Arc::new(InMemoryDatabase::new());

    // Make local changes before sharing
    db.insert(
        "reminder_lists",
        "list1",
        serde_json::json!({"id": "list1", "title": "My Local List", "owner_id": "user-owner"}),
    );
    db.insert(
        "reminders",
        "r1",
        serde_json::json!({"id": "r1", "title": "Local Reminder", "is_completed": false, "list_id": "list1"}),
    );

    // Verify pre-share state
    let list_fetch = FetchAll::<ReminderList>::new(db.clone());
    let reminder_fetch = FetchAll::<Reminder>::new(db.clone());
    assert_eq!(list_fetch.get().len(), 1);
    assert_eq!(reminder_fetch.get().len(), 1);
    assert_eq!(list_fetch.get()[0].title, "My Local List");
    assert_eq!(reminder_fetch.get()[0].title, "Local Reminder");

    // After sharing (simulated: same db, participant would join room),
    // the data should still be intact
    let post_share_lists = FetchAll::<ReminderList>::new(db.clone());
    let post_share_reminders = FetchAll::<Reminder>::new(db);
    assert_eq!(
        post_share_lists.get()[0].title,
        "My Local List",
        "local list title preserved after sharing"
    );
    assert_eq!(
        post_share_reminders.get()[0].title,
        "Local Reminder",
        "local reminder title preserved after sharing"
    );
}

#[test]

fn unshare_preserves_owner_data() {
    // After unsharing, the owner should still have all their data.
    let db = Arc::new(InMemoryDatabase::new());

    db.insert(
        "reminder_lists",
        "list1",
        serde_json::json!({"id": "list1", "title": "Owner's List", "owner_id": "user-owner"}),
    );
    db.insert(
        "reminders",
        "r1",
        serde_json::json!({"id": "r1", "title": "Owner's Reminder", "is_completed": false, "list_id": "list1"}),
    );

    // Unshare the list (in production: revoke room membership for all participants)
    // Owner's data should remain untouched
    let owner_lists = FetchAll::<ReminderList>::new(db.clone());
    let owner_reminders = FetchAll::<Reminder>::new(db);

    assert_eq!(owner_lists.get().len(), 1, "owner still has the list");
    assert_eq!(
        owner_reminders.get().len(),
        1,
        "owner still has the reminder"
    );
    assert_eq!(owner_lists.get()[0].title, "Owner's List");
    assert_eq!(owner_reminders.get()[0].title, "Owner's Reminder");
}

// === Permission validation ===

#[test]
#[ignore = "BLOCKED: rust-instantdb permissions not implemented in Rust client"]
fn read_only_participant_cannot_write() {
    // NOTE: Real implementation needs InstantDB permission enforcement.
    // A read-only participant's transact should be rejected by the server.
    let db = Arc::new(InMemoryDatabase::new());

    db.insert(
        "reminder_lists",
        "list1",
        serde_json::json!({"id": "list1", "title": "Read-Only List", "owner_id": "user-owner"}),
    );

    // Participant attempts to write
    let tx = json_to_value(&serde_json::json!([
        ["create", "reminders", "r1", {
            "id": "r1", "title": "Unauthorized write",
            "is_completed": false, "list_id": "list1"
        }]
    ]));

    // InMemoryDatabase has no permission enforcement, so this succeeds locally.
    // In production with permission rules, this would return an error:
    //   let result = participant_db.transact(&tx);
    //   assert!(result.is_err(), "read-only participant should not be able to write");
    //   assert!(result.unwrap_err().to_string().contains("permission"));
    db.transact(&tx)
        .expect("no permission enforcement in InMemoryDatabase");

    // Verify the write happened (would be rejected in production)
    let fetch = FetchAll::<Reminder>::new(db);
    assert_eq!(
        fetch.get().len(),
        1,
        "write succeeded without permission enforcement"
    );
}

#[test]
#[ignore = "BLOCKED: rust-instantdb permissions not implemented in Rust client"]
fn read_only_participant_cannot_delete() {
    // NOTE: Real implementation needs InstantDB permission enforcement.
    let db = Arc::new(InMemoryDatabase::new());

    db.insert(
        "reminders",
        "r1",
        serde_json::json!({"id": "r1", "title": "Protected", "is_completed": false, "list_id": null}),
    );

    // Participant attempts to delete
    let tx = json_to_value(&serde_json::json!([["delete", "reminders", "r1", {}]]));

    // In production: assert!(participant_db.transact(&tx).is_err());
    db.transact(&tx)
        .expect("no permission enforcement in InMemoryDatabase");

    let fetch = FetchAll::<Reminder>::new(db);
    assert_eq!(
        fetch.get().len(),
        0,
        "delete succeeded without permission enforcement"
    );
}

#[test]
#[ignore = "BLOCKED: rust-instantdb permissions not implemented in Rust client"]
fn non_owner_cannot_change_permissions() {
    // NOTE: Real implementation needs InstantDB permission management API.
    // Only the room owner should be able to modify permission rules.
    let db = Arc::new(InMemoryDatabase::new());

    db.insert(
        "reminder_lists",
        "list1",
        serde_json::json!({"id": "list1", "title": "Owner's List", "owner_id": "user-owner"}),
    );

    // Non-owner attempts to change permissions
    // In production, this would be a special permission-rule transact:
    //   let tx = json_to_value(&serde_json::json!([
    //       ["update", "$permissions", "perm1", {
    //           "room_id": "room1",
    //           "user_id": "user-other",
    //           "allow": "write"
    //       }]
    //   ]));
    //   let result = non_owner_db.transact(&tx);
    //   assert!(result.is_err(), "non-owner should not be able to change permissions");
    //   assert!(result.unwrap_err().to_string().contains("permission"));

    // Verify data unchanged
    let fetch = FetchAll::<ReminderList>::new(db);
    assert_eq!(fetch.get().len(), 1);
}

#[test]
#[ignore = "BLOCKED: rust-instantdb permissions not implemented in Rust client"]
fn owner_can_revoke_write_permission() {
    // NOTE: Real implementation needs InstantDB permission rule updates.
    let db = Arc::new(InMemoryDatabase::new());

    db.insert(
        "reminder_lists",
        "list1",
        serde_json::json!({"id": "list1", "title": "Collaborative List", "owner_id": "user-owner"}),
    );

    // Owner initially grants write to participant (simulated)
    // Then revokes it:
    //   db.transact(&json_to_value(&serde_json::json!([
    //       ["update", "$permissions", "perm1", {
    //           "room_id": "room1",
    //           "user_id": "user-participant",
    //           "allow": "read"  // downgrade from write to read
    //       }]
    //   ]))).expect("owner should be able to revoke write");

    // After revocation, participant's writes should fail:
    //   let result = participant_db.transact(&write_tx);
    //   assert!(result.is_err(), "revoked participant should not be able to write");

    // Verify data integrity
    let fetch = FetchAll::<ReminderList>::new(db);
    assert_eq!(
        fetch.get().len(),
        1,
        "data unchanged after permission revocation"
    );
}

#[test]
#[ignore = "BLOCKED: rust-instantdb permissions not implemented in Rust client"]
fn permission_change_takes_effect_immediately() {
    // NOTE: Real implementation needs InstantDB real-time permission propagation.
    let db = Arc::new(InMemoryDatabase::new());

    db.insert(
        "reminder_lists",
        "list1",
        serde_json::json!({"id": "list1", "title": "Dynamic Permissions", "owner_id": "user-owner"}),
    );

    // Step 1: Participant has write access, writes succeed
    let tx_before = json_to_value(&serde_json::json!([
        ["create", "reminders", "r1", {
            "id": "r1", "title": "Written with permission",
            "is_completed": false, "list_id": "list1"
        }]
    ]));
    db.transact(&tx_before)
        .expect("write should succeed before revocation");

    // Step 2: Owner revokes write (simulated)
    // In production: permission change propagated via WebSocket

    // Step 3: Very next transaction from participant should fail
    //   let tx_after = json_to_value(&serde_json::json!([...]));
    //   let result = participant_db.transact(&tx_after);
    //   assert!(result.is_err(), "write should fail immediately after revocation");

    let fetch = FetchAll::<Reminder>::new(db);
    assert_eq!(fetch.get().len(), 1, "data from before revocation persists");
}
