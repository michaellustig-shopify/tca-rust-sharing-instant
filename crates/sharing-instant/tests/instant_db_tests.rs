//! Tests for the InstantDB top-level client (Phase 6).
//!
//! Uses InMemoryDatabase via SyncEngine::new() (no connection) to test
//! the API surface without a real server.

use sharing_instant::database::InMemoryDatabase;
use sharing_instant::instant::InstantDB;
use sharing_instant::sync::engine::SyncConfig;
use sharing_instant::table::{ColumnDef, Table};
use sharing_instant::{ConnectionState, FetchAll, Mutator};
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
struct Todo {
    id: String,
    title: String,
    is_completed: bool,
}

impl Table for Todo {
    const TABLE_NAME: &'static str = "todos";
    fn columns() -> &'static [ColumnDef] {
        &[]
    }
}

#[tokio::test]
async fn new_without_connect() {
    let db = InstantDB::new(SyncConfig {
        app_id: "test-app".into(),
        ..Default::default()
    });

    // Connection state starts disconnected.
    let conn = db.connection_state();
    assert!(matches!(*conn.get(), ConnectionState::Disconnected));
}

// use_query requires a live server connection (LiveDatabase blocks on subscribe),
// so it's tested in integration_tests.rs with #[cfg(feature = "integration")].

#[tokio::test]
async fn tx_returns_mutator() {
    let db = InstantDB::new(SyncConfig {
        app_id: "test".into(),
        ..Default::default()
    });

    let _mutator: Mutator<Todo> = db.tx::<Todo>();
}

#[tokio::test]
async fn auth_starts_unauthenticated() {
    let db = InstantDB::new(SyncConfig {
        app_id: "test".into(),
        ..Default::default()
    });

    let auth_state = db.auth_state();
    assert!(matches!(
        *auth_state.get(),
        sharing_instant::AuthState::Unauthenticated
    ));
}

// sign_in_as_guest hits real InstantDB API, so tested in integration_tests.
// Here just verify the auth state starts unauthenticated and sign_out works.
#[tokio::test]
async fn auth_sign_out_is_noop_without_session() {
    let db = InstantDB::new(SyncConfig {
        app_id: "test".into(),
        ..Default::default()
    });

    db.auth().sign_out().await.expect("sign out should work");
    let state = db.auth_state();
    assert!(matches!(
        *state.get(),
        sharing_instant::AuthState::Unauthenticated
    ));
}

#[tokio::test]
async fn engine_accessor() {
    let db = InstantDB::new(SyncConfig {
        app_id: "test".into(),
        ..Default::default()
    });

    let engine = db.engine();
    assert!(!engine.status().is_connected);
}
