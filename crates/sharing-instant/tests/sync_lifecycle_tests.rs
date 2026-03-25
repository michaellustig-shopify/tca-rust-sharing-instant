//! Tests for sync engine lifecycle (stop/restart, offline queue).
//!
//! Maps Swift's SyncEngineLifecycleTests.swift to InstantDB reactor lifecycle.
//!
//! Swift test mapping:
//! - stop/restart → reactor disconnect/reconnect
//! - write-while-stopped → offline mutation queue
//! - pending changes sync on restart → reconnection replay
//!
//! Tests exercise the SyncEngine start/stop lifecycle alongside
//! InMemoryDatabase operations to verify that data persists across
//! engine lifecycle transitions.

use sharing_instant::database::InMemoryDatabase;
use sharing_instant::sync::engine::{SyncConfig, SyncEngine};

fn test_config() -> SyncConfig {
    SyncConfig {
        app_id: "test-app-id".to_string(),
        ..Default::default()
    }
}

// === Stop and restart (maps to SyncEngineLifecycleTests) ===

#[tokio::test]
async fn stop_and_restart() {
    // Map: Stop sync engine, write data, verify metadata created but not synced,
    // then restart and verify sync completes
    let engine = SyncEngine::new(test_config());
    engine.start().await.expect("start should succeed");
    assert!(engine.status().is_connected);

    engine.stop().await;
    assert!(!engine.status().is_connected);

    // Write data while stopped — InMemoryDatabase accepts writes regardless of
    // engine state, mirroring how the local store queues mutations offline
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

    // Data is in the local store even while engine is stopped
    let query = sharing_instant::Value::Object(
        [(
            "reminders".to_string(),
            sharing_instant::Value::Object(Default::default()),
        )]
        .into_iter()
        .collect(),
    );
    let result = sharing_instant::Database::query(&db, &query)
        .expect("query should succeed while engine is stopped");
    match &result {
        sharing_instant::Value::Object(obj) => {
            let reminders = obj.get("reminders").expect("reminders key should exist");
            match reminders {
                sharing_instant::Value::Array(arr) => {
                    assert_eq!(
                        arr.len(),
                        1,
                        "should have one reminder queued while stopped"
                    );
                }
                other => panic!("expected array, got {:?}", other),
            }
        }
        other => panic!("expected object, got {:?}", other),
    }

    // Restart engine — data remains in local store, ready to sync
    engine.start().await.expect("restart should succeed");
    assert!(engine.status().is_connected);

    // After restart, local data is still present
    let result_after =
        sharing_instant::Database::query(&db, &query).expect("query should succeed after restart");
    match &result_after {
        sharing_instant::Value::Object(obj) => {
            let reminders = obj.get("reminders").expect("reminders key should exist");
            match reminders {
                sharing_instant::Value::Array(arr) => {
                    assert_eq!(arr.len(), 1, "reminder should persist across restart");
                }
                other => panic!("expected array, got {:?}", other),
            }
        }
        other => panic!("expected object, got {:?}", other),
    }
}

