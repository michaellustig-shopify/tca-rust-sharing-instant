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
