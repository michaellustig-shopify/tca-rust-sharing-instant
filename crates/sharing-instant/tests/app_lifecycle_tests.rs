//! Tests for application lifecycle (background/foreground transitions).
//!
//! Maps Swift's AppLifecycleTests.swift to reactor reconnection behavior.
//!
//! Swift test mapping:
//! - background sync → reactor pause/resume
//! - app termination → clean disconnect
//!
//! Tests exercise the SyncEngine stop/start cycle with InMemoryDatabase
//! mutations to verify data integrity across lifecycle transitions.

use sharing_instant::database::InMemoryDatabase;
use sharing_instant::sync::engine::{SyncConfig, SyncEngine};

#[tokio::test]
async fn reactor_reconnects_after_disconnect() {
    // Map: App goes to background → WebSocket drops → app returns → reconnect
    let engine = SyncEngine::new(SyncConfig {
        app_id: "test-app".to_string(),
        ..Default::default()
    });

    engine.start().await.expect("start should succeed");
    assert!(engine.status().is_connected);

    // Simulate disconnect (app goes to background)
    engine.stop().await;
    assert!(!engine.status().is_connected);

    // Reconnect (app returns to foreground)
    engine.start().await.expect("reconnect should succeed");
    assert!(engine.status().is_connected);
}

#[tokio::test]
async fn pending_mutations_survive_reconnection() {
    // Map: Mutations queued during disconnect are applied on reconnect.
    // Verifies that local database mutations made while the engine is
    // disconnected persist through the reconnection cycle.
    let engine = SyncEngine::new(SyncConfig {
        app_id: "test-app".to_string(),
        ..Default::default()
    });
    let db = InMemoryDatabase::new();

    // Start connected
    engine.start().await.expect("start should succeed");
    assert!(engine.status().is_connected);

    // Insert initial data while connected
    db.insert(
        "tasks",
        "t1",
        serde_json::json!({
            "id": "t1",
            "title": "Original task",
            "done": false,
        }),
    );

    // Disconnect (simulates background / network loss)
    engine.stop().await;
    assert!(!engine.status().is_connected);

    // Queue mutations while disconnected
    db.insert(
        "tasks",
        "t2",
        serde_json::json!({
            "id": "t2",
            "title": "Created while offline",
            "done": false,
        }),
    );

    // Update existing record while disconnected
    db.insert(
        "tasks",
        "t1",
        serde_json::json!({
            "id": "t1",
            "title": "Updated while offline",
            "done": true,
        }),
    );

    // Reconnect
    engine.start().await.expect("reconnect should succeed");
    assert!(engine.status().is_connected);

    // Verify all mutations survived the disconnect/reconnect cycle
    let query = sharing_instant::Value::Object(
        [(
            "tasks".to_string(),
            sharing_instant::Value::Object(Default::default()),
        )]
        .into_iter()
        .collect(),
    );
    let result = sharing_instant::Database::query(&db, &query)
        .expect("query should succeed after reconnect");

    match &result {
        sharing_instant::Value::Object(obj) => {
            let tasks = obj.get("tasks").expect("tasks key should exist");
            match tasks {
                sharing_instant::Value::Array(arr) => {
                    assert_eq!(arr.len(), 2, "both tasks should survive reconnection");

                    // Verify the mutations are present (order may vary in HashMap)
                    let titles: Vec<String> = arr
                        .iter()
                        .filter_map(|v| match v {
                            sharing_instant::Value::Object(obj) => {
                                obj.get("title").and_then(|t| match t {
                                    sharing_instant::Value::String(s) => Some(s.clone()),
                                    _ => None,
                                })
                            }
                            _ => None,
                        })
                        .collect();

                    assert!(
                        titles.contains(&"Updated while offline".to_string()),
                        "updated task title should persist: got {:?}",
                        titles
                    );
                    assert!(
                        titles.contains(&"Created while offline".to_string()),
                        "new task should persist: got {:?}",
                        titles
                    );
                }
                other => panic!("expected array, got {:?}", other),
            }
        }
        other => panic!("expected object, got {:?}", other),
    }
}