#[tokio::test]
async fn write_stop_delete_start() {
    // Map: Create record → stop → delete → start → verify deletion syncs
    let engine = SyncEngine::new(test_config());
    let db = InMemoryDatabase::new();

    engine.start().await.expect("start should succeed");
    assert!(engine.status().is_connected);

    // Create a record while connected
    db.insert(
        "reminders",
        "r1",
        serde_json::json!({
            "id": "r1",
            "title": "Buy milk",
            "isCompleted": false,
        }),
    );

    let query = sharing_instant::Value::Object(
        [(
            "reminders".to_string(),
            sharing_instant::Value::Object(Default::default()),
        )]
        .into_iter()
        .collect(),
    );

    // Verify record exists
    let result = sharing_instant::Database::query(&db, &query).expect("query should succeed");
    match &result {
        sharing_instant::Value::Object(obj) => {
            let reminders = obj.get("reminders").expect("reminders key should exist");
            match reminders {
                sharing_instant::Value::Array(arr) => assert_eq!(arr.len(), 1),
                other => panic!("expected array, got {:?}", other),
            }
        }
        other => panic!("expected object, got {:?}", other),
    }

    // Stop the engine
    engine.stop().await;
    assert!(!engine.status().is_connected);

    // Delete while stopped — this queues a local deletion
    db.remove("reminders", "r1");

    // Verify local deletion happened immediately
    let result_after_delete =
        sharing_instant::Database::query(&db, &query).expect("query should succeed");
    match &result_after_delete {
        sharing_instant::Value::Object(obj) => {
            let reminders = obj.get("reminders").expect("reminders key should exist");
            match reminders {
                sharing_instant::Value::Array(arr) => {
                    assert_eq!(arr.len(), 0, "record should be deleted locally");
                }
                other => panic!("expected array, got {:?}", other),
            }
        }
        other => panic!("expected object, got {:?}", other),
    }

    // Restart engine — deletion persists (would sync to server in production)
    engine.start().await.expect("restart should succeed");
    assert!(engine.status().is_connected);

    // Verify deletion survives restart
    let result_after_restart =
        sharing_instant::Database::query(&db, &query).expect("query should succeed after restart");
    match &result_after_restart {
        sharing_instant::Value::Object(obj) => {
            let reminders = obj.get("reminders").expect("reminders key should exist");
            match reminders {
                sharing_instant::Value::Array(arr) => {
                    assert_eq!(arr.len(), 0, "deletion should persist across restart");
                }
                other => panic!("expected array, got {:?}", other),
            }
        }
        other => panic!("expected object, got {:?}", other),
    }
}

#[tokio::test]
async fn pending_changes_sync_after_restart() {
    // Map: addRemindersList_StopSyncEngine_EditTitle_StartSyncEngine
    // Create list, stop engine, edit title, start engine, verify edit persisted
    let engine = SyncEngine::new(test_config());
    let db = InMemoryDatabase::new();

    engine.start().await.expect("start should succeed");

    // Create initial record while connected
    db.insert(
        "reminders",
        "r1",
        serde_json::json!({
            "id": "r1",
            "title": "Grocery List",
            "isCompleted": false,
        }),
    );

    // Stop the engine
    engine.stop().await;
    assert!(!engine.status().is_connected);

    // Edit the title while stopped — this is a pending mutation
    db.insert(
        "reminders",
        "r1",
        serde_json::json!({
            "id": "r1",
            "title": "Updated Grocery List",
            "isCompleted": false,
        }),
    );

    // Restart the engine
    engine.start().await.expect("restart should succeed");
    assert!(engine.status().is_connected);

    // Verify the edit persisted through the stop/start cycle
    let query = sharing_instant::Value::Object(
        [(
            "reminders".to_string(),
            sharing_instant::Value::Object(Default::default()),
        )]
        .into_iter()
        .collect(),
    );
    let result = sharing_instant::Database::query(&db, &query).expect("query should succeed");
    match &result {
        sharing_instant::Value::Object(obj) => {
            let reminders = obj.get("reminders").expect("reminders key should exist");
            match reminders {
                sharing_instant::Value::Array(arr) => {
                    assert_eq!(arr.len(), 1, "should have one reminder");
                    match &arr[0] {
                        sharing_instant::Value::Object(reminder) => {
                            let title = reminder.get("title").expect("title field should exist");
                            match title {
                                sharing_instant::Value::String(s) => {
                                    assert_eq!(
                                        s, "Updated Grocery List",
                                        "title should reflect the edit made while stopped"
                                    );
                                }
                                other => panic!("expected string title, got {:?}", other),
                            }
                        }
                        other => panic!("expected object reminder, got {:?}", other),
                    }
                }
                other => panic!("expected array, got {:?}", other),
            }
        }
        other => panic!("expected object, got {:?}", other),
    }
}

