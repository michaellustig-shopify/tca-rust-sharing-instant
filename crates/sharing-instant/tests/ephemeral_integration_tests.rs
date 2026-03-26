//! Integration tests using ephemeral InstantDB apps.
//!
//! Each test creates its own 14-day TTL app via the unauthenticated
//! `/dash/apps/ephemeral` endpoint — no environment variables needed.
//!
//! Run: cargo test -p sharing-instant --test ephemeral_integration_tests --features integration
//!
//! These tests hit the real InstantDB server and exercise:
//! - Room join/set_presence/set_presence_with_callbacks/leave
//! - TopicChannel subscribe/publish/PublishHandle
//! - InstantDB init/use_query/tx/auth_state/watch_auth_state
//! - AuthCoordinator sign_in_as_guest (success path)

#![cfg(feature = "integration")]

use instant_admin::ephemeral::{create_ephemeral_app, EphemeralApp};
use instant_client::{ConnectionConfig, Reactor};
use sharing_instant::rooms::Room;
use sharing_instant::sync::engine::SyncConfig;
use sharing_instant::table::{ColumnDef, Table};
use sharing_instant::topics::TopicChannel;
use sharing_instant::{AuthState, InstantDB, MutationCallbacks};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

// --- Test Table ---

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
struct Todo {
    id: String,
    title: String,
    done: bool,
}

impl Table for Todo {
    const TABLE_NAME: &'static str = "todos";
    fn columns() -> &'static [ColumnDef] {
        &[]
    }
}

// --- Helpers ---

async fn make_app(label: &str) -> EphemeralApp {
    create_ephemeral_app(label)
        .await
        .expect("failed to create ephemeral app")
}

async fn ephemeral_reactor(label: &str) -> (String, String, Arc<Reactor>, tokio::runtime::Handle) {
    let app = make_app(label).await;
    let config = ConnectionConfig::admin(&app.id, &app.admin_token);
    let reactor = Arc::new(Reactor::new(config));
    reactor.start().await.expect("reactor should start");
    // Wait for WebSocket handshake + InitOk
    tokio::time::sleep(Duration::from_millis(1500)).await;
    (
        app.id,
        app.admin_token,
        reactor,
        tokio::runtime::Handle::current(),
    )
}

// ============================================================
// Room tests
// ============================================================

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
struct Cursor {
    x: f64,
    y: f64,
}

#[tokio::test(flavor = "multi_thread")]
async fn room_join_and_set_presence() {
    let (_app_id, _token, reactor, handle) = ephemeral_reactor("room-join").await;

    let room =
        Room::<Cursor>::join(reactor.clone(), handle, "editor", "doc-1").expect("should join room");

    // Wait for join to complete
    tokio::time::sleep(Duration::from_millis(500)).await;
    assert!(room.is_joined(), "room should be joined after delay");

    // Set presence
    room.set_presence(&Cursor { x: 10.0, y: 20.0 })
        .expect("set_presence should succeed");

    // Optimistic update should be immediate
    let state = room.presence();
    assert!(
        state.user.is_some(),
        "user presence should be set optimistically"
    );
    assert_eq!(state.user.as_ref().expect("user").x, 10.0);

    // OperationState should transition
    let op_rx = room.presence_operation_state();
    // Give async task time to complete
    tokio::time::sleep(Duration::from_millis(500)).await;
    assert!(op_rx.borrow().is_success(), "presence op should succeed");

    room.leave();
    reactor.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn room_set_presence_with_callbacks() {
    let (_app_id, _token, reactor, handle) = ephemeral_reactor("room-cb").await;

    let room =
        Room::<Cursor>::join(reactor.clone(), handle, "editor", "doc-2").expect("should join room");
    tokio::time::sleep(Duration::from_millis(500)).await;

    let mutate_called = Arc::new(AtomicBool::new(false));
    let success_called = Arc::new(AtomicBool::new(false));
    let settled_called = Arc::new(AtomicBool::new(false));
    let mc = mutate_called.clone();
    let sc = success_called.clone();
    let stc = settled_called.clone();

    let cb = MutationCallbacks::<()>::new()
        .on_mutate(move || mc.store(true, Ordering::SeqCst))
        .on_success(move |_| sc.store(true, Ordering::SeqCst))
        .on_settled(move || stc.store(true, Ordering::SeqCst));

    room.set_presence_with_callbacks(&Cursor { x: 5.0, y: 15.0 }, cb)
        .expect("set_presence_with_callbacks should succeed");

    tokio::time::sleep(Duration::from_millis(1000)).await;

    assert!(
        mutate_called.load(Ordering::SeqCst),
        "on_mutate should fire"
    );
    assert!(
        success_called.load(Ordering::SeqCst),
        "on_success should fire"
    );
    assert!(
        settled_called.load(Ordering::SeqCst),
        "on_settled should fire"
    );

    room.leave();
    reactor.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn room_watch_presence_receives_updates() {
    let (_app_id, _token, reactor, handle) = ephemeral_reactor("room-watch").await;

    let room =
        Room::<Cursor>::join(reactor.clone(), handle, "editor", "doc-3").expect("should join room");
    tokio::time::sleep(Duration::from_millis(500)).await;

    let mut rx = room.watch_presence();

    room.set_presence(&Cursor { x: 42.0, y: 84.0 })
        .expect("set_presence");

    // Watch should receive the optimistic update
    let got_update = tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            rx.changed().await.expect("watch should not close");
            let state = rx.borrow().clone();
            if state.user.is_some() {
                return state;
            }
        }
    })
    .await;

    assert!(
        got_update.is_ok(),
        "should receive presence update within 3s"
    );
    let state = got_update.expect("presence state");
    assert_eq!(state.user.expect("user").x, 42.0);

    room.leave();
    reactor.stop().await;
}

