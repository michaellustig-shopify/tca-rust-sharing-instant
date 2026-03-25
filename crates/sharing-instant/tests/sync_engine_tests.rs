//! Tests for SyncEngine.
//!
//! Mirrors tests from Swift's SyncEngineTests.swift and
//! SyncEngineLifecycleTests.swift.

use sharing_instant::sync::engine::{SyncConfig, SyncEngine};

#[tokio::test]
async fn sync_engine_default_status() {
    let engine = SyncEngine::new(SyncConfig::default());
    let status = engine.status();
    assert!(!status.is_connected);
    assert!(!status.is_sending_changes);
    assert!(!status.is_receiving_changes);
    assert!(status.session_id.is_none());
}

#[tokio::test]
async fn sync_engine_start() {
    let engine = SyncEngine::new(SyncConfig {
        app_id: "test-app".to_string(),
        ..Default::default()
    });

    engine.start().await.unwrap();
    assert!(engine.status().is_connected);
}

#[tokio::test]
async fn sync_engine_stop() {
    let engine = SyncEngine::new(SyncConfig::default());

    engine.start().await.unwrap();
    assert!(engine.status().is_connected);

    engine.stop().await;
    assert!(!engine.status().is_connected);
}

#[tokio::test]
async fn sync_engine_status_change_notification() {
    let engine = SyncEngine::new(SyncConfig::default());
    let mut rx = engine.on_status_change();

    engine.start().await.unwrap();

    // The receiver should have been notified
    rx.changed().await.unwrap();
    assert!(rx.borrow().is_connected);
}

#[test]
fn sync_config_default_ws_uri() {
    let config = SyncConfig::default();
    assert_eq!(config.ws_uri, "wss://api.instantdb.com/runtime/session");
}
