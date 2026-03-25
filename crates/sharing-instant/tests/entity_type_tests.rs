//! Tests for entity type registration and schema setup.
//!
//! Maps Swift's RecordTypeTests.swift to InstantDB entity schema registration.
//!
//! All tests remain #[ignore] — blocked by rust-instantdb schema management.
//! Test bodies show what the test WOULD do using the best approximation
//! available with InMemoryDatabase and the Table trait.

use sharing_instant::database::{Database, InMemoryDatabase};
use sharing_instant::fetch_all::FetchAll;
use sharing_instant::table::{ColumnDef, Table};
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
struct Project {
    id: String,
    name: String,
    status: String,
}

impl Table for Project {
    const TABLE_NAME: &'static str = "projects";
    fn columns() -> &'static [ColumnDef] {
        &[
            ColumnDef {
                name: "id",
                rust_type: "String",
                value_type: "string",
                is_optional: false,
                is_primary_key: true,
                is_unique: true,
                is_indexed: true,
            },
            ColumnDef {
                name: "name",
                rust_type: "String",
                value_type: "string",
                is_optional: false,
                is_primary_key: false,
                is_unique: false,
                is_indexed: false,
            },
            ColumnDef {
                name: "status",
                rust_type: "String",
                value_type: "string",
                is_optional: false,
                is_primary_key: false,
                is_unique: false,
                is_indexed: false,
            },
        ]
    }
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
struct TaskItem {
    id: String,
    title: String,
    project_id: Option<String>,
}

impl Table for TaskItem {
    const TABLE_NAME: &'static str = "task_items";
    fn columns() -> &'static [ColumnDef] {
        &[
            ColumnDef {
                name: "id",
                rust_type: "String",
                value_type: "string",
                is_optional: false,
                is_primary_key: true,
                is_unique: true,
                is_indexed: true,
            },
            ColumnDef {
                name: "title",
                rust_type: "String",
                value_type: "string",
                is_optional: false,
                is_primary_key: false,
                is_unique: false,
                is_indexed: false,
            },
            ColumnDef {
                name: "project_id",
                rust_type: "Option<String>",
                value_type: "string",
                is_optional: true,
                is_primary_key: false,
                is_unique: false,
                is_indexed: false,
            },
        ]
    }
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
struct User {
    id: String,
    email: String,
}

impl Table for User {
    const TABLE_NAME: &'static str = "users";
    fn columns() -> &'static [ColumnDef] {
        &[
            ColumnDef {
                name: "id",
                rust_type: "String",
                value_type: "string",
                is_optional: false,
                is_primary_key: true,
                is_unique: true,
                is_indexed: true,
            },
            ColumnDef {
                name: "email",
                rust_type: "String",
                value_type: "string",
                is_optional: false,
                is_primary_key: false,
                is_unique: true,
                is_indexed: true,
            },
        ]
    }
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
struct Metric {
    id: String,
    label: String,
    value: f64,
    is_active: bool,
    recorded_at: String,
}

impl Table for Metric {
    const TABLE_NAME: &'static str = "metrics";
    fn columns() -> &'static [ColumnDef] {
        &[
            ColumnDef {
                name: "label",
                rust_type: "String",
                value_type: "string",
                is_optional: false,
                is_primary_key: false,
                is_unique: false,
                is_indexed: false,
            },
            ColumnDef {
                name: "value",
                rust_type: "f64",
                value_type: "number",
                is_optional: false,
                is_primary_key: false,
                is_unique: false,
                is_indexed: false,
            },
            ColumnDef {
                name: "is_active",
                rust_type: "bool",
                value_type: "boolean",
                is_optional: false,
                is_primary_key: false,
                is_unique: false,
                is_indexed: false,
            },
            ColumnDef {
                name: "recorded_at",
                rust_type: "String",
                value_type: "date",
                is_optional: false,
                is_primary_key: false,
                is_unique: false,
                is_indexed: false,
            },
        ]
    }
}

