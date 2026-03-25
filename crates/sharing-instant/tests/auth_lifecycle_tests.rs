//! Tests for authentication lifecycle.
//!
//! Maps Swift's AccountLifecycleTests.swift to InstantDB auth tokens.
//!
//! Swift test mapping:
//! - iCloud sign-in → InstantDB refresh token → session
//! - iCloud sign-out → token invalidation
//! - account change → re-authentication
//!
//! All tests remain #[ignore] — blocked by rust-instantdb auth not implemented.
//! Test bodies demonstrate the expected auth lifecycle using SyncEngine
//! and SyncConfig as the closest approximation.

use sharing_instant::sync::engine::{SyncConfig, SyncEngine};

#[tokio::test]
#[ignore = "BLOCKED: rust-instantdb auth not implemented in Rust client"]
async fn auth_token_connects_session() {
    // NOTE: Real implementation needs InstantDB auth token exchange.
    // Flow: refresh_token → POST /runtime/auth/token → session_token
    // Then the reactor uses the session_token for the WebSocket handshake.
    let config = SyncConfig {
        app_id: "test-app".to_string(),
        refresh_token: Some("test-refresh-token".to_string()),
        ..Default::default()
    };

    let engine = SyncEngine::new(config);

    // Before connecting, engine should not be connected
    assert!(
        !engine.status().is_connected,
        "should not be connected before start()"
    );
    assert!(
        engine.status().session_id.is_none(),
        "should have no session_id before auth"
    );

    // Start the engine (connects with the refresh token)
    engine
        .start()
        .await
        .expect("start with refresh token should succeed");

    // After start, engine should report connected
    assert!(
        engine.status().is_connected,
        "should be connected after start()"
    );

    // In production, the engine would also have:
    //   - A session_id from the server handshake
    //   - An active WebSocket connection
    //   - Subscriptions registered
    //
    //   assert!(engine.status().session_id.is_some());

    // Clean up
    engine.stop().await;
    assert!(
        !engine.status().is_connected,
        "should be disconnected after stop()"
    );
}

#[tokio::test]
#[ignore = "BLOCKED: rust-instantdb auth not implemented in Rust client"]
async fn sign_out_disconnects() {
    // NOTE: Real implementation needs InstantDB token invalidation.
    // Sign-out should: clear the refresh token, disconnect the WebSocket,
    // and invalidate any cached session state.
    let engine = SyncEngine::new(SyncConfig {
        app_id: "test-app".to_string(),
        refresh_token: Some("test-refresh-token".to_string()),
        ..Default::default()
    });

    // Connect first
    engine.start().await.expect("start should succeed");
    assert!(engine.status().is_connected);

    // Sign out (stop the engine)
    engine.stop().await;

    // Verify disconnected state
    assert!(
        !engine.status().is_connected,
        "should be disconnected after sign out"
    );
    assert!(
        !engine.status().is_sending_changes,
        "should not be sending after sign out"
    );
    assert!(
        !engine.status().is_receiving_changes,
        "should not be receiving after sign out"
    );

    // In production, sign-out would also:
    //   - POST /runtime/auth/sign-out to invalidate the token server-side
    //   - Clear local subscription state
    //   - Emit a status change event
}

#[tokio::test]
#[ignore = "BLOCKED: rust-instantdb auth not implemented in Rust client"]
async fn token_refresh_on_expiry() {
    // NOTE: Real implementation needs automatic token refresh.
    // When the session token expires, the engine should automatically
    // use the refresh_token to obtain a new session_token without
    // interrupting active subscriptions.
    let engine = SyncEngine::new(SyncConfig {
        app_id: "test-app".to_string(),
        refresh_token: Some("test-refresh-token".to_string()),
        ..Default::default()
    });

    engine.start().await.expect("initial start should succeed");
    assert!(engine.status().is_connected);

    // Simulate token expiry by monitoring status changes
    let status_rx = engine.on_status_change();

    // In production, the engine would:
    //   1. Detect a 401/token-expired error from the WebSocket
    //   2. Automatically POST /runtime/auth/token with refresh_token
    //   3. Reconnect the WebSocket with the new session_token
    //   4. Re-register all active subscriptions
    //   5. Continue without data loss
    //
    //   // The status receiver would briefly show disconnected, then reconnected:
    //   status_rx.changed().await.unwrap();
    //   assert!(status_rx.borrow().is_connected, "should auto-reconnect");

    // Verify we can read the current status through the watch channel
    let current_status = status_rx.borrow().clone();
    assert!(
        current_status.is_connected,
        "status channel should reflect current connection state"
    );

    engine.stop().await;
}

#[tokio::test]
#[ignore = "BLOCKED: rust-instantdb auth not implemented in Rust client"]
async fn account_change_resets_local_state() {
    // NOTE: Real implementation needs account-aware cache management.
    // When the user switches accounts (different refresh token), the engine
    // should reset all local state and re-sync from the new account.

    // First account session
    let engine_a = SyncEngine::new(SyncConfig {
        app_id: "test-app".to_string(),
        refresh_token: Some("token-user-a".to_string()),
        ..Default::default()
    });
    engine_a.start().await.expect("user A session should start");
    assert!(engine_a.status().is_connected);

    // Disconnect first account
    engine_a.stop().await;
    assert!(!engine_a.status().is_connected);

    // Second account session with different token
    let engine_b = SyncEngine::new(SyncConfig {
        app_id: "test-app".to_string(),
        refresh_token: Some("token-user-b".to_string()),
        ..Default::default()
    });
    engine_b.start().await.expect("user B session should start");
    assert!(engine_b.status().is_connected);

    // In production, switching accounts would:
    //   1. Clear all local cached data from user A
    //   2. Drop all active subscriptions
    //   3. Establish new session with user B's token
    //   4. Re-run all queries to populate fresh data for user B
    //   5. Subscriptions see entirely new data (no cross-account leakage)
    //
    //   // Verify no data leakage:
    //   let fetch = FetchAll::<Reminder>::new(db);
    //   assert!(fetch.get().iter().all(|r| r.owner_id == "user-b"));

    engine_b.stop().await;
}