// ============================================================
// Two-peer presence sync (regression test)
// ============================================================

/// Two reactors on the same app join the same room, set presence,
/// and verify each sees the other as a peer via the server round-trip.
/// This catches protocol deserialization bugs (e.g., missing room-type,
/// wrong field names) that single-reactor tests miss.
#[tokio::test(flavor = "multi_thread")]
async fn two_peers_see_each_other_presence() {
    let app = make_app("two-peer-presence").await;

    // Create two independent reactors on the same app.
    let config_a = ConnectionConfig::admin(&app.id, &app.admin_token);
    let reactor_a = Arc::new(Reactor::new(config_a));
    reactor_a.start().await.expect("reactor A start");
    tokio::time::sleep(Duration::from_millis(1500)).await;
    let handle_a = tokio::runtime::Handle::current();

    let config_b = ConnectionConfig::admin(&app.id, &app.admin_token);
    let reactor_b = Arc::new(Reactor::new(config_b));
    reactor_b.start().await.expect("reactor B start");
    tokio::time::sleep(Duration::from_millis(1500)).await;
    let handle_b = tokio::runtime::Handle::current();

    // Both join the same room.
    let room_a = Room::<Cursor>::join(reactor_a.clone(), handle_a, "test", "sync-room")
        .expect("A should join");
    tokio::time::sleep(Duration::from_millis(500)).await;

    let room_b = Room::<Cursor>::join(reactor_b.clone(), handle_b, "test", "sync-room")
        .expect("B should join");
    tokio::time::sleep(Duration::from_millis(500)).await;

    // A sets presence.
    room_a
        .set_presence(&Cursor { x: 1.0, y: 2.0 })
        .expect("A set_presence");
    tokio::time::sleep(Duration::from_millis(500)).await;

    // B sets presence.
    room_b
        .set_presence(&Cursor { x: 3.0, y: 4.0 })
        .expect("B set_presence");

    // Wait for server round-trip (RefreshPresence).
    let mut rx_a = room_a.watch_presence();
    let a_sees_peer = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            rx_a.changed().await.expect("watch should not close");
            let state = rx_a.borrow().clone();
            if !state.peers.is_empty() {
                return state;
            }
        }
    })
    .await;

    assert!(
        a_sees_peer.is_ok(),
        "A should see B as a peer within 5 seconds"
    );
    let state_a = a_sees_peer.expect("state_a");
    assert_eq!(state_a.peers.len(), 1, "A should see exactly 1 peer");
    let peer_cursor = state_a.peers.values().next().expect("peer cursor");
    assert_eq!(peer_cursor.x, 3.0, "A should see B's x=3.0");
    assert_eq!(peer_cursor.y, 4.0, "A should see B's y=4.0");

    // Also verify B sees A.
    let mut rx_b = room_b.watch_presence();
    let b_sees_peer = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            rx_b.changed().await.expect("watch should not close");
            let state = rx_b.borrow().clone();
            if !state.peers.is_empty() {
                return state;
            }
        }
    })
    .await;

    assert!(
        b_sees_peer.is_ok(),
        "B should see A as a peer within 5 seconds"
    );
    let state_b = b_sees_peer.expect("state_b");
    assert_eq!(state_b.peers.len(), 1, "B should see exactly 1 peer");
    let peer_cursor_b = state_b.peers.values().next().expect("peer cursor");
    assert_eq!(peer_cursor_b.x, 1.0, "B should see A's x=1.0");
    assert_eq!(peer_cursor_b.y, 2.0, "B should see A's y=2.0");

    // Cleanup
    room_a.leave();
    room_b.leave();
    reactor_a.stop().await;
    reactor_b.stop().await;
}