#[test]
#[ignore = "BLOCKED: rust-instantdb schema management not available in Rust client"]
fn register_entity_type_with_attrs() {
    // NOTE: Real implementation needs InstantDB schema management API.
    // In production, you'd register the schema before inserting data:
    //   db.transact(&json_to_value(&serde_json::json!([
    //       ["add-attr", {"id": "projects/name", "value-type": "blob", "cardinality": "one"}],
    //       ["add-attr", {"id": "projects/status", "value-type": "blob", "cardinality": "one"}]
    //   ])))
    //
    // We approximate by verifying the Table trait metadata matches expectations.
    let _db = Arc::new(InMemoryDatabase::new());

    // Verify entity type metadata from Table trait
    assert_eq!(Project::TABLE_NAME, "projects");

    let columns = Project::columns();
    assert_eq!(
        columns.len(),
        3,
        "projects should have 3 columns: id, name, status"
    );

    let name_col = columns.iter().find(|c| c.name == "name");
    assert!(name_col.is_some(), "should have a 'name' column");
    assert_eq!(name_col.expect("name column exists").value_type, "string");

    let status_col = columns.iter().find(|c| c.name == "status");
    assert!(status_col.is_some(), "should have a 'status' column");
    assert_eq!(
        status_col.expect("status column exists").value_type,
        "string"
    );

    // Verify data can be inserted and retrieved with this schema
    let db = Arc::new(InMemoryDatabase::new());
    db.insert(
        Project::TABLE_NAME,
        "p1",
        serde_json::json!({"id": "p1", "name": "Alpha", "status": "active"}),
    );

    let fetch = FetchAll::<Project>::new(db);
    let projects = fetch.get();
    assert_eq!(projects.len(), 1);
    assert_eq!(projects[0].name, "Alpha");
    assert_eq!(projects[0].status, "active");
}

#[test]
#[ignore = "BLOCKED: rust-instantdb schema management not available in Rust client"]
fn query_registered_entity_type() {
    // NOTE: Real implementation needs InstantDB schema registration + InstaQL.
    // After registration, queries against the entity type should return data
    // with correct types.
    let db = Arc::new(InMemoryDatabase::new());

    db.insert(
        "projects",
        "p1",
        serde_json::json!({"id": "p1", "name": "Alpha", "status": "active"}),
    );
    db.insert(
        "projects",
        "p2",
        serde_json::json!({"id": "p2", "name": "Beta", "status": "completed"}),
    );

    // Query via the Table's query builder
    let query = Project::query().build();
    let result = db.query(&query).expect("query should succeed");

    // Verify the result structure matches the registered schema
    match &result {
        sharing_instant::Value::Object(obj) => {
            let projects = obj.get("projects").expect("should have projects key");
            match projects {
                sharing_instant::Value::Array(arr) => {
                    assert_eq!(arr.len(), 2, "should return 2 projects");
                }
                other => panic!("expected Array, got {other:?}"),
            }
        }
        other => panic!("expected Object, got {other:?}"),
    }

    // Verify deserialization to typed struct
    let fetch = FetchAll::<Project>::new(db);
    let projects = fetch.get();
    assert_eq!(projects.len(), 2);
    assert!(projects.iter().any(|p| p.name == "Alpha"));
    assert!(projects.iter().any(|p| p.name == "Beta"));
}

#[test]
#[ignore = "BLOCKED: rust-instantdb schema management not available in Rust client"]
fn register_multiple_entity_types() {
    // NOTE: Real implementation needs batch schema registration.
    // In production: register all entity types before the app starts.
    let db = Arc::new(InMemoryDatabase::new());

    // Insert data for three different entity types
    db.insert(
        "projects",
        "p1",
        serde_json::json!({"id": "p1", "name": "Project A", "status": "active"}),
    );
    db.insert(
        "task_items",
        "t1",
        serde_json::json!({"id": "t1", "title": "Task 1", "project_id": "p1"}),
    );
    db.insert(
        "users",
        "u1",
        serde_json::json!({"id": "u1", "email": "alice@example.com"}),
    );

    // All three entity types should be queryable independently
    let projects = FetchAll::<Project>::new(db.clone()).get();
    let tasks = FetchAll::<TaskItem>::new(db.clone()).get();
    let users = FetchAll::<User>::new(db).get();

    assert_eq!(projects.len(), 1);
    assert_eq!(tasks.len(), 1);
    assert_eq!(users.len(), 1);

    // Verify each has correct table name mapping
    assert_eq!(Project::TABLE_NAME, "projects");
    assert_eq!(TaskItem::TABLE_NAME, "task_items");
    assert_eq!(User::TABLE_NAME, "users");
}