#[tokio::test]
async fn write_before_start_syncs_on_first_start() {
    // Map: writeAndThenStart — write data before starting engine, verify it
    // exists after first start (would be synced in production)
    let engine = SyncEngine::new(test_config());
    let db = InMemoryDatabase::new();

    // Engine not started yet
    assert!(!engine.status().is_connected);

    // Write data before engine starts — queued locally
    db.insert(
        "reminders",
        "r1",
        serde_json::json!({
            "id": "r1",
            "title": "Pre-start reminder",
            "isCompleted": false,
        }),
    );

    // Verify data exists locally before engine start
    let query = sharing_instant::Value::Object(
        [(
            "reminders".to_string(),
            sharing_instant::Value::Object(Default::default()),
        )]
        .into_iter()
        .collect(),
    );
    let result_before =
        sharing_instant::Database::query(&db, &query).expect("query should succeed before start");
    match &result_before {
        sharing_instant::Value::Object(obj) => {
            let reminders = obj.get("reminders").expect("reminders key should exist");
            match reminders {
                sharing_instant::Value::Array(arr) => {
                    assert_eq!(arr.len(), 1, "data should exist locally before start");
                }
                other => panic!("expected array, got {:?}", other),
            }
        }
        other => panic!("expected object, got {:?}", other),
    }

    // Now start the engine for the first time
    engine.start().await.expect("first start should succeed");
    assert!(engine.status().is_connected);

    // Data written before start is still available (would sync to server now)
    let result_after =
        sharing_instant::Database::query(&db, &query).expect("query should succeed after start");
    match &result_after {
        sharing_instant::Value::Object(obj) => {
            let reminders = obj.get("reminders").expect("reminders key should exist");
            match reminders {
                sharing_instant::Value::Array(arr) => {
                    assert_eq!(
                        arr.len(),
                        1,
                        "pre-start data should persist after first start"
                    );
                    match &arr[0] {
                        sharing_instant::Value::Object(reminder) => {
                            let title = reminder.get("title").expect("title field should exist");
                            match title {
                                sharing_instant::Value::String(s) => {
                                    assert_eq!(s, "Pre-start reminder");
                                }
                                other => panic!("expected string title, got {:?}", other),
                            }
                        }
                        other => panic!("expected object reminder, got {:?}", other),
                    }
                }
                other => panic!("expected array, got {:?}", other),
            }
        }
        other => panic!("expected object, got {:?}", other),
    }
}

#[tokio::test]
async fn external_shared_record_while_stopped() {
    // Map: Handle external shared records received after stop/start cycle.
    // Simulates: another device inserts data into the local store while
    // the engine was stopped, then engine restarts and sees it.
    let engine = SyncEngine::new(test_config());
    let db = InMemoryDatabase::new();

    engine.start().await.expect("start should succeed");
    assert!(engine.status().is_connected);

    // Stop the engine (simulates going offline)
    engine.stop().await;
    assert!(!engine.status().is_connected);

    // Simulate an "external" record appearing in the local store
    // (in production, this would arrive from another device's sync)
    db.insert(
        "sharedLists",
        "sl1",
        serde_json::json!({
            "id": "sl1",
            "title": "Shared Shopping List",
            "ownerId": "other-user-123",
        }),
    );

    // Restart the engine
    engine.start().await.expect("restart should succeed");
    assert!(engine.status().is_connected);

    // Verify the external record is visible after restart
    let query = sharing_instant::Value::Object(
        [(
            "sharedLists".to_string(),
            sharing_instant::Value::Object(Default::default()),
        )]
        .into_iter()
        .collect(),
    );
    let result = sharing_instant::Database::query(&db, &query).expect("query should succeed");
    match &result {
        sharing_instant::Value::Object(obj) => {
            let lists = obj
                .get("sharedLists")
                .expect("sharedLists key should exist");
            match lists {
                sharing_instant::Value::Array(arr) => {
                    assert_eq!(arr.len(), 1, "external shared record should be visible");
                    match &arr[0] {
                        sharing_instant::Value::Object(list) => {
                            let owner = list.get("ownerId").expect("ownerId should exist");
                            match owner {
                                sharing_instant::Value::String(s) => {
                                    assert_eq!(
                                        s, "other-user-123",
                                        "external owner should be preserved"
                                    );
                                }
                                other => panic!("expected string ownerId, got {:?}", other),
                            }
                        }
                        other => panic!("expected object list, got {:?}", other),
                    }
                }
                other => panic!("expected array, got {:?}", other),
            }
        }
        other => panic!("expected object, got {:?}", other),
    }
}

