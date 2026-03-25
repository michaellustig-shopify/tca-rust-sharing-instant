//! Integration tests for live InstantDB sync.
//!
//! Each test creates a real ephemeral InstantDB app. No mocks.
//!
//! KEY INSIGHT: The reactor only sends AddQuery for subscriptions
//! that exist when the connection is established. So we must
//! subscribe BEFORE calling start().

use instant_admin::ephemeral::create_ephemeral_app;
use instant_client::connection::ConnectionConfig;
use instant_client::reactor::Reactor;
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;

fn uuid() -> String {
    uuid::Uuid::new_v4().to_string()
}

/// Create an app + admin client. Reactor is NOT started yet.
async fn setup() -> (instant_admin::AdminClient, Arc<Reactor>, String) {
    let app = create_ephemeral_app("sharing-instant-test")
        .await
        .expect("failed to create ephemeral app");
    let admin = instant_admin::AdminClient::new(&app.id, &app.admin_token);
    let reactor = Arc::new(Reactor::new(ConnectionConfig::admin(
        &app.id,
        &app.admin_token,
    )));
    (admin, reactor, app.id)
}

/// Wait for the watch receiver to match a predicate.
async fn wait_for<F>(
    rx: &mut tokio::sync::watch::Receiver<Option<serde_json::Value>>,
    timeout: Duration,
    pred: F,
) -> serde_json::Value
where
    F: Fn(&serde_json::Value) -> bool,
{
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        {
            let val = rx.borrow_and_update().clone();
            if let Some(v) = &val {
                if pred(v) {
                    return v.clone();
                }
            }
        }
        match tokio::time::timeout_at(deadline, rx.changed()).await {
            Ok(Ok(())) => continue,
            Ok(Err(_)) => panic!("channel closed"),
            Err(_) => {
                let val = rx.borrow().clone();
                panic!(
                    "timed out. last value: {}",
                    serde_json::to_string_pretty(&val).unwrap_or_default()
                );
            }
        }
    }
}

fn todo_count(v: &serde_json::Value) -> usize {
    v.get("todos")
        .and_then(|t| t.as_array())
        .map(|a| a.len())
        .unwrap_or(0)
}

// ═══════════════════════════════════════════════════════════════
// Test 1: Admin HTTP write + read (no WebSocket needed)
// ═══════════════════════════════════════════════════════════════

#[tokio::test]
async fn admin_write_then_read() {
    let (admin, _reactor, _) = setup().await;

    let id = uuid();
    admin
        .transact(&json!([
            ["update", "items", &id, {"name": "test-item", "count": 42}]
        ]))
        .await
        .expect("transact failed");

    let data = admin
        .query(&json!({"items": {}}))
        .await
        .expect("query failed");

    let items = data["items"].as_array().expect("items should be array");
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["name"], "test-item");
    assert_eq!(items[0]["count"], 42);
}

// ═══════════════════════════════════════════════════════════════
// Test 2: Subscription receives initial data
// ═══════════════════════════════════════════════════════════════

#[tokio::test]
async fn subscription_receives_initial_data() {
    let (admin, reactor, _) = setup().await;

    // Seed BEFORE subscribing
    admin
        .transact(&json!([
            ["update", "todos", uuid(), {"text": "first", "done": false, "ts": 1}],
            ["update", "todos", uuid(), {"text": "second", "done": true, "ts": 2}],
        ]))
        .await
        .expect("seed failed");

    // Subscribe, then start → AddQuery sent on connect
    let mut rx = reactor.subscribe(json!({"todos": {}})).await;
    reactor.start().await.expect("start failed");

    // Give the reactor time to connect and send AddQuery
    tokio::time::sleep(Duration::from_millis(500)).await;

    let data = wait_for(&mut rx, Duration::from_secs(10), |v| todo_count(v) == 2).await;
    assert_eq!(todo_count(&data), 2);

    reactor.stop().await;
}

// ═══════════════════════════════════════════════════════════════
// Test 3: Admin write propagates to existing subscription
// ═══════════════════════════════════════════════════════════════

#[tokio::test]
async fn write_propagates_to_subscription() {
    let (admin, reactor, _) = setup().await;

    let mut rx = reactor.subscribe(json!({"todos": {}})).await;
    reactor.start().await.expect("start failed");

    // Wait for initial (empty)
    wait_for(&mut rx, Duration::from_secs(10), |v| {
        v.get("todos").is_some()
    })
    .await;

    // Write via admin HTTP (different channel than WebSocket)
    admin
        .transact(&json!([
            ["update", "todos", uuid(), {"text": "from admin", "done": false, "ts": 1}]
        ]))
        .await
        .expect("transact failed");

    let data = wait_for(&mut rx, Duration::from_secs(10), |v| todo_count(v) == 1).await;
    assert_eq!(todo_count(&data), 1);

    reactor.stop().await;
}

// ═══════════════════════════════════════════════════════════════
// Test 4: WebSocket transact propagates to own subscription
// ═══════════════════════════════════════════════════════════════

#[tokio::test]
async fn websocket_transact_propagates() {
    let (_admin, reactor, _) = setup().await;

    let mut rx = reactor.subscribe(json!({"todos": {}})).await;
    reactor.start().await.expect("start failed");

    // Wait for initial
    wait_for(&mut rx, Duration::from_secs(10), |v| {
        v.get("todos").is_some()
    })
    .await;

    // Transact via WebSocket
    reactor
        .transact(json!([
            ["update", "todos", uuid(), {"text": "via ws", "done": false, "ts": 1}]
        ]))
        .await
        .expect("ws transact failed");

    let data = wait_for(&mut rx, Duration::from_secs(10), |v| todo_count(v) == 1).await;
    assert_eq!(todo_count(&data), 1);

    reactor.stop().await;
}