// ============================================================
// Two-peer todos sync (subscribe + transact)
// ============================================================

/// Admin client writes a todo, WebSocket client B sees it via
/// subscription. Then update + delete, B sees each change.
/// Covers the todos and merge_tiles recipe pattern.
#[tokio::test(flavor = "multi_thread")]
async fn two_peers_todos_subscribe_and_transact() {
    use futures::StreamExt;
    use instant_client::async_api::InstantAsync;

    let app = make_app("two-peer-todos").await;
    let admin = instant_admin::AdminClient::new(&app.id, &app.admin_token);

    // B connects via WebSocket and subscribes.
    let client_b = InstantAsync::new(ConnectionConfig::admin(&app.id, &app.admin_token))
        .await
        .expect("client B");
    // Wait for InitOk before subscribing.
    tokio::time::sleep(Duration::from_millis(1500)).await;
    let mut stream_b = client_b.subscribe(&serde_json::json!({"todos": {}})).await;

    // Wait for initial empty result.
    let initial = tokio::time::timeout(Duration::from_secs(10), stream_b.next())
        .await
        .expect("should get initial data")
        .expect("stream should yield");
    let initial_count = initial
        .get("todos")
        .and_then(|v| v.as_array())
        .map(|a| a.len())
        .unwrap_or(0);
    assert_eq!(initial_count, 0, "should start with no todos");

    // A creates a todo via admin REST API.
    let todo_id = uuid::Uuid::new_v4().to_string();
    let tx = serde_json::json!([
        ["update", "todos", &todo_id, {"text": "test-todo", "done": false}]
    ]);
    admin.transact(&tx).await.expect("admin transact");

    // B should see it via subscription.
    let update = tokio::time::timeout(Duration::from_secs(5), async {
        while let Some(data) = stream_b.next().await {
            if let Some(arr) = data.get("todos").and_then(|v| v.as_array()) {
                if !arr.is_empty() {
                    return arr.clone();
                }
            }
        }
        vec![]
    })
    .await
    .expect("B should see the todo within 5s");

    assert_eq!(update.len(), 1, "B should see exactly 1 todo");
    assert_eq!(
        update[0].get("text").and_then(|v| v.as_str()),
        Some("test-todo")
    );

    // A toggles done.
    let tx2 = serde_json::json!([["update", "todos", &todo_id, {"done": true}]]);
    admin.transact(&tx2).await.expect("admin toggle");

    let toggle_update = tokio::time::timeout(Duration::from_secs(5), async {
        while let Some(data) = stream_b.next().await {
            if let Some(arr) = data.get("todos").and_then(|v| v.as_array()) {
                if let Some(todo) = arr.first() {
                    if todo.get("done").and_then(|v| v.as_bool()) == Some(true) {
                        return true;
                    }
                }
            }
        }
        false
    })
    .await
    .expect("B should see done=true within 5s");
    assert!(toggle_update, "todo should be marked done");

    // A deletes the todo.
    let tx3 = serde_json::json!([["delete", "todos", &todo_id, {}]]);
    admin.transact(&tx3).await.expect("admin delete");

    let delete_update = tokio::time::timeout(Duration::from_secs(5), async {
        while let Some(data) = stream_b.next().await {
            if let Some(arr) = data.get("todos").and_then(|v| v.as_array()) {
                if arr.is_empty() {
                    return true;
                }
            }
        }
        false
    })
    .await
    .expect("B should see empty todos within 5s");
    assert!(delete_update, "todos should be empty after delete");

    client_b.close().await;
}

// ============================================================
// WebSocket transact_chunks (proper client-side transform)
// ============================================================

