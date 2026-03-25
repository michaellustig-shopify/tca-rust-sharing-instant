//! Tests for schema evolution.
//!
//! Maps Swift's SchemaChangeTests.swift to InstantDB attrs updates.
//!
//! Swift test mapping:
//! - add column → add new attribute
//! - rename column → add attr + migrate data
//! - old/new schema sync → schema evolution via attrs
//! - remove column → attr deprecation
//!
//! Tests that can be expressed against InMemoryDatabase + serde deserialization
//! are implemented. Tests requiring server-side schema management (add/remove
//! entity types, rename attrs, cardinality changes) remain ignored.

use sharing_instant::database::{Database, InMemoryDatabase};
use sharing_instant::table::{json_to_value, ColumnDef, Table};
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
struct TaskV1 {
    id: String,
    title: String,
}

impl Table for TaskV1 {
    const TABLE_NAME: &'static str = "tasks";
    fn columns() -> &'static [ColumnDef] {
        &[]
    }
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
struct TaskV2 {
    id: String,
    title: String,
    priority: Option<i32>,
    due_date: Option<f64>,
}

impl Table for TaskV2 {
    const TABLE_NAME: &'static str = "tasks";
    fn columns() -> &'static [ColumnDef] {
        &[]
    }
}

// === Adding attributes ===

#[test]
fn add_new_attribute_to_existing_entity() {
    // Insert V1 data (no priority/due_date), then read it back as V2.
    // The new Option fields should deserialize as None.
    let db = Arc::new(InMemoryDatabase::new());

    db.insert(
        "tasks",
        "t1",
        serde_json::json!({"id": "t1", "title": "Old task"}),
    );

    let query = json_to_value(&serde_json::json!({"tasks": {}}));
    let result = db.query(&query).expect("query should succeed");

    // Parse as V2 — missing fields become None
    let tasks = parse_all_v2(&result);
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0].id, "t1");
    assert_eq!(tasks[0].title, "Old task");
    assert_eq!(
        tasks[0].priority, None,
        "new attr should default to None for existing records"
    );
    assert_eq!(
        tasks[0].due_date, None,
        "new attr should default to None for existing records"
    );
}

#[test]
fn new_attribute_defaults_to_null() {
    // Explicitly verify that multiple existing records all get None
    // for newly added attributes.
    let db = Arc::new(InMemoryDatabase::new());

    db.insert(
        "tasks",
        "t1",
        serde_json::json!({"id": "t1", "title": "Task 1"}),
    );
    db.insert(
        "tasks",
        "t2",
        serde_json::json!({"id": "t2", "title": "Task 2"}),
    );
    db.insert(
        "tasks",
        "t3",
        serde_json::json!({"id": "t3", "title": "Task 3"}),
    );

    let query = json_to_value(&serde_json::json!({"tasks": {}}));
    let result = db.query(&query).expect("query should succeed");

    let tasks = parse_all_v2(&result);
    assert_eq!(tasks.len(), 3);

    for task in &tasks {
        assert_eq!(
            task.priority, None,
            "priority should be None for V1 data (task {})",
            task.id
        );
        assert_eq!(
            task.due_date, None,
            "due_date should be None for V1 data (task {})",
            task.id
        );
    }
}

#[test]
fn write_to_new_attribute() {
    // Insert data with the new V2 fields populated, then read it back.
    let db = Arc::new(InMemoryDatabase::new());

    db.insert(
        "tasks",
        "t1",
        serde_json::json!({"id": "t1", "title": "Priority task", "priority": 5, "due_date": 1700000000.0}),
    );

    let query = json_to_value(&serde_json::json!({"tasks": {}}));
    let result = db.query(&query).expect("query should succeed");

    let tasks = parse_all_v2(&result);
    assert_eq!(tasks.len(), 1);
    assert_eq!(
        tasks[0].priority,
        Some(5),
        "newly written priority should round-trip"
    );
    assert_eq!(
        tasks[0].due_date,
        Some(1700000000.0),
        "newly written due_date should round-trip"
    );
}

// === Schema version compatibility ===

#[test]
fn old_client_reads_new_schema() {
    // V2 data (with priority and due_date) is inserted.
    // A V1 client reads it — serde ignores unknown fields by default.
    let db = Arc::new(InMemoryDatabase::new());

    db.insert(
        "tasks",
        "t1",
        serde_json::json!({"id": "t1", "title": "V2 task", "priority": 3, "due_date": 1700000000.0}),
    );

    let query = json_to_value(&serde_json::json!({"tasks": {}}));
    let result = db.query(&query).expect("query should succeed");

    // Parse as V1 — priority and due_date should be silently ignored
    let tasks = parse_all_v1(&result);
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0].id, "t1");
    assert_eq!(
        tasks[0].title, "V2 task",
        "V1 client should read the known fields correctly"
    );
}