#[test]
#[ignore = "BLOCKED: rust-instantdb schema management not available in Rust client"]
fn entity_type_with_link_attrs() {
    // NOTE: Real implementation needs InstantDB link attribute registration.
    // Link attrs create first-class relationships between entity types:
    //   {"id": "task_items/project", "value-type": "ref", "cardinality": "one"}
    let db = Arc::new(InMemoryDatabase::new());

    db.insert(
        "projects",
        "p1",
        serde_json::json!({"id": "p1", "name": "Alpha", "status": "active"}),
    );
    db.insert(
        "task_items",
        "t1",
        serde_json::json!({"id": "t1", "title": "Task 1", "project_id": "p1"}),
    );

    // Verify the link attribute is defined in the Table metadata
    let task_columns = TaskItem::columns();
    let link_col = task_columns.iter().find(|c| c.name == "project_id");
    assert!(link_col.is_some(), "should have project_id link attribute");
    assert!(
        link_col.expect("link column exists").is_optional,
        "project_id should be optional (nullable FK)"
    );

    // Verify the field-level reference works
    let tasks = FetchAll::<TaskItem>::new(db.clone()).get();
    assert_eq!(tasks[0].project_id.as_deref(), Some("p1"));

    // Resolve the link
    let projects = FetchAll::<Project>::new(db).get();
    let linked_project = projects
        .iter()
        .find(|p| Some(p.id.as_str()) == tasks[0].project_id.as_deref());
    assert!(
        linked_project.is_some(),
        "link should resolve to existing project"
    );
    assert_eq!(linked_project.expect("project exists").name, "Alpha");
}

#[test]
#[ignore = "BLOCKED: rust-instantdb schema management not available in Rust client"]
fn entity_type_attr_types() {
    // NOTE: Real implementation needs InstantDB typed attribute registration.
    // InstantDB supports: blob (string), number, boolean, date, ref (link).
    let db = Arc::new(InMemoryDatabase::new());

    // Verify column type metadata
    let columns = Metric::columns();

    let label_col = columns.iter().find(|c| c.name == "label");
    assert_eq!(
        label_col.expect("label exists").value_type,
        "string",
        "label should be string type"
    );

    let value_col = columns.iter().find(|c| c.name == "value");
    assert_eq!(
        value_col.expect("value exists").value_type,
        "number",
        "value should be number type"
    );

    let active_col = columns.iter().find(|c| c.name == "is_active");
    assert_eq!(
        active_col.expect("is_active exists").value_type,
        "boolean",
        "is_active should be boolean type"
    );

    let date_col = columns.iter().find(|c| c.name == "recorded_at");
    assert_eq!(
        date_col.expect("recorded_at exists").value_type,
        "date",
        "recorded_at should be date type"
    );

    // Verify data round-trips correctly with all types
    db.insert(
        "metrics",
        "m1",
        serde_json::json!({
            "id": "m1",
            "label": "CPU Usage",
            "value": 87.5,
            "is_active": true,
            "recorded_at": "2025-01-15T10:30:00Z"
        }),
    );

    let fetch = FetchAll::<Metric>::new(db);
    let metrics = fetch.get();
    assert_eq!(metrics.len(), 1);
    assert_eq!(metrics[0].label, "CPU Usage");
    assert!(
        (metrics[0].value - 87.5).abs() < f64::EPSILON,
        "number should round-trip"
    );
    assert!(metrics[0].is_active, "boolean should round-trip");
    assert_eq!(
        metrics[0].recorded_at, "2025-01-15T10:30:00Z",
        "date string should round-trip"
    );
}