/// Verifies that transact_chunks (which transforms high-level ops
/// using the attrs catalog) works end-to-end through the WebSocket.
/// This is the correct way to write data — the server rejects raw
/// high-level ops like ["update", ...] on the WebSocket endpoint.
#[tokio::test(flavor = "multi_thread")]
async fn transact_chunks_creates_and_queries() {
    use futures::StreamExt;
    use instant_client::async_api::InstantAsync;
    use instant_core::instatx::tx;
    use instant_core::value::Value;

    let app = make_app("transact-chunks").await;
    let client = InstantAsync::new(ConnectionConfig::admin(&app.id, &app.admin_token))
        .await
        .expect("client");

    // Wait for InitOk (attrs catalog needed for transact_chunks transform).
    tokio::time::sleep(Duration::from_millis(1500)).await;

    // Create a todo via transact_chunks.
    let todo_id = uuid::Uuid::new_v4().to_string();
    let mut attrs = std::collections::BTreeMap::new();
    attrs.insert("text".to_string(), Value::from("chunks-test"));
    attrs.insert("done".to_string(), Value::from(false));
    let chunks = vec![tx("todos", todo_id.as_str()).update(Value::Object(attrs))];

    let result = client.transact_chunks(&chunks).await;
    assert!(
        result.is_ok(),
        "transact_chunks should succeed: {:?}",
        result.err()
    );

    // Verify via subscription.
    let mut stream = client.subscribe(&serde_json::json!({"todos": {}})).await;
    let found = tokio::time::timeout(Duration::from_secs(5), async {
        while let Some(data) = stream.next().await {
            if let Some(arr) = data.get("todos").and_then(|v| v.as_array()) {
                for todo in arr {
                    if todo.get("text").and_then(|v| v.as_str()) == Some("chunks-test") {
                        return true;
                    }
                }
            }
        }
        false
    })
    .await
    .expect("should find todo via subscription within 5s");
    assert!(found, "todo created via transact_chunks should be queryable");

    // Delete via transact_chunks.
    let del_chunks = vec![tx("todos", todo_id.as_str()).delete()];
    client
        .transact_chunks(&del_chunks)
        .await
        .expect("delete should succeed");

    client.close().await;
}

// ============================================================
// Two-peer topic channel broadcast (reactions pattern)
// ============================================================

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
struct EmojiReaction {
    emoji: String,
    sender: String,
}

/// Two reactors on the same app: A publishes an emoji reaction,
/// B receives it via TopicChannel watch. Covers the reactions recipe.
#[tokio::test(flavor = "multi_thread")]
async fn two_peers_topic_channel_broadcast() {
    let app = make_app("two-peer-topic").await;

    let config_a = ConnectionConfig::admin(&app.id, &app.admin_token);
    let reactor_a = Arc::new(Reactor::new(config_a));
    reactor_a.start().await.expect("reactor A");
    tokio::time::sleep(Duration::from_millis(1500)).await;
    let handle_a = tokio::runtime::Handle::current();

    let config_b = ConnectionConfig::admin(&app.id, &app.admin_token);
    let reactor_b = Arc::new(Reactor::new(config_b));
    reactor_b.start().await.expect("reactor B");
    tokio::time::sleep(Duration::from_millis(1500)).await;
    let handle_b = tokio::runtime::Handle::current();

    // TopicChannel::subscribe now calls join_room internally (matching JS client).
    // Both subscribe to the same topic.
    let channel_a = TopicChannel::<EmojiReaction>::subscribe(
        reactor_a.clone(),
        handle_a,
        "reactions",
        "lobby",
        "emoji",
    )
    .expect("A subscribe");

    let channel_b = TopicChannel::<EmojiReaction>::subscribe(
        reactor_b.clone(),
        handle_b,
        "reactions",
        "lobby",
        "emoji",
    )
    .expect("B subscribe");

    // Wait for both topic subscriptions to be active on the server.
    tokio::time::sleep(Duration::from_millis(1000)).await;

    // Set up B's watcher BEFORE A publishes.
    let mut rx_b = channel_b.watch();

    // A publishes.
    channel_a
        .publish(&EmojiReaction {
            emoji: "fire".into(),
            sender: "alice".into(),
        })
        .expect("A publish");

    // B should receive it via watch.
    let b_received = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            rx_b.changed().await.expect("watch");
            let events = rx_b.borrow().clone();
            if let Some(ev) = events.last() {
                if ev.data.emoji == "fire" {
                    return ev.data.clone();
                }
            }
        }
    })
    .await;

    assert!(b_received.is_ok(), "B should receive the emoji within 5s");
    let reaction = b_received.expect("reaction");
    assert_eq!(reaction.emoji, "fire");
    assert_eq!(reaction.sender, "alice");

    reactor_a.stop().await;
    reactor_b.stop().await;
}