// ═══════════════════════════════════════════════════════════════
// Test 5: Multiple writes accumulate
// ═══════════════════════════════════════════════════════════════

#[tokio::test]
async fn multiple_writes_accumulate() {
    let (admin, reactor, _) = setup().await;

    let mut rx = reactor.subscribe(json!({"todos": {}})).await;
    reactor.start().await.expect("start failed");

    wait_for(&mut rx, Duration::from_secs(10), |v| {
        v.get("todos").is_some()
    })
    .await;

    for i in 1..=3 {
        admin
            .transact(&json!([
                ["update", "todos", uuid(), {"text": format!("item-{i}"), "done": false, "ts": i}]
            ]))
            .await
            .expect("transact failed");
    }

    let data = wait_for(&mut rx, Duration::from_secs(15), |v| todo_count(v) >= 3).await;
    assert_eq!(todo_count(&data), 3);

    reactor.stop().await;
}

// ═══════════════════════════════════════════════════════════════
// Test 6: Delete propagates
// ═══════════════════════════════════════════════════════════════

#[tokio::test]
async fn delete_propagates() {
    let (admin, reactor, _) = setup().await;

    let todo_id = uuid();
    admin
        .transact(&json!([
            ["update", "todos", &todo_id, {"text": "to-delete", "done": false, "ts": 1}]
        ]))
        .await
        .expect("seed failed");

    let mut rx = reactor.subscribe(json!({"todos": {}})).await;
    reactor.start().await.expect("start failed");

    wait_for(&mut rx, Duration::from_secs(10), |v| todo_count(v) == 1).await;

    admin
        .transact(&json!([["delete", "todos", &todo_id, {}]]))
        .await
        .expect("delete failed");

    let data = wait_for(&mut rx, Duration::from_secs(10), |v| todo_count(v) == 0).await;
    assert_eq!(todo_count(&data), 0);

    reactor.stop().await;
}

// ═══════════════════════════════════════════════════════════════
// Test 7: Update field propagates
// ═══════════════════════════════════════════════════════════════

#[tokio::test]
async fn update_field_propagates() {
    let (admin, reactor, _) = setup().await;

    let todo_id = uuid();
    admin
        .transact(&json!([
            ["update", "todos", &todo_id, {"text": "toggle-me", "done": false, "ts": 1}]
        ]))
        .await
        .expect("seed failed");

    let mut rx = reactor.subscribe(json!({"todos": {}})).await;
    reactor.start().await.expect("start failed");

    wait_for(&mut rx, Duration::from_secs(10), |v| todo_count(v) == 1).await;

    admin
        .transact(&json!([["update", "todos", &todo_id, {"done": true}]]))
        .await
        .expect("update failed");

    let data = wait_for(&mut rx, Duration::from_secs(10), |v| {
        v["todos"]
            .as_array()
            .and_then(|a| a.first())
            .and_then(|t| t["done"].as_bool())
            == Some(true)
    })
    .await;

    assert!(data["todos"][0]["done"].as_bool().expect("should have done"));

    reactor.stop().await;
}

// ═══════════════════════════════════════════════════════════════
// Test 8: Multi-entity query
// ═══════════════════════════════════════════════════════════════

#[tokio::test]
async fn multi_entity_query() {
    let (admin, reactor, _) = setup().await;

    admin
        .transact(&json!([
            ["update", "projects", uuid(), {"name": "Personal", "color": "blue"}],
            ["update", "todos", uuid(), {"text": "task-1", "done": false, "ts": 1}],
            ["update", "todos", uuid(), {"text": "task-2", "done": false, "ts": 2}],
        ]))
        .await
        .expect("seed failed");

    let mut rx = reactor
        .subscribe(json!({"projects": {}, "todos": {}}))
        .await;
    reactor.start().await.expect("start failed");

    let data = wait_for(&mut rx, Duration::from_secs(10), |v| {
        let p = v["projects"].as_array().map(|a| a.len()).unwrap_or(0);
        let t = todo_count(v);
        p >= 1 && t >= 2
    })
    .await;

    assert_eq!(data["projects"].as_array().map(|a| a.len()).unwrap_or(0), 1);
    assert_eq!(todo_count(&data), 2);

    reactor.stop().await;
}

// ═══════════════════════════════════════════════════════════════
// Test 9: Two reactors see each other's writes
// ═══════════════════════════════════════════════════════════════

#[tokio::test]
async fn two_clients_sync() {
    let app = create_ephemeral_app("two-client-test")
        .await
        .expect("failed to create app");

    let r1 = Arc::new(Reactor::new(ConnectionConfig::admin(
        &app.id,
        &app.admin_token,
    )));
    let r2 = Arc::new(Reactor::new(ConnectionConfig::admin(
        &app.id,
        &app.admin_token,
    )));

    // R2 subscribes and starts
    let mut rx2 = r2.subscribe(json!({"todos": {}})).await;
    r2.start().await.expect("r2 start failed");

    // R1 starts (no subscription needed, just transacting)
    r1.start().await.expect("r1 start failed");

    // Wait for R2's initial
    wait_for(&mut rx2, Duration::from_secs(10), |v| {
        v.get("todos").is_some()
    })
    .await;

    // R1 writes
    r1.transact(json!([
        ["update", "todos", uuid(), {"text": "from client1", "done": false, "ts": 1}]
    ]))
    .await
    .expect("r1 transact failed");

    // R2 should see it
    let data = wait_for(&mut rx2, Duration::from_secs(10), |v| todo_count(v) == 1).await;
    assert_eq!(data["todos"][0]["text"].as_str().expect("text"), "from client1");

    r1.stop().await;
    r2.stop().await;
}
