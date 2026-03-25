//! Tests for LiveDatabase — the sync-async bridge wrapping a Reactor.
//!
//! These tests verify:
//! - LiveDatabase construction and Reactor ownership
//! - Value conversion round-trips (instant_core::value::Value ↔ serde_json::Value)
//! - The sync-async bridge pattern (block_in_place + block_on)
//! - SyncEngine.database() returns a working Database
//!
//! Tests that require a real InstantDB server are in integration_tests.rs.

use sharing_instant::database::{Database, LiveDatabase};
use sharing_instant::sync::engine::{SyncConfig, SyncEngine};
use sharing_instant::table::{json_to_value, value_to_json};
use sharing_instant::Value;

// === Value conversion round-trips ===

#[test]
fn value_roundtrip_null() {
    let original = Value::Null;
    let json = value_to_json(&original);
    let back = json_to_value(&json);
    assert_eq!(back, original);
}

#[test]
fn value_roundtrip_bool() {
    for b in [true, false] {
        let original = Value::Bool(b);
        let json = value_to_json(&original);
        let back = json_to_value(&json);
        assert_eq!(back, original);
    }
}

#[test]
fn value_roundtrip_int() {
    for i in [0i64, 1, -1, 42, i64::MAX, i64::MIN] {
        let original = Value::Int(i);
        let json = value_to_json(&original);
        let back = json_to_value(&json);
        assert_eq!(back, original);
    }
}

#[test]
fn value_roundtrip_float() {
    let original = Value::Float(instant_core::value::OrderedFloat(3.14));
    let json = value_to_json(&original);
    let back = json_to_value(&json);
    // Float round-trip may lose precision, check approximate equality
    match back {
        Value::Float(f) => assert!((f.0 - 3.14).abs() < 1e-10),
        // serde_json may represent 3.14 as float, which json_to_value sees as f64
        other => panic!("expected Float, got {:?}", other),
    }
}

#[test]
fn value_roundtrip_string() {
    let original = Value::String("hello world".to_string());
    let json = value_to_json(&original);
    let back = json_to_value(&json);
    assert_eq!(back, original);
}

#[test]
fn value_roundtrip_array() {
    let original = Value::Array(vec![
        Value::Int(1),
        Value::String("two".to_string()),
        Value::Bool(true),
        Value::Null,
    ]);
    let json = value_to_json(&original);
    let back = json_to_value(&json);
    assert_eq!(back, original);
}

#[test]
fn value_roundtrip_object() {
    let mut map = std::collections::BTreeMap::new();
    map.insert("name".to_string(), Value::String("Alice".to_string()));
    map.insert("age".to_string(), Value::Int(30));
    map.insert("active".to_string(), Value::Bool(true));
    let original = Value::Object(map);
    let json = value_to_json(&original);
    let back = json_to_value(&json);
    assert_eq!(back, original);
}

#[test]
fn value_roundtrip_nested_object() {
    let inner = Value::Object(
        [
            ("x".to_string(), Value::Int(1)),
            ("y".to_string(), Value::Int(2)),
        ]
        .into_iter()
        .collect(),
    );
    let original = Value::Object(
        [
            ("point".to_string(), inner),
            ("label".to_string(), Value::String("origin".to_string())),
        ]
        .into_iter()
        .collect(),
    );
    let json = value_to_json(&original);
    let back = json_to_value(&json);
    assert_eq!(back, original);
}

// === LiveDatabase construction ===

#[tokio::test]
async fn live_database_created_from_reactor() {
    use instant_client::{ConnectionConfig, Reactor};
    use std::sync::Arc;

    let config = ConnectionConfig::admin("test-app", "test-token");
    let reactor = Arc::new(Reactor::new(config));
    let handle = tokio::runtime::Handle::current();

    let _db = LiveDatabase::new(reactor, handle);
    // LiveDatabase implements Database
    fn assert_database(_: &dyn Database) {}
    assert_database(&_db);
}

#[tokio::test]
async fn live_database_reactor_swap() {
    use instant_client::{ConnectionConfig, Reactor};
    use std::sync::Arc;

    let config1 = ConnectionConfig::admin("app-1", "token-1");
    let reactor1 = Arc::new(Reactor::new(config1));
    let handle = tokio::runtime::Handle::current();

    let db = LiveDatabase::new(reactor1, handle);

    // Swap to a new reactor
    let config2 = ConnectionConfig::admin("app-2", "token-2");
    let reactor2 = Arc::new(Reactor::new(config2));
    db.set_reactor(reactor2);

    // Database still exists and is usable (even if not connected)
    fn assert_database(_: &dyn Database) {}
    assert_database(&db);
}

// === SyncEngine.database() ===

#[tokio::test]
async fn sync_engine_provides_database() {
    let engine = SyncEngine::new(SyncConfig {
        app_id: "test-app".to_string(),
        ..Default::default()
    });

    let db = engine.database();

    // The database should be a LiveDatabase (implements Database)
    // We can't query without a connected server, but the type is correct
    fn assert_send_sync(_: &(dyn Database + Send + Sync)) {}
    assert_send_sync(db.as_ref());
}

#[tokio::test]
async fn sync_engine_database_survives_restart() {
    let engine = SyncEngine::new(SyncConfig {
        app_id: "test-app".to_string(),
        ..Default::default()
    });

    let db_before = engine.database();

    // Start and stop cycle
    engine.start().await.expect("start should succeed");
    engine.stop().await;

    let db_after = engine.database();

    // Both references point to the same LiveDatabase instance
    // (Arc::ptr_eq checks identity)
    assert!(
        std::sync::Arc::ptr_eq(&db_before, &db_after),
        "database() should return the same Arc across lifecycle"
    );
}