// ============================================================
// Two-peer typing presence sync
// ============================================================

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
struct TypingPresence {
    name: String,
    is_typing: bool,
}

/// Two reactors: A sets is_typing=true, B sees it in peers.
/// Then A sets is_typing=false, B sees the change.
/// Covers the typing_indicator recipe.
#[tokio::test(flavor = "multi_thread")]
async fn two_peers_typing_presence_sync() {
    let app = make_app("two-peer-typing").await;

    let config_a = ConnectionConfig::admin(&app.id, &app.admin_token);
    let reactor_a = Arc::new(Reactor::new(config_a));
    reactor_a.start().await.expect("reactor A");
    tokio::time::sleep(Duration::from_millis(1500)).await;
    let handle_a = tokio::runtime::Handle::current();

    let config_b = ConnectionConfig::admin(&app.id, &app.admin_token);
    let reactor_b = Arc::new(Reactor::new(config_b));
    reactor_b.start().await.expect("reactor B");
    tokio::time::sleep(Duration::from_millis(1500)).await;
    let handle_b = tokio::runtime::Handle::current();

    let room_a = Room::<TypingPresence>::join(reactor_a.clone(), handle_a, "typing", "chat")
        .expect("A join");
    tokio::time::sleep(Duration::from_millis(500)).await;

    let room_b = Room::<TypingPresence>::join(reactor_b.clone(), handle_b, "typing", "chat")
        .expect("B join");
    tokio::time::sleep(Duration::from_millis(500)).await;

    // B sets idle presence so A can see B.
    room_b
        .set_presence(&TypingPresence {
            name: "bob".into(),
            is_typing: false,
        })
        .expect("B set presence");
    tokio::time::sleep(Duration::from_millis(500)).await;

    // A starts typing.
    room_a
        .set_presence(&TypingPresence {
            name: "alice".into(),
            is_typing: true,
        })
        .expect("A start typing");

    // B should see A typing.
    let mut rx_b = room_b.watch_presence();
    let b_sees_typing = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            rx_b.changed().await.expect("watch");
            let state = rx_b.borrow().clone();
            for p in state.peers.values() {
                if p.name == "alice" && p.is_typing {
                    return true;
                }
            }
        }
    })
    .await;
    assert!(
        b_sees_typing.is_ok(),
        "B should see alice typing within 5s"
    );

    // A stops typing.
    room_a
        .set_presence(&TypingPresence {
            name: "alice".into(),
            is_typing: false,
        })
        .expect("A stop typing");

    // B should see A not typing.
    let b_sees_idle = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            rx_b.changed().await.expect("watch");
            let state = rx_b.borrow().clone();
            for p in state.peers.values() {
                if p.name == "alice" && !p.is_typing {
                    return true;
                }
            }
        }
    })
    .await;
    assert!(
        b_sees_idle.is_ok(),
        "B should see alice idle within 5s"
    );

    room_a.leave();
    room_b.leave();
    reactor_a.stop().await;
    reactor_b.stop().await;
}

// ============================================================
// Two-peer merge tiles sync (subscribe + transact for grid)
// ============================================================

/// Admin writes a tile, WebSocket B sees the color change.
/// Covers the merge_tiles recipe.
#[tokio::test(flavor = "multi_thread")]
async fn two_peers_merge_tiles_sync() {
    use futures::StreamExt;
    use instant_client::async_api::InstantAsync;

    let app = make_app("two-peer-tiles").await;
    let admin = instant_admin::AdminClient::new(&app.id, &app.admin_token);

    // Seed a tile via admin.
    let tile_id = "00000000-0000-0000-0001-000000000002";
    let seed = serde_json::json!([
        ["update", "tiles", tile_id, {"row": 1, "col": 2, "color": "gray"}]
    ]);
    admin.transact(&seed).await.expect("seed tile");

    // B subscribes via WebSocket.
    let client_b = InstantAsync::new(ConnectionConfig::admin(&app.id, &app.admin_token))
        .await
        .expect("client B");
    let mut stream_b = client_b.subscribe(&serde_json::json!({"tiles": {}})).await;

    // Wait for initial data with the gray tile.
    let initial = tokio::time::timeout(Duration::from_secs(5), async {
        while let Some(data) = stream_b.next().await {
            if let Some(arr) = data.get("tiles").and_then(|v| v.as_array()) {
                if !arr.is_empty() {
                    return arr.clone();
                }
            }
        }
        vec![]
    })
    .await
    .expect("B should see initial tile");
    assert!(!initial.is_empty(), "should have at least 1 tile");

    // A paints it red via admin.
    let paint = serde_json::json!([["update", "tiles", tile_id, {"color": "red"}]]);
    admin.transact(&paint).await.expect("paint red");

    // B should see the color change.
    let color_update = tokio::time::timeout(Duration::from_secs(5), async {
        while let Some(data) = stream_b.next().await {
            if let Some(arr) = data.get("tiles").and_then(|v| v.as_array()) {
                for tile in arr {
                    if tile.get("color").and_then(|v| v.as_str()) == Some("red") {
                        return true;
                    }
                }
            }
        }
        false
    })
    .await
    .expect("B should see red within 5s");
    assert!(color_update, "tile should be red");

    client_b.close().await;
}

