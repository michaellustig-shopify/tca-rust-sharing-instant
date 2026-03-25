//! Tests for Table trait, query builder, and Value conversion.
//!
//! Mirrors tests from Swift's StructuredQueries test suite.

use sharing_instant::table::{json_to_value, value_to_json, ColumnDef, QueryBuilder, Table};
use sharing_instant::Value;

// -- Test schema --

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
struct Reminder {
    id: String,
    title: String,
    is_completed: bool,
    priority: Option<i64>,
    reminders_list_id: String,
}

impl Table for Reminder {
    const TABLE_NAME: &'static str = "reminders";
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
                name: "isCompleted",
                rust_type: "bool",
                value_type: "boolean",
                is_optional: false,
                is_primary_key: false,
                is_unique: false,
                is_indexed: false,
            },
            ColumnDef {
                name: "priority",
                rust_type: "Option<i64>",
                value_type: "number",
                is_optional: true,
                is_primary_key: false,
                is_unique: false,
                is_indexed: false,
            },
            ColumnDef {
                name: "remindersListId",
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

// -- Table trait tests --

#[test]
fn table_name() {
    assert_eq!(Reminder::TABLE_NAME, "reminders");
}

#[test]
fn table_columns_count() {
    assert_eq!(Reminder::columns().len(), 5);
}

#[test]
fn table_primary_key_column() {
    let pk = Reminder::columns()
        .iter()
        .find(|c| c.is_primary_key)
        .expect("should have a primary key");
    assert_eq!(pk.name, "id");
}

#[test]
fn table_optional_column() {
    let opt = Reminder::columns()
        .iter()
        .find(|c| c.is_optional)
        .expect("should have an optional column");
    assert_eq!(opt.name, "priority");
}

// -- Value conversion tests --

#[test]
fn json_to_value_null() {
    let json = serde_json::json!(null);
    let value = json_to_value(&json);
    assert!(matches!(value, Value::Null));
}

#[test]
fn json_to_value_bool() {
    let json = serde_json::json!(true);
    let value = json_to_value(&json);
    assert!(matches!(value, Value::Bool(true)));
}

#[test]
fn json_to_value_int() {
    let json = serde_json::json!(42);
    let value = json_to_value(&json);
    assert!(matches!(value, Value::Int(42)));
}

#[test]
fn json_to_value_float() {
    let json = serde_json::json!(3.14);
    let value = json_to_value(&json);
    match value {
        Value::Float(f) => assert!((f.0 - 3.14).abs() < f64::EPSILON),
        _ => panic!("expected Float"),
    }
}

#[test]
fn json_to_value_string() {
    let json = serde_json::json!("hello");
    let value = json_to_value(&json);
    assert!(matches!(value, Value::String(ref s) if s == "hello"));
}

#[test]
fn json_to_value_array() {
    let json = serde_json::json!([1, 2, 3]);
    let value = json_to_value(&json);
    match value {
        Value::Array(arr) => assert_eq!(arr.len(), 3),
        _ => panic!("expected Array"),
    }
}

#[test]
fn json_to_value_object() {
    let json = serde_json::json!({"name": "Alice", "age": 30});
    let value = json_to_value(&json);
    match value {
        Value::Object(obj) => {
            assert_eq!(obj.len(), 2);
            assert!(obj.contains_key("name"));
            assert!(obj.contains_key("age"));
        }
        _ => panic!("expected Object"),
    }
}

#[test]
fn value_roundtrip() {
    let json = serde_json::json!({
        "id": "r1",
        "title": "Buy milk",
        "is_completed": false,
        "priority": null,
        "reminders_list_id": "list1"
    });

    let value = json_to_value(&json);
    let back = value_to_json(&value);

    assert_eq!(json, back);
}

// -- Table serialization tests --

#[test]
fn table_to_value() {
    let reminder = Reminder {
        id: "r1".to_string(),
        title: "Buy milk".to_string(),
        is_completed: false,
        priority: None,
        reminders_list_id: "list1".to_string(),
    };

    let value = reminder.to_value().expect("should serialize");
    match &value {
        Value::Object(obj) => {
            assert!(matches!(obj.get("id"), Some(Value::String(s)) if s == "r1"));
            assert!(matches!(obj.get("title"), Some(Value::String(s)) if s == "Buy milk"));
            assert!(matches!(obj.get("is_completed"), Some(Value::Bool(false))));
            assert!(matches!(obj.get("priority"), Some(Value::Null)));
        }
        _ => panic!("expected Object"),
    }
}

#[test]
fn table_from_value() {
    let value = json_to_value(&serde_json::json!({
        "id": "r1",
        "title": "Buy milk",
        "is_completed": false,
        "priority": null,
        "reminders_list_id": "list1"
    }));

    let reminder = Reminder::from_value(&value).expect("should deserialize");
    assert_eq!(reminder.id, "r1");
    assert_eq!(reminder.title, "Buy milk");
    assert!(!reminder.is_completed);
    assert_eq!(reminder.priority, None);
}

#[test]
fn table_roundtrip() {
    let original = Reminder {
        id: "r1".to_string(),
        title: "Buy milk".to_string(),
        is_completed: true,
        priority: Some(3),
        reminders_list_id: "list1".to_string(),
    };

    let value = original.to_value().expect("should serialize");
    let restored = Reminder::from_value(&value).expect("should deserialize");
    assert_eq!(original, restored);
}

// -- Query builder tests --

#[test]
fn query_builder_empty() {
    let query = Reminder::query().build();
    match &query {
        Value::Object(obj) => {
            assert!(obj.contains_key("reminders"));
        }
        _ => panic!("expected Object"),
    }
}

#[test]
fn query_builder_where_eq() {
    let query = Reminder::query()
        .where_eq("is_completed", Value::Bool(false))
        .build();

    let json = value_to_json(&query);
    let where_clause = &json["reminders"]["$"]["where"]["is_completed"];
    assert_eq!(where_clause, &serde_json::json!(false));
}

#[test]
fn query_builder_where_gt() {
    let query = Reminder::query()
        .where_gt("priority", Value::Int(3))
        .build();

    let json = value_to_json(&query);
    let where_clause = &json["reminders"]["$"]["where"]["priority"]["$gt"];
    assert_eq!(where_clause, &serde_json::json!(3));
}

#[test]
fn query_builder_order() {
    let query = Reminder::query().order("title", "asc").build();

    let json = value_to_json(&query);
    let order = &json["reminders"]["$"]["order"];
    assert_eq!(order["field"], "title");
    assert_eq!(order["direction"], "asc");
}

#[test]
fn query_builder_limit() {
    let query = Reminder::query().limit(10).build();

    let json = value_to_json(&query);
    assert_eq!(json["reminders"]["$"]["limit"], 10);
}

#[test]
fn query_builder_combined() {
    let query = Reminder::query()
        .where_eq("is_completed", Value::Bool(false))
        .order("priority", "desc")
        .limit(5)
        .offset(10)
        .build();

    let json = value_to_json(&query);
    assert_eq!(json["reminders"]["$"]["where"]["is_completed"], false);
    assert_eq!(json["reminders"]["$"]["order"]["field"], "priority");
    assert_eq!(json["reminders"]["$"]["order"]["direction"], "desc");
    assert_eq!(json["reminders"]["$"]["limit"], 5);
    assert_eq!(json["reminders"]["$"]["offset"], 10);
}

#[test]
fn query_builder_where_in() {
    let query = Reminder::query()
        .where_in(
            "priority",
            vec![Value::Int(1), Value::Int(2), Value::Int(3)],
        )
        .build();

    let json = value_to_json(&query);
    let in_clause = &json["reminders"]["$"]["where"]["priority"]["$in"];
    assert_eq!(in_clause, &serde_json::json!([1, 2, 3]));
}

#[test]
fn query_builder_where_is_null() {
    let query = Reminder::query().where_is_null("priority", true).build();

    let json = value_to_json(&query);
    let null_clause = &json["reminders"]["$"]["where"]["priority"]["$isNull"];
    assert_eq!(null_clause, &serde_json::json!(true));
}
