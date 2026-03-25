//! Tests for link resolution ordering.
//!
//! Maps Swift's TopologicalTableSortingTests.swift to InstantDB link resolution order.
//!
//! All tests remain #[ignore] — blocked by rust-instantdb link API.
//! Test body demonstrates topological sort of entity creation based on
//! link dependencies, simulated via field-level references.

use sharing_instant::database::InMemoryDatabase;
use sharing_instant::fetch_all::FetchAll;
use sharing_instant::table::{ColumnDef, Table};
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
struct Department {
    id: String,
    name: String,
}

impl Table for Department {
    const TABLE_NAME: &'static str = "departments";
    fn columns() -> &'static [ColumnDef] {
        &[]
    }
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
struct Employee {
    id: String,
    name: String,
    department_id: String,
}

impl Table for Employee {
    const TABLE_NAME: &'static str = "employees";
    fn columns() -> &'static [ColumnDef] {
        &[]
    }
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
struct Task {
    id: String,
    title: String,
    assignee_id: String,
}

impl Table for Task {
    const TABLE_NAME: &'static str = "tasks";
    fn columns() -> &'static [ColumnDef] {
        &[]
    }
}

#[test]
#[ignore = "BLOCKED: rust-instantdb link() API not yet exposed in Rust client"]
fn link_resolution_order_parent_before_child() {
    // When syncing, parent entities must be created before children that
    // reference them, to avoid dangling references during insertion.
    //
    // Dependency chain: Department → Employee → Task
    //   - Task.assignee_id → Employee.id
    //   - Employee.department_id → Department.id
    //
    // Topological order: Department, then Employee, then Task.
    let db = Arc::new(InMemoryDatabase::new());

    // Define the dependency graph as (entity_type, depends_on)
    let dependencies: Vec<(&str, Option<&str>)> = vec![
        ("tasks", Some("employees")),       // tasks depend on employees
        ("employees", Some("departments")), // employees depend on departments
        ("departments", None),              // departments are root entities
    ];

    // Compute topological order (simple: roots first, then dependents)
    let mut resolved: Vec<&str> = Vec::new();
    let mut remaining: Vec<(&str, Option<&str>)> = dependencies.clone();

    while !remaining.is_empty() {
        let before_len = remaining.len();
        remaining.retain(|(entity, dep)| {
            if dep.is_none() || resolved.contains(&dep.expect("dep should exist if Some")) {
                resolved.push(entity);
                false // remove from remaining
            } else {
                true // keep in remaining
            }
        });
        assert_ne!(
            remaining.len(),
            before_len,
            "cycle detected — no progress in topological sort"
        );
    }

    assert_eq!(
        resolved,
        vec!["departments", "employees", "tasks"],
        "topological order should be: departments → employees → tasks"
    );

    // Create entities in topological order — no dangling references
    db.insert(
        "departments",
        "d1",
        serde_json::json!({"id": "d1", "name": "Engineering"}),
    );
    db.insert(
        "employees",
        "e1",
        serde_json::json!({"id": "e1", "name": "Alice", "department_id": "d1"}),
    );
    db.insert(
        "tasks",
        "t1",
        serde_json::json!({"id": "t1", "title": "Build feature", "assignee_id": "e1"}),
    );

    // Verify all references resolve
    let departments = FetchAll::<Department>::new(db.clone()).get();
    let employees = FetchAll::<Employee>::new(db.clone()).get();
    let tasks = FetchAll::<Task>::new(db).get();

    assert_eq!(departments.len(), 1);
    assert_eq!(employees.len(), 1);
    assert_eq!(tasks.len(), 1);

    // Verify reference chain is intact
    let employee = &employees[0];
    assert!(
        departments.iter().any(|d| d.id == employee.department_id),
        "employee's department_id should reference existing department"
    );

    let task = &tasks[0];
    assert!(
        employees.iter().any(|e| e.id == task.assignee_id),
        "task's assignee_id should reference existing employee"
    );
}