// ============================================================
// Two-peer cursor position sync
// ============================================================

/// Two reactors set cursor positions, verify each sees the
/// other's x/y coordinates. Covers the cursors recipe.
#[tokio::test(flavor = "multi_thread")]
async fn two_peers_cursor_position_sync() {
    #[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
    struct CursorPos {
        name: String,
        x: i32,
        y: i32,
    }

    let app = make_app("two-peer-cursor").await;

    let config_a = ConnectionConfig::admin(&app.id, &app.admin_token);
    let reactor_a = Arc::new(Reactor::new(config_a));
    reactor_a.start().await.expect("reactor A");
    tokio::time::sleep(Duration::from_millis(1500)).await;
    let handle_a = tokio::runtime::Handle::current();

    let config_b = ConnectionConfig::admin(&app.id, &app.admin_token);
    let reactor_b = Arc::new(Reactor::new(config_b));
    reactor_b.start().await.expect("reactor B");
    tokio::time::sleep(Duration::from_millis(1500)).await;
    let handle_b = tokio::runtime::Handle::current();

    let room_a = Room::<CursorPos>::join(reactor_a.clone(), handle_a, "cursors", "canvas")
        .expect("A join");
    tokio::time::sleep(Duration::from_millis(500)).await;

    let room_b = Room::<CursorPos>::join(reactor_b.clone(), handle_b, "cursors", "canvas")
        .expect("B join");
    tokio::time::sleep(Duration::from_millis(500)).await;

    // A sets position (10, 15).
    room_a
        .set_presence(&CursorPos {
            name: "alice".into(),
            x: 10,
            y: 15,
        })
        .expect("A set cursor");
    tokio::time::sleep(Duration::from_millis(500)).await;

    // B sets position (30, 5).
    room_b
        .set_presence(&CursorPos {
            name: "bob".into(),
            x: 30,
            y: 5,
        })
        .expect("B set cursor");

    // A should see B's cursor at (30, 5).
    let mut rx_a = room_a.watch_presence();
    let a_sees_b = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            rx_a.changed().await.expect("watch");
            let state = rx_a.borrow().clone();
            for p in state.peers.values() {
                if p.name == "bob" && p.x == 30 && p.y == 5 {
                    return true;
                }
            }
        }
    })
    .await;
    assert!(a_sees_b.is_ok(), "A should see bob at (30,5) within 5s");

    // B should see A's cursor at (10, 15).
    let mut rx_b = room_b.watch_presence();
    let b_sees_a = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            rx_b.changed().await.expect("watch");
            let state = rx_b.borrow().clone();
            for p in state.peers.values() {
                if p.name == "alice" && p.x == 10 && p.y == 15 {
                    return true;
                }
            }
        }
    })
    .await;
    assert!(b_sees_a.is_ok(), "B should see alice at (10,15) within 5s");

    // A moves to a new position.
    room_a
        .set_presence(&CursorPos {
            name: "alice".into(),
            x: 20,
            y: 20,
        })
        .expect("A move");

    let b_sees_move = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            rx_b.changed().await.expect("watch");
            let state = rx_b.borrow().clone();
            for p in state.peers.values() {
                if p.name == "alice" && p.x == 20 && p.y == 20 {
                    return true;
                }
            }
        }
    })
    .await;
    assert!(
        b_sees_move.is_ok(),
        "B should see alice move to (20,20) within 5s"
    );

    room_a.leave();
    room_b.leave();
    reactor_a.stop().await;
    reactor_b.stop().await;
}

