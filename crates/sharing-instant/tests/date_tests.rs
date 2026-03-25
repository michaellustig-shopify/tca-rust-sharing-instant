//! Tests for date value roundtripping through InstantDB.
//!
//! Maps Swift's DateTests.swift to InstantDB Value date handling.

use sharing_instant::database::InMemoryDatabase;
use sharing_instant::fetch_one::FetchOne;
use sharing_instant::table::{ColumnDef, Table};
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
struct Event {
    id: String,
    name: String,
    timestamp: f64,
}

impl Table for Event {
    const TABLE_NAME: &'static str = "events";
    fn columns() -> &'static [ColumnDef] {
        &[]
    }
}

#[test]
fn date_roundtrip_as_f64_timestamp() {
    let db = Arc::new(InMemoryDatabase::new());

    let timestamp = 1700000000.0_f64; // Nov 14, 2023
    db.insert(
        "events",
        "e1",
        serde_json::json!({
            "id": "e1",
            "name": "Launch",
            "timestamp": timestamp,
        }),
    );

    let fetch = FetchOne::<Event>::new(db);
    let event = fetch.get().expect("should have event");
    assert_eq!(event.timestamp, timestamp);
}

#[test]
fn date_zero_epoch() {
    let db = Arc::new(InMemoryDatabase::new());

    db.insert(
        "events",
        "e1",
        serde_json::json!({
            "id": "e1",
            "name": "Epoch",
            "timestamp": 0.0,
        }),
    );

    let fetch = FetchOne::<Event>::new(db);
    let event = fetch.get().expect("should have event");
    assert_eq!(event.timestamp, 0.0);
}

#[test]
fn date_negative_timestamp() {
    let db = Arc::new(InMemoryDatabase::new());

    db.insert(
        "events",
        "e1",
        serde_json::json!({
            "id": "e1",
            "name": "Before Epoch",
            "timestamp": -86400.0,
        }),
    );

    let fetch = FetchOne::<Event>::new(db);
    let event = fetch.get().expect("should have event");
    assert_eq!(event.timestamp, -86400.0);
}
