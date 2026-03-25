//! Integration tests connecting to a real InstantDB server.
//!
//! These tests require a real InstantDB app. Set environment variables:
//!   INSTANT_APP_ID=<app-id>
//!   INSTANT_ADMIN_TOKEN=<admin-token>
//!
//! Run: cargo test -p sharing-instant --test integration_tests --features integration
//!
//! All reads AND writes go through LiveDatabase → Reactor → WebSocket.
//! This proves the full round-trip: LiveDatabase.transact() → instaml transform
//! → Reactor → server → Reactor → LiveDatabase.query()/subscribe().

#![cfg(feature = "integration")]

use instant_client::{ConnectionConfig, Reactor};
use sharing_instant::database::{Database, LiveDatabase};
use sharing_instant::sync::engine::{SyncConfig, SyncEngine};
use sharing_instant::table::{json_to_value, value_to_json};
use std::sync::Arc;

fn get_config() -> (String, String) {
    let app_id =
        std::env::var("INSTANT_APP_ID").expect("INSTANT_APP_ID must be set for integration tests");
    let admin_token = std::env::var("INSTANT_ADMIN_TOKEN")
        .expect("INSTANT_ADMIN_TOKEN must be set for integration tests");
    (app_id, admin_token)
}

fn unique_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

async fn make_live_db() -> (Arc<Reactor>, LiveDatabase) {
    let (app_id, admin_token) = get_config();
    let config = ConnectionConfig::admin(&app_id, &admin_token);
    let reactor = Arc::new(Reactor::new(config));
    reactor.start().await.expect("reactor should start");

    // Give the reactor time to complete WebSocket handshake + receive InitOk.
    tokio::time::sleep(std::time::Duration::from_millis(1000)).await;

    let handle = tokio::runtime::Handle::current();
    let db = LiveDatabase::new(reactor.clone(), handle);
    (reactor, db)
}

// === CRUD through LiveDatabase ===