// ============================================================
// TopicChannel + PublishHandle tests
// ============================================================

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
struct Emoji {
    name: String,
}

#[tokio::test(flavor = "multi_thread")]
async fn topic_publish_returns_publish_handle() {
    let (_app_id, _token, reactor, handle) = ephemeral_reactor("topic-handle").await;

    let channel =
        TopicChannel::<Emoji>::subscribe(reactor.clone(), handle, "game", "room-1", "emoji")
            .expect("subscribe should succeed");

    let publish_handle = channel
        .publish(&Emoji {
            name: "fire".into(),
        })
        .expect("publish should succeed");

    // PublishHandle starts in-flight
    assert!(
        publish_handle.is_loading() || publish_handle.is_success(),
        "handle should be loading or already succeeded"
    );

    // Wait for broadcast to complete
    tokio::time::sleep(Duration::from_millis(1000)).await;

    assert!(publish_handle.is_success(), "publish should succeed");
    assert!(publish_handle.error().is_none(), "no error expected");

    reactor.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn topic_publish_with_callbacks_fires_all() {
    let (_app_id, _token, reactor, handle) = ephemeral_reactor("topic-cb").await;

    let channel =
        TopicChannel::<Emoji>::subscribe(reactor.clone(), handle, "game", "room-2", "emoji")
            .expect("subscribe");

    let mutate_called = Arc::new(AtomicBool::new(false));
    let success_called = Arc::new(AtomicBool::new(false));
    let settled_called = Arc::new(AtomicBool::new(false));
    let mc = mutate_called.clone();
    let sc = success_called.clone();
    let stc = settled_called.clone();

    let cb = MutationCallbacks::<()>::new()
        .on_mutate(move || mc.store(true, Ordering::SeqCst))
        .on_success(move |_| sc.store(true, Ordering::SeqCst))
        .on_settled(move || stc.store(true, Ordering::SeqCst));

    let handle_result = channel
        .publish_with_callbacks(
            &Emoji {
                name: "wave".into(),
            },
            cb,
        )
        .expect("publish_with_callbacks");

    tokio::time::sleep(Duration::from_millis(1500)).await;

    assert!(
        mutate_called.load(Ordering::SeqCst),
        "on_mutate should fire"
    );
    assert!(
        success_called.load(Ordering::SeqCst),
        "on_success should fire"
    );
    assert!(
        settled_called.load(Ordering::SeqCst),
        "on_settled should fire"
    );
    assert!(
        handle_result.is_success(),
        "PublishHandle should show success"
    );

    reactor.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn topic_channel_accessors() {
    let (_app_id, _token, reactor, handle) = ephemeral_reactor("topic-access").await;

    let channel =
        TopicChannel::<Emoji>::subscribe(reactor.clone(), handle, "game", "room-3", "emoji")
            .expect("subscribe");

    assert_eq!(channel.topic(), "emoji");
    assert_eq!(channel.room_type(), "game");
    assert_eq!(channel.room_id(), "room-3");
    assert!(channel.events().is_empty(), "no events yet");
    assert!(channel.latest_event().is_none(), "no latest event yet");

    reactor.stop().await;
}

// ============================================================
// InstantDB top-level API tests
// ============================================================

#[tokio::test(flavor = "multi_thread")]
async fn instantdb_init_and_use_query() {
    let app = make_app("instantdb-query").await;

    let db = InstantDB::init(SyncConfig {
        app_id: app.id.clone(),
        admin_token: Some(app.admin_token.clone()),
        ..Default::default()
    })
    .await
    .expect("InstantDB::init should succeed");

    // use_query returns a FetchAll
    let todos = db.use_query::<Todo>();
    assert_eq!(todos.get().len(), 0, "empty app should have no todos");

    // tx returns a Mutator
    db.tx::<Todo>()
        .create(&Todo {
            id: uuid::Uuid::new_v4().to_string(),
            title: "From InstantDB".into(),
            done: false,
        })
        .expect("create via tx()");

    db.disconnect().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn instantdb_auth_state_and_watch() {
    let app = make_app("instantdb-auth").await;

    let db = InstantDB::init(SyncConfig {
        app_id: app.id.clone(),
        admin_token: Some(app.admin_token.clone()),
        ..Default::default()
    })
    .await
    .expect("init");

    // Auth state starts unauthenticated
    let state = db.auth_state();
    assert!(
        matches!(*state.get(), AuthState::Unauthenticated),
        "initial auth state should be Unauthenticated"
    );

    // watch_auth_state returns a receiver
    let rx = db.watch_auth_state();
    assert!(
        matches!(*rx.borrow(), AuthState::Unauthenticated),
        "watch receiver should also be Unauthenticated"
    );

    db.disconnect().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn instantdb_room_and_topic_delegation() {
    let app = make_app("instantdb-rt").await;

    let db = InstantDB::init(SyncConfig {
        app_id: app.id.clone(),
        admin_token: Some(app.admin_token.clone()),
        ..Default::default()
    })
    .await
    .expect("init");

    // room() delegates to engine
    let room = db.room::<Cursor>("editor", "doc-1");
    assert!(room.is_ok(), "room() should succeed: {:?}", room.err());

    // topic() delegates to engine
    let topic = db.topic::<Emoji>("game", "room-1", "emoji");
    assert!(topic.is_ok(), "topic() should succeed: {:?}", topic.err());

    db.disconnect().await;
}

// ============================================================
// Auth success path (real server)
// ============================================================

#[tokio::test(flavor = "multi_thread")]
async fn auth_sign_in_as_guest_succeeds() {
    let app = make_app("auth-guest").await;

    let auth = sharing_instant::AuthCoordinator::new(&app.id);
    let result = auth.sign_in_as_guest().await;

    assert!(
        result.is_ok(),
        "sign_in_as_guest should succeed with ephemeral app: {:?}",
        result.err()
    );

    let user = result.expect("auth user");
    assert!(!user.id.is_empty(), "user should have an id");
    assert!(
        user.refresh_token.is_some(),
        "guest should have a refresh token"
    );

    // State should be Guest
    let state = auth.state();
    assert!(
        matches!(*state.get(), AuthState::Guest { .. }),
        "state should be Guest after sign_in_as_guest, got: {:?}",
        *state.get()
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn auth_sign_in_as_guest_with_callbacks_succeeds() {
    let app = make_app("auth-guest-cb").await;

    let auth = sharing_instant::AuthCoordinator::new(&app.id);

    let mutate_called = Arc::new(AtomicBool::new(false));
    let success_called = Arc::new(AtomicBool::new(false));
    let settled_called = Arc::new(AtomicBool::new(false));
    let mc = mutate_called.clone();
    let sc = success_called.clone();
    let stc = settled_called.clone();

    let cb = MutationCallbacks::new()
        .on_mutate(move || mc.store(true, Ordering::SeqCst))
        .on_success(move |user: sharing_instant::AuthUser| {
            assert!(!user.id.is_empty());
            sc.store(true, Ordering::SeqCst);
        })
        .on_settled(move || stc.store(true, Ordering::SeqCst));

    auth.sign_in_as_guest_with_callbacks(cb).await;

    assert!(mutate_called.load(Ordering::SeqCst), "on_mutate");
    assert!(success_called.load(Ordering::SeqCst), "on_success");
    assert!(settled_called.load(Ordering::SeqCst), "on_settled");
}

#[tokio::test(flavor = "multi_thread")]
async fn auth_sign_in_then_sign_out() {
    let app = make_app("auth-signout").await;

    let auth = sharing_instant::AuthCoordinator::new(&app.id);

    // Sign in
    auth.sign_in_as_guest().await.expect("sign in");
    assert!(matches!(*auth.state().get(), AuthState::Guest { .. }));

    // Sign out
    auth.sign_out().await.expect("sign out");
    assert!(matches!(*auth.state().get(), AuthState::Unauthenticated));
}

#[tokio::test(flavor = "multi_thread")]
async fn auth_sign_in_with_token_round_trip() {
    let app = make_app("auth-token").await;

    let auth = sharing_instant::AuthCoordinator::new(&app.id);

    // Get a guest token
    let user = auth.sign_in_as_guest().await.expect("guest sign in");
    let token = user.refresh_token.expect("should have refresh token");

    // Sign out, then re-authenticate with the token
    auth.sign_out().await.expect("sign out");
    assert!(matches!(*auth.state().get(), AuthState::Unauthenticated));

    let user2 = auth
        .sign_in_with_token(&token)
        .await
        .expect("token sign in");
    assert_eq!(user.id, user2.id, "should be the same user");
    assert!(matches!(
        *auth.state().get(),
        AuthState::Authenticated { .. }
    ));
}
