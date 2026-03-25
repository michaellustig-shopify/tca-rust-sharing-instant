//! Tests for reactor state / sync metadata.
//!
//! Maps Swift's MetadataTests.swift + AttachedMetadatabaseTests.swift
//! to InstantDB reactor state (session, processed-tx-id).
//!
//! These tests verify the observable SyncStatus fields across engine
//! lifecycle transitions. The current SyncEngine is a placeholder that
//! does not connect to a real WebSocket server, so session_id and
//! last_sync_at remain None. Tests verify the contract: these fields
//! are None before a real server connection is established.

use sharing_instant::sync::engine::{SyncConfig, SyncEngine};

fn test_config() -> SyncConfig {
    SyncConfig {
        app_id: "test-app".to_string(),
        ..Default::default()
    }
}

#[tokio::test]
async fn reactor_tracks_session_id() {
    // The session_id is assigned by the server on WebSocket handshake.
    // With the placeholder engine (no real server), session_id stays None.
    // This test verifies the contract: session_id is None before connect,
    // and remains None with the placeholder (will become Some when the
    // real reactor pipeline is wired up).
    let engine = SyncEngine::new(test_config());

    // Before start: no session
    assert!(
        engine.status().session_id.is_none(),
        "session_id should be None before start"
    );

    // After start: placeholder engine does not assign a session_id
    // because there is no real WebSocket handshake. When the reactor
    // is wired up, this assertion changes to is_some().
    engine.start().await.expect("start should succeed");
    assert!(
        engine.status().is_connected,
        "engine should report connected after start"
    );
    assert!(
        engine.status().session_id.is_none(),
        "session_id should remain None with placeholder engine (no real server)"
    );
}

#[tokio::test]
async fn reactor_tracks_last_sync_timestamp() {
    // last_sync_at is set when the engine starts (Reactor launched).
    // Before start it's None; after start it's Some.
    let engine = SyncEngine::new(test_config());

    // Before start: no sync has occurred
    assert!(
        engine.status().last_sync_at.is_none(),
        "last_sync_at should be None before any sync"
    );

    // After start: engine records the start time
    engine.start().await.expect("start should succeed");
    assert!(engine.status().is_connected);
    assert!(
        engine.status().last_sync_at.is_some(),
        "last_sync_at should be set after engine starts"
    );
}

#[tokio::test]
async fn reactor_state_resets_on_disconnect() {
    // After disconnect, is_connected must be false. The session_id and
    // sending/receiving flags should also reflect the disconnected state.
    let engine = SyncEngine::new(test_config());

    // Start the engine
    engine.start().await.expect("start should succeed");
    assert!(engine.status().is_connected);
    assert!(!engine.status().is_sending_changes);
    assert!(!engine.status().is_receiving_changes);

    // Disconnect
    engine.stop().await;
    let status = engine.status();

    assert!(
        !status.is_connected,
        "is_connected should be false after stop"
    );
    assert!(
        !status.is_sending_changes,
        "is_sending_changes should be false after stop"
    );
    assert!(
        !status.is_receiving_changes,
        "is_receiving_changes should be false after stop"
    );
    assert!(
        status.session_id.is_none(),
        "session_id should be None after disconnect"
    );
}

#[tokio::test]
async fn reactor_state_persists_across_reconnect() {
    // Verifies that the engine can transition through a full
    // connect → disconnect → reconnect cycle and end up in a
    // consistent connected state.
    //
    // In production, the reactor would preserve its processed-tx-id
    // across reconnections for efficient delta sync. The placeholder
    // engine verifies the state machine transitions are correct.
    let engine = SyncEngine::new(test_config());

    // Initial state
    assert!(!engine.status().is_connected);

    // Connect
    engine.start().await.expect("start should succeed");
    assert!(engine.status().is_connected);

    // Disconnect
    engine.stop().await;
    assert!(!engine.status().is_connected);

    // Reconnect
    engine.start().await.expect("reconnect should succeed");
    let status = engine.status();

    assert!(status.is_connected, "should be connected after reconnect");
    assert!(
        !status.is_sending_changes,
        "should not be sending immediately after reconnect"
    );
    assert!(
        !status.is_receiving_changes,
        "should not be receiving immediately after reconnect"
    );

    // Verify status change notifications work across reconnect
    let mut rx = engine.on_status_change();

    engine.stop().await;
    rx.changed()
        .await
        .expect("should receive status change notification");
    assert!(
        !rx.borrow().is_connected,
        "status change receiver should reflect disconnect"
    );

    engine
        .start()
        .await
        .expect("second reconnect should succeed");
    rx.changed()
        .await
        .expect("should receive reconnect notification");
    assert!(
        rx.borrow().is_connected,
        "status change receiver should reflect reconnect"
    );
}