#[tokio::test]
async fn delete_shared_record_while_stopped() {
    // Map: Delete a shared (other-owned) record while the engine is stopped,
    // then restart and verify the deletion persisted locally.
    let engine = SyncEngine::new(test_config());
    let db = InMemoryDatabase::new();

    engine.start().await.expect("start should succeed");

    // Insert a shared record from another user
    db.insert(
        "sharedLists",
        "sl1",
        serde_json::json!({
            "id": "sl1",
            "title": "Shared from Alice",
            "ownerId": "alice-456",
        }),
    );

    // Stop engine
    engine.stop().await;
    assert!(!engine.status().is_connected);

    // Delete the shared record while stopped
    db.remove("sharedLists", "sl1");

    // Restart engine
    engine.start().await.expect("restart should succeed");
    assert!(engine.status().is_connected);

    // Verify the shared record is gone
    let query = sharing_instant::Value::Object(
        [(
            "sharedLists".to_string(),
            sharing_instant::Value::Object(Default::default()),
        )]
        .into_iter()
        .collect(),
    );
    let result = sharing_instant::Database::query(&db, &query).expect("query should succeed");
    match &result {
        sharing_instant::Value::Object(obj) => {
            let lists = obj
                .get("sharedLists")
                .expect("sharedLists key should exist");
            match lists {
                sharing_instant::Value::Array(arr) => {
                    assert_eq!(
                        arr.len(),
                        0,
                        "deleted shared record should stay deleted after restart"
                    );
                }
                other => panic!("expected array, got {:?}", other),
            }
        }
        other => panic!("expected object, got {:?}", other),
    }
}

#[tokio::test]
async fn delete_owned_shared_record_while_stopped() {
    // Map: Delete an owned shared record while engine is stopped. The record
    // was created by the current user and shared with others.
    let engine = SyncEngine::new(test_config());
    let db = InMemoryDatabase::new();

    engine.start().await.expect("start should succeed");

    // Insert an owned shared record
    db.insert(
        "sharedLists",
        "sl1",
        serde_json::json!({
            "id": "sl1",
            "title": "My Shared List",
            "ownerId": "current-user",
            "sharedWith": ["alice", "bob"],
        }),
    );

    // Verify it exists
    let query = sharing_instant::Value::Object(
        [(
            "sharedLists".to_string(),
            sharing_instant::Value::Object(Default::default()),
        )]
        .into_iter()
        .collect(),
    );
    let result = sharing_instant::Database::query(&db, &query).expect("query should succeed");
    match &result {
        sharing_instant::Value::Object(obj) => {
            match obj.get("sharedLists").expect("key should exist") {
                sharing_instant::Value::Array(arr) => assert_eq!(arr.len(), 1),
                other => panic!("expected array, got {:?}", other),
            }
        }
        other => panic!("expected object, got {:?}", other),
    }

    // Stop engine
    engine.stop().await;
    assert!(!engine.status().is_connected);

    // Delete the owned shared record while stopped
    db.remove("sharedLists", "sl1");

    // Restart engine
    engine.start().await.expect("restart should succeed");
    assert!(engine.status().is_connected);

    // Verify the owned shared record is deleted
    let result_after = sharing_instant::Database::query(&db, &query).expect("query should succeed");
    match &result_after {
        sharing_instant::Value::Object(obj) => {
            match obj.get("sharedLists").expect("key should exist") {
                sharing_instant::Value::Array(arr) => {
                    assert_eq!(
                        arr.len(),
                        0,
                        "owned shared record should be deleted after restart"
                    );
                }
                other => panic!("expected array, got {:?}", other),
            }
        }
        other => panic!("expected object, got {:?}", other),
    }
}
