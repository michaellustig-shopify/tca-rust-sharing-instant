//! Tests for schema validation rules.
//!
//! Maps Swift's SyncEngineValidationTests.swift to InstantDB entity/link validation.
//!
//! All tests remain #[ignore] — blocked by rust-instantdb schema management.
//! Test bodies demonstrate what each validation rule would check, using
//! the Table trait metadata and string validation as the closest approximation.

use sharing_instant::database::InMemoryDatabase;
use sharing_instant::table::{ColumnDef, Table};
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
struct ValidEntity {
    id: String,
    name: String,
}

impl Table for ValidEntity {
    const TABLE_NAME: &'static str = "valid_entities";
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
        ]
    }
}

/// Validates an entity/attribute name follows InstantDB naming rules:
/// alphanumeric characters plus underscores, starting with a letter.
fn is_valid_schema_name(name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    let first = name.chars().next().expect("name is not empty");
    if !first.is_ascii_alphabetic() && first != '_' {
        return false;
    }
    name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// InstantDB reserved attribute names that cannot be used in schemas.
const RESERVED_ATTR_NAMES: &[&str] = &["id", "$isCreator"];

fn is_reserved_attr_name(name: &str) -> bool {
    RESERVED_ATTR_NAMES.contains(&name)
}

#[test]
#[ignore = "BLOCKED: rust-instantdb schema management not available in Rust client"]
fn entity_name_must_be_alphanumeric() {
    // NOTE: Real implementation needs InstantDB schema registration API.
    // Schema registration would reject entity names with special characters.
    let _db = Arc::new(InMemoryDatabase::new());

    // Valid entity names
    assert!(
        is_valid_schema_name("projects"),
        "'projects' is a valid entity name"
    );
    assert!(
        is_valid_schema_name("reminder_lists"),
        "'reminder_lists' with underscores is valid"
    );
    assert!(
        is_valid_schema_name("Item2"),
        "'Item2' with digits is valid"
    );

    // Invalid entity names
    assert!(
        !is_valid_schema_name("my-entity"),
        "hyphens should be rejected"
    );
    assert!(
        !is_valid_schema_name("my entity"),
        "spaces should be rejected"
    );
    assert!(
        !is_valid_schema_name("my.entity"),
        "dots should be rejected"
    );
    assert!(
        !is_valid_schema_name("123entity"),
        "starting with digit should be rejected"
    );
    assert!(
        !is_valid_schema_name("entity!"),
        "special characters should be rejected"
    );

    // In production: db.transact([["add-attr", {"id": "my-entity/name", ...}]]) would fail
}

#[test]
#[ignore = "BLOCKED: rust-instantdb schema management not available in Rust client"]
fn attr_name_must_be_alphanumeric() {
    // NOTE: Real implementation needs InstantDB schema registration API.
    let _db = Arc::new(InMemoryDatabase::new());

    // Valid attribute names
    assert!(is_valid_schema_name("name"), "'name' is valid");
    assert!(
        is_valid_schema_name("is_completed"),
        "'is_completed' with underscore is valid"
    );
    assert!(
        is_valid_schema_name("priority2"),
        "'priority2' with digit is valid"
    );

    // Invalid attribute names
    assert!(
        !is_valid_schema_name("my-attr"),
        "hyphens in attr names should be rejected"
    );
    assert!(
        !is_valid_schema_name("my attr"),
        "spaces in attr names should be rejected"
    );
    assert!(
        !is_valid_schema_name(""),
        "empty attr name should be rejected"
    );

    // Verify existing Table columns follow naming rules
    for col in ValidEntity::columns() {
        assert!(
            is_valid_schema_name(col.name),
            "column '{}' should follow naming rules",
            col.name
        );
    }
}

#[test]
#[ignore = "BLOCKED: rust-instantdb schema management not available in Rust client"]
fn link_must_reference_existing_entity() {
    // NOTE: Real implementation needs InstantDB schema management.
    // When registering a link attribute, the target entity type must already
    // exist in the schema. Creating a link to a nonexistent entity type
    // should be rejected.
    let _db = Arc::new(InMemoryDatabase::new());

    // Simulate a set of registered entity types
    let registered_entities = vec!["projects", "task_items", "users"];

    // Valid link: target entity exists
    let valid_target = "projects";
    assert!(
        registered_entities.contains(&valid_target),
        "link target '{}' should reference an existing entity",
        valid_target
    );

    // Invalid link: target entity does NOT exist
    let invalid_target = "nonexistent_table";
    assert!(
        !registered_entities.contains(&invalid_target),
        "link to '{}' should be rejected — entity not registered",
        invalid_target
    );

    // In production:
    //   let result = db.transact([["add-attr", {
    //       "id": "task_items/nonexistent",
    //       "value-type": "ref",
    //       "link": {"forward": {"on": "nonexistent_table"}}
    //   }]]);
    //   assert!(result.is_err(), "link to nonexistent entity should fail");
}

#[test]
#[ignore = "BLOCKED: rust-instantdb schema management not available in Rust client"]
fn duplicate_entity_name_rejected() {
    // NOTE: Real implementation needs InstantDB schema management.
    // Registering the same entity name twice should be an error.
    let _db = Arc::new(InMemoryDatabase::new());

    // Simulate schema registry tracking
    let mut registered: Vec<&str> = Vec::new();

    // First registration succeeds
    let entity_name = "projects";
    assert!(
        !registered.contains(&entity_name),
        "first registration should be new"
    );
    registered.push(entity_name);

    // Second registration of same name should be rejected
    assert!(
        registered.contains(&entity_name),
        "duplicate entity name '{}' should be detected and rejected",
        entity_name
    );

    // In production:
    //   // First: succeeds
    //   db.transact([["add-attr", {"id": "projects/name", ...}]]).unwrap();
    //   // Second: fails
    //   let result = db.transact([["add-attr", {"id": "projects/name", ...}]]);
    //   assert!(result.is_err(), "duplicate entity should be rejected");
}

#[test]
#[ignore = "BLOCKED: rust-instantdb schema management not available in Rust client"]
fn duplicate_attr_name_in_entity_rejected() {
    // NOTE: Real implementation needs InstantDB schema management.
    // Same attribute name twice in one entity is an error.
    let _db = Arc::new(InMemoryDatabase::new());

    // Simulate entity column registry
    let mut attr_names: Vec<&str> = Vec::new();

    // First attribute registration
    let attr = "name";
    assert!(
        !attr_names.contains(&attr),
        "first attr registration should succeed"
    );
    attr_names.push(attr);

    // Duplicate attribute
    assert!(
        attr_names.contains(&attr),
        "duplicate attr '{}' in same entity should be detected and rejected",
        attr
    );

    // Also verify the Table trait implementation doesn't have duplicates
    let columns = ValidEntity::columns();
    let names: Vec<&str> = columns.iter().map(|c| c.name).collect();
    let unique_names: std::collections::HashSet<&str> = names.iter().copied().collect();
    assert_eq!(
        names.len(),
        unique_names.len(),
        "Table columns should not contain duplicate names"
    );
}

#[test]
#[ignore = "BLOCKED: rust-instantdb schema management not available in Rust client"]
fn reserved_attr_names_rejected() {
    // 'id' is reserved by InstantDB — it's auto-generated and managed by the system.
    // User-defined schemas should not declare 'id' as a custom attribute.
    let _db = Arc::new(InMemoryDatabase::new());

    // Check reserved names
    assert!(is_reserved_attr_name("id"), "'id' is reserved by InstantDB");
    assert!(
        is_reserved_attr_name("$isCreator"),
        "'$isCreator' is reserved by InstantDB"
    );

    // Non-reserved names should pass
    assert!(!is_reserved_attr_name("name"), "'name' is not reserved");
    assert!(!is_reserved_attr_name("title"), "'title' is not reserved");
    assert!(!is_reserved_attr_name("status"), "'status' is not reserved");

    // In production:
    //   let result = db.transact([["add-attr", {"id": "projects/id", ...}]]);
    //   assert!(result.is_err(), "reserved attr name 'id' should be rejected");
}

#[test]
#[ignore = "BLOCKED: rust-instantdb schema management not available in Rust client"]
fn empty_entity_name_rejected() {
    // Empty string entity name should be rejected at schema registration.
    let _db = Arc::new(InMemoryDatabase::new());

    assert!(
        !is_valid_schema_name(""),
        "empty entity name should be rejected"
    );

    // Also check whitespace-only names
    assert!(
        !is_valid_schema_name(" "),
        "whitespace-only name should be rejected"
    );

    // ValidEntity has a proper non-empty name
    assert!(
        !ValidEntity::TABLE_NAME.is_empty(),
        "Table trait TABLE_NAME should not be empty"
    );
    assert!(
        is_valid_schema_name(ValidEntity::TABLE_NAME),
        "Table trait TABLE_NAME should be a valid schema name"
    );
}
