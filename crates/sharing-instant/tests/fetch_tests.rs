//! Tests for Fetch<R: FetchKeyRequest>.
//!
//! Mirrors tests from Swift's FetchTests.swift.

use sharing_instant::database::{Database, InMemoryDatabase};
use sharing_instant::error::Result;
use sharing_instant::fetch::Fetch;
use sharing_instant::fetch_key_request::FetchKeyRequest;
use sharing_instant::Value;
use std::sync::Arc;

#[derive(Debug, Clone, Default)]
struct DashboardStats {
    total_items: usize,
    active_items: usize,
}

struct DashboardRequest;

impl FetchKeyRequest for DashboardRequest {
    type Value = DashboardStats;

    fn fetch(&self, db: &dyn Database) -> Result<DashboardStats> {
        let query = sharing_instant::table::json_to_value(&serde_json::json!({ "items": {} }));
        let result = db.query(&query)?;

        let total = match &result {
            Value::Object(obj) => match obj.get("items") {
                Some(Value::Array(arr)) => arr.len(),
                _ => 0,
            },
            _ => 0,
        };

        Ok(DashboardStats {
            total_items: total,
            active_items: total, // simplified
        })
    }

    fn queries(&self) -> Vec<Value> {
        vec![sharing_instant::table::json_to_value(
            &serde_json::json!({ "items": {} }),
        )]
    }
}

#[test]
fn fetch_empty_database() {
    let db = Arc::new(InMemoryDatabase::new());
    let fetch = Fetch::new(DashboardRequest, db);
    assert_eq!(fetch.get().total_items, 0);
}

#[test]
fn fetch_with_data() {
    let db = Arc::new(InMemoryDatabase::new());

    db.insert("items", "i1", serde_json::json!({"id": "i1"}));
    db.insert("items", "i2", serde_json::json!({"id": "i2"}));
    db.insert("items", "i3", serde_json::json!({"id": "i3"}));

    let fetch = Fetch::new(DashboardRequest, db);
    assert_eq!(fetch.get().total_items, 3);
}

#[test]
fn fetch_is_not_loading_after_init() {
    let db = Arc::new(InMemoryDatabase::new());
    let fetch = Fetch::new(DashboardRequest, db);
    assert!(!fetch.is_loading());
}

#[test]
fn fetch_reader_works() {
    let db = Arc::new(InMemoryDatabase::new());
    db.insert("items", "i1", serde_json::json!({"id": "i1"}));

    let fetch = Fetch::new(DashboardRequest, db);
    let reader = fetch.reader();
    assert_eq!(reader.get().total_items, 1);
}