#[test]
fn new_client_reads_old_data() {
    // V1 data (no priority/due_date) is in the store.
    // A V2 client reads it — new Option fields are None.
    let db = Arc::new(InMemoryDatabase::new());

    db.insert(
        "tasks",
        "t1",
        serde_json::json!({"id": "t1", "title": "Legacy task"}),
    );

    let query = json_to_value(&serde_json::json!({"tasks": {}}));
    let result = db.query(&query).expect("query should succeed");

    let tasks = parse_all_v2(&result);
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0].id, "t1");
    assert_eq!(tasks[0].title, "Legacy task");
    assert_eq!(
        tasks[0].priority, None,
        "V2 client should see None for missing V1 fields"
    );
    assert_eq!(
        tasks[0].due_date, None,
        "V2 client should see None for missing V1 fields"
    );
}

#[test]
fn bidirectional_schema_sync() {
    // V1 writes a record. V2 reads it (new fields are None).
    // V2 writes the new attribute. V1 re-reads and ignores the extra field.
    let db = Arc::new(InMemoryDatabase::new());

    // Step 1: V1 client writes
    let tx_v1 = json_to_value(&serde_json::json!([
        ["create", "tasks", "t1", {"id": "t1", "title": "Shared task"}]
    ]));
    db.transact(&tx_v1).expect("V1 create should succeed");

    // Step 2: V2 client reads — new fields are None
    let query = json_to_value(&serde_json::json!({"tasks": {}}));
    let result = db.query(&query).expect("query should succeed");
    let tasks_v2 = parse_all_v2(&result);
    assert_eq!(tasks_v2.len(), 1);
    assert_eq!(
        tasks_v2[0].priority, None,
        "V2 should see None before writing priority"
    );

    // Step 3: V2 client writes the new field
    let tx_v2 = json_to_value(&serde_json::json!([
        ["update", "tasks", "t1", {"id": "t1", "title": "Shared task", "priority": 7, "due_date": 1700000000.0}]
    ]));
    db.transact(&tx_v2).expect("V2 update should succeed");

    // Step 4: V1 client re-reads — ignores unknown fields
    let result2 = db.query(&query).expect("second query should succeed");
    let tasks_v1 = parse_all_v1(&result2);
    assert_eq!(tasks_v1.len(), 1);
    assert_eq!(
        tasks_v1[0].title, "Shared task",
        "V1 should still read its known fields"
    );

    // Step 5: V2 client reads again — sees the priority it wrote
    let tasks_v2_again = parse_all_v2(&result2);
    assert_eq!(
        tasks_v2_again[0].priority,
        Some(7),
        "V2 should see the priority it wrote"
    );
    assert_eq!(
        tasks_v2_again[0].due_date,
        Some(1700000000.0),
        "V2 should see the due_date it wrote"
    );
}

// === Attribute removal / deprecation ===

#[test]
#[ignore = "Requires server-side schema management not available in InMemoryDatabase"]
fn remove_attribute_from_schema() {
    let _db = Arc::new(InMemoryDatabase::new());
    todo!("Remove attr from schema, existing data remains but not queryable")
}

#[test]
#[ignore = "Requires server-side schema management not available in InMemoryDatabase"]
fn rename_attribute_via_migration() {
    let _db = Arc::new(InMemoryDatabase::new());
    todo!("Rename attr: add new, copy data, remove old")
}

// === Entity type management ===

#[test]
#[ignore = "Requires server-side schema management not available in InMemoryDatabase"]
fn add_new_entity_type() {
    let _db = Arc::new(InMemoryDatabase::new());
    todo!("Register new entity type in schema, start querying it")
}

#[test]
#[ignore = "Requires server-side schema management not available in InMemoryDatabase"]
fn remove_entity_type() {
    let _db = Arc::new(InMemoryDatabase::new());
    todo!("Remove entity type from schema")
}

#[test]
#[ignore = "Requires server-side cardinality control not available in InMemoryDatabase"]
fn change_attribute_cardinality() {
    let _db = Arc::new(InMemoryDatabase::new());
    todo!("Change attr cardinality, verify queries adapt")
}

#[test]
#[ignore = "Requires server-side index hints not available in InMemoryDatabase"]
fn index_attribute_for_query_performance() {
    let _db = Arc::new(InMemoryDatabase::new());
    todo!("Add index hint to attr, verify query still works")
}

#[test]
#[ignore = "Requires server-side schema validation not available in InMemoryDatabase"]
fn schema_validation_rejects_invalid_entity_name() {
    let _db = Arc::new(InMemoryDatabase::new());
    todo!("Entity name with special chars is rejected")
}

// === Helpers ===

fn parse_all_v1(result: &sharing_instant::Value) -> Vec<TaskV1> {
    match result {
        sharing_instant::Value::Object(obj) => match obj.get("tasks") {
            Some(sharing_instant::Value::Array(arr)) => arr
                .iter()
                .map(|v| TaskV1::from_value(v).expect("should deserialize TaskV1"))
                .collect(),
            other => panic!("expected tasks Array, got: {other:?}"),
        },
        other => panic!("expected Object result, got: {other:?}"),
    }
}

fn parse_all_v2(result: &sharing_instant::Value) -> Vec<TaskV2> {
    match result {
        sharing_instant::Value::Object(obj) => match obj.get("tasks") {
            Some(sharing_instant::Value::Array(arr)) => arr
                .iter()
                .map(|v| TaskV2::from_value(v).expect("should deserialize TaskV2"))
                .collect(),
            other => panic!("expected tasks Array, got: {other:?}"),
        },
        other => panic!("expected Object result, got: {other:?}"),
    }
}