#[tokio::test(flavor = "multi_thread")]
async fn transact_and_query_round_trip() {
    let (_reactor, db) = make_live_db().await;
    let id = unique_id();

    // Write through LiveDatabase (uses transact_chunks → instaml transform)
    let tx = json_to_value(&serde_json::json!([
        ["update", "integration_tests", &id, {
            "title": "round trip test",
            "count": 42
        }]
    ]));
    db.transact(&tx).expect("transact should succeed");

    // Give server time to process + reactor to receive the update.
    tokio::time::sleep(std::time::Duration::from_millis(1500)).await;

    // Query through LiveDatabase
    let query = json_to_value(&serde_json::json!({"integration_tests": {}}));
    let result = db.query(&query).expect("query should succeed");
    let json_result = value_to_json(&result);

    let items = json_result
        .get("integration_tests")
        .and_then(|v| v.as_array())
        .expect("should have integration_tests array");

    let found = items
        .iter()
        .any(|item| item.get("id").and_then(|v| v.as_str()) == Some(&id));
    assert!(
        found,
        "item written via LiveDatabase should be queryable. Got: {:?}",
        items
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn transact_update_existing_record() {
    let (_reactor, db) = make_live_db().await;
    let id = unique_id();

    // Create
    let tx1 = json_to_value(&serde_json::json!([
        ["update", "integration_tests", &id, {"title": "original"}]
    ]));
    db.transact(&tx1).expect("create should succeed");
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    // Update
    let tx2 = json_to_value(&serde_json::json!([
        ["update", "integration_tests", &id, {"title": "updated"}]
    ]));
    db.transact(&tx2).expect("update should succeed");
    tokio::time::sleep(std::time::Duration::from_millis(1500)).await;

    // Query
    let query = json_to_value(&serde_json::json!({"integration_tests": {}}));
    let result = db.query(&query).expect("query should succeed");
    let json_result = value_to_json(&result);

    let items = json_result
        .get("integration_tests")
        .and_then(|v| v.as_array())
        .expect("should have array");

    let item = items
        .iter()
        .find(|i| i.get("id").and_then(|v| v.as_str()) == Some(&id))
        .expect("should find the updated item");

    assert_eq!(
        item.get("title").and_then(|v| v.as_str()),
        Some("updated"),
        "title should reflect the update"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn transact_delete_record() {
    let (_reactor, db) = make_live_db().await;
    let id = unique_id();

    // Create
    let tx1 = json_to_value(&serde_json::json!([
        ["update", "integration_tests", &id, {"title": "to-delete"}]
    ]));
    db.transact(&tx1).expect("create should succeed");
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    // Delete
    let tx2 = json_to_value(&serde_json::json!([[
        "delete",
        "integration_tests",
        &id,
        {}
    ]]));
    db.transact(&tx2).expect("delete should succeed");
    tokio::time::sleep(std::time::Duration::from_millis(1500)).await;

    // Query — item should be gone
    let query = json_to_value(&serde_json::json!({"integration_tests": {}}));
    let result = db.query(&query).expect("query should succeed");
    let json_result = value_to_json(&result);

    let items = json_result
        .get("integration_tests")
        .and_then(|v| v.as_array())
        .expect("should have array");

    let found = items
        .iter()
        .any(|item| item.get("id").and_then(|v| v.as_str()) == Some(&id));
    assert!(!found, "deleted item should not appear in query results");
}

// === Subscription bridge ===

#[tokio::test(flavor = "multi_thread")]
async fn subscribe_receives_updates_from_transact() {
    let (_reactor, db) = make_live_db().await;
    let id = unique_id();

    // Subscribe through LiveDatabase
    let query = json_to_value(&serde_json::json!({"integration_tests": {}}));
    let mut rx = db.subscribe(&query).expect("subscribe should succeed");

    // Wait for initial result
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    // Write through LiveDatabase
    let tx = json_to_value(&serde_json::json!([
        ["update", "integration_tests", &id, {"title": "subscription test"}]
    ]));
    db.transact(&tx).expect("transact should succeed");

    // Wait for the subscription to fire with the new data
    let result = tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            rx.changed().await.expect("subscription should not close");
            let val = rx.borrow().clone();
            if let Some(data) = val {
                let json = value_to_json(&data);
                if let Some(items) = json.get("integration_tests").and_then(|v| v.as_array()) {
                    if items
                        .iter()
                        .any(|i| i.get("id").and_then(|v| v.as_str()) == Some(&id))
                    {
                        return;
                    }
                }
            }
        }
    })
    .await;

    assert!(
        result.is_ok(),
        "subscription should receive the transacted item within 5s"
    );
}

// === SyncEngine lifecycle ===

#[tokio::test(flavor = "multi_thread")]
async fn sync_engine_connects_to_real_server() {
    let (app_id, admin_token) = get_config();
    let engine = SyncEngine::new(SyncConfig {
        app_id,
        admin_token: Some(admin_token),
        ..Default::default()
    });

    engine
        .start()
        .await
        .expect("start should succeed with real server");
    assert!(engine.status().is_connected);

    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    engine.stop().await;
    assert!(!engine.status().is_connected);
}

#[tokio::test(flavor = "multi_thread")]
async fn sync_engine_stop_start_cycle() {
    let (app_id, admin_token) = get_config();
    let engine = SyncEngine::new(SyncConfig {
        app_id,
        admin_token: Some(admin_token),
        ..Default::default()
    });

    engine.start().await.expect("first start");
    assert!(engine.status().is_connected);
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;

    engine.stop().await;
    assert!(!engine.status().is_connected);

    engine.start().await.expect("second start");
    assert!(engine.status().is_connected);

    engine.stop().await;
}

// === Cross-client sync ===

#[tokio::test(flavor = "multi_thread")]
async fn two_clients_see_each_others_writes() {
    let (app_id, admin_token) = get_config();
    let id = unique_id();

    // Client 1
    let config1 = ConnectionConfig::admin(&app_id, &admin_token);
    let reactor1 = Arc::new(Reactor::new(config1));
    reactor1.start().await.expect("reactor1 start");
    tokio::time::sleep(std::time::Duration::from_millis(1000)).await;
    let db1 = LiveDatabase::new(reactor1.clone(), tokio::runtime::Handle::current());

    // Client 2
    let config2 = ConnectionConfig::admin(&app_id, &admin_token);
    let reactor2 = Arc::new(Reactor::new(config2));
    reactor2.start().await.expect("reactor2 start");
    tokio::time::sleep(std::time::Duration::from_millis(1000)).await;
    let db2 = LiveDatabase::new(reactor2.clone(), tokio::runtime::Handle::current());

    // Client 2 subscribes
    let query = json_to_value(&serde_json::json!({"integration_tests": {}}));
    let mut rx2 = db2.subscribe(&query).expect("client2 subscribe");

    // Client 1 writes through LiveDatabase
    let tx = json_to_value(&serde_json::json!([
        ["update", "integration_tests", &id, {"title": "from client 1"}]
    ]));
    db1.transact(&tx).expect("client1 transact");

    // Client 2 should see the write via subscription
    let result = tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            rx2.changed().await.expect("rx2 should not close");
            let val = rx2.borrow().clone();
            if let Some(data) = val {
                let json = value_to_json(&data);
                if let Some(items) = json.get("integration_tests").and_then(|v| v.as_array()) {
                    if items
                        .iter()
                        .any(|i| i.get("id").and_then(|v| v.as_str()) == Some(&id))
                    {
                        return;
                    }
                }
            }
        }
    })
    .await;

    assert!(
        result.is_ok(),
        "client 2 should see client 1's write within 5s"
    );

    // Client 1 should also see it via query
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    let result1 = db1.query(&query).expect("client1 query");
    let json1 = value_to_json(&result1);
    let items1 = json1
        .get("integration_tests")
        .and_then(|v| v.as_array())
        .expect("should have array");
    let found1 = items1
        .iter()
        .any(|i| i.get("id").and_then(|v| v.as_str()) == Some(&id));
    assert!(found1, "client 1 should also see its own write");

    reactor1.stop().await;
    reactor2.stop().await;
}
