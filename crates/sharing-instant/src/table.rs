//! ┌─────────────────────────────────────────────────────┐
//! │  TABLE TRAIT                                         │
//! │  Maps Rust structs to InstantDB entities              │
//! ├─────────────────────────────────────────────────────┤
//! │                                                      │
//! │  #[derive(Table)]                                    │
//! │  struct Reminder {                                   │
//! │      id: String,         ──► entity_id               │
//! │      title: String,      ──► attr "title"            │
//! │      is_completed: bool, ──► attr "isCompleted"      │
//! │  }                                                   │
//! │         │                                            │
//! │         ▼                                            │
//! │  InstantDB EAV Triple:                               │
//! │  [entity_id, attr_id, value, timestamp]              │
//! │                                                      │
//! ├─────────────────────────────────────────────────────┤
//! │  WHY: Mirrors Swift's @Table macro. Provides         │
//! │  compile-time type safety for database operations    │
//! │  without runtime reflection.                         │
//! │                                                      │
//! │  ALTERNATIVES: Manual InstantEntity impls (verbose), │
//! │  serde-based generic mapping (loses type safety).    │
//! │                                                      │
//! │  TESTED BY: tests/table_tests.rs                     │
//! │  EDGE CASES: optional fields, custom names,          │
//! │  nested types, ID generation                         │
//! │                                                      │
//! │  CHANGELOG:                                          │
//! │  • v0.1.0 — Initial Table trait + ColumnDef           │
//! │                                                      │
//! │  HISTORY: git log --oneline --follow -- crates/sharing-instant/src/table.rs │
//! └─────────────────────────────────────────────────────┘

use instant_core::value::Value;
use serde::{de::DeserializeOwned, Serialize};
use std::fmt::Debug;

/// Metadata for a single column in a table.
///
/// Describes how a Rust struct field maps to an InstantDB attribute.
///
/// # Example
///
/// ```
/// use sharing_instant::table::ColumnDef;
///
/// let col = ColumnDef {
///     name: "title",
///     rust_type: "String",
///     value_type: "string",
///     is_optional: false,
///     is_primary_key: false,
///     is_unique: false,
///     is_indexed: false,
/// };
/// assert_eq!(col.name, "title");
/// ```
#[derive(Debug, Clone)]
pub struct ColumnDef {
    /// The attribute name in InstantDB (camelCase).
    pub name: &'static str,
    /// The Rust type name for documentation purposes.
    pub rust_type: &'static str,
    /// The InstantDB value type: "string", "number", "boolean", "date", "json", "any".
    pub value_type: &'static str,
    /// Whether this field is `Option<T>`.
    pub is_optional: bool,
    /// Whether this is the primary key / entity ID field.
    pub is_primary_key: bool,
    /// Whether this attribute has a uniqueness constraint.
    pub is_unique: bool,
    /// Whether this attribute is indexed for fast lookups.
    pub is_indexed: bool,
}

/// Core trait for types that map to InstantDB entities.
///
/// Analogous to Swift's `@Table` macro from swift-structured-queries,
/// combined with InstantDB's `InstantEntity` trait. Provides:
/// - Entity/collection name mapping
/// - Column metadata for schema validation
/// - Serialization to/from InstantDB `Value`
/// - Query builder entry point
///
/// # Derive Macro
///
/// Use `#[derive(Table)]` to auto-generate this implementation:
///
/// ```ignore
/// #[derive(Table, Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
/// struct Reminder {
///     id: String,
///     title: String,
///     is_completed: bool,
///     priority: Option<i64>,
/// }
/// ```
///
/// # Manual Implementation
///
/// ```
/// use sharing_instant::table::{Table, ColumnDef};
/// use sharing_instant::Value;
///
/// #[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
/// struct Tag {
///     id: String,
///     name: String,
/// }
///
/// impl Table for Tag {
///     const TABLE_NAME: &'static str = "tags";
///
///     fn columns() -> &'static [ColumnDef] {
///         &[
///             ColumnDef {
///                 name: "id",
///                 rust_type: "String",
///                 value_type: "string",
///                 is_optional: false,
///                 is_primary_key: true,
///                 is_unique: true,
///                 is_indexed: true,
///             },
///             ColumnDef {
///                 name: "name",
///                 rust_type: "String",
///                 value_type: "string",
///                 is_optional: false,
///                 is_primary_key: false,
///                 is_unique: false,
///                 is_indexed: false,
///             },
///         ]
///     }
/// }
///
/// assert_eq!(Tag::TABLE_NAME, "tags");
/// assert_eq!(Tag::columns().len(), 2);
/// ```
pub trait Table: Debug + Clone + Send + Sync + Serialize + DeserializeOwned + 'static {
    /// The InstantDB entity/collection name (e.g., "reminders").
    ///
    /// By convention, this is the pluralized snake_case of the struct name.
    const TABLE_NAME: &'static str;

    /// Column definitions describing the struct's fields.
    fn columns() -> &'static [ColumnDef];

    /// Convert this value to an InstantDB `Value` for storage.
    ///
    /// Default implementation uses serde_json as intermediate format.
    fn to_value(&self) -> std::result::Result<Value, String> {
        let json = serde_json::to_value(self).map_err(|e| e.to_string())?;
        Ok(json_to_value(&json))
    }

    /// Construct this type from an InstantDB `Value`.
    ///
    /// Default implementation uses serde_json as intermediate format.
    fn from_value(value: &Value) -> std::result::Result<Self, String> {
        let json = value_to_json(value);
        serde_json::from_value(json).map_err(|e| e.to_string())
    }

    /// Create a query builder for this table.
    fn query() -> QueryBuilder<Self> {
        QueryBuilder::new()
    }
}

/// Convert a `serde_json::Value` to an InstantDB `Value`.
///
/// # Example
///
/// ```
/// use sharing_instant::table::json_to_value;
/// use sharing_instant::Value;
///
/// let json = serde_json::json!({"name": "Alice", "age": 30});
/// let value = json_to_value(&json);
/// // Value is now an InstantDB Object
/// ```
pub fn json_to_value(json: &serde_json::Value) -> Value {
    match json {
        serde_json::Value::Null => Value::Null,
        serde_json::Value::Bool(b) => Value::Bool(*b),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Value::Int(i)
            } else if let Some(f) = n.as_f64() {
                Value::Float(instant_core::value::OrderedFloat(f))
            } else {
                Value::Null
            }
        }
        serde_json::Value::String(s) => Value::String(s.clone()),
        serde_json::Value::Array(arr) => Value::Array(arr.iter().map(json_to_value).collect()),
        serde_json::Value::Object(obj) => {
            let map = obj
                .iter()
                .map(|(k, v)| (k.clone(), json_to_value(v)))
                .collect();
            Value::Object(map)
        }
    }
}

/// Convert an InstantDB `Value` to a `serde_json::Value`.
///
/// # Example
///
/// ```
/// use sharing_instant::table::value_to_json;
/// use sharing_instant::Value;
///
/// let value = Value::String("hello".to_string());
/// let json = value_to_json(&value);
/// assert_eq!(json, serde_json::json!("hello"));
/// ```
pub fn value_to_json(value: &Value) -> serde_json::Value {
    match value {
        Value::Null => serde_json::Value::Null,
        Value::Bool(b) => serde_json::Value::Bool(*b),
        Value::Int(i) => serde_json::json!(*i),
        Value::Float(f) => serde_json::json!(f.0),
        Value::String(s) => serde_json::Value::String(s.clone()),
        Value::Array(arr) => serde_json::Value::Array(arr.iter().map(value_to_json).collect()),
        Value::Object(obj) => {
            let map: serde_json::Map<String, serde_json::Value> = obj
                .iter()
                .map(|(k, v)| (k.clone(), value_to_json(v)))
                .collect();
            serde_json::Value::Object(map)
        }
    }
}

/// Fluent query builder for `Table` types.
///
/// Mirrors Swift's structured query DSL:
/// ```ignore
/// Reminder::query()
///     .where_eq("is_completed", false)
///     .where_gt("priority", 3)
///     .order("title", "asc")
///     .limit(10)
///     .build()
/// ```
///
/// Translates to InstantDB's InstaQL format under the hood.
#[derive(Debug, Clone)]
pub struct QueryBuilder<T: Table> {
    wheres: Vec<(String, WhereClause)>,
    order_key: Option<String>,
    order_dir: Option<String>,
    limit: Option<usize>,
    offset: Option<usize>,
    _phantom: std::marker::PhantomData<T>,
}

/// A single filter condition in a query.
///
/// # Example
///
/// ```
/// use sharing_instant::table::WhereClause;
/// use sharing_instant::Value;
///
/// let clause = WhereClause::Eq(Value::Bool(true));
/// assert!(matches!(clause, WhereClause::Eq(_)));
/// ```
#[derive(Debug, Clone)]
pub enum WhereClause {
    /// Exact equality match.
    Eq(Value),
    /// Greater than comparison.
    Gt(Value),
    /// Less than comparison.
    Lt(Value),
    /// Greater than or equal comparison.
    Gte(Value),
    /// Less than or equal comparison.
    Lte(Value),
    /// Value is one of the given set.
    In(Vec<Value>),
    /// Value is null / not null.
    IsNull(bool),
}

impl<T: Table> QueryBuilder<T> {
    /// Create a new empty query builder for the table.
    pub fn new() -> Self {
        Self {
            wheres: Vec::new(),
            order_key: None,
            order_dir: None,
            limit: None,
            offset: None,
            _phantom: std::marker::PhantomData,
        }
    }

    /// Add an equality filter: `WHERE field = value`.
    pub fn where_eq(mut self, field: &str, value: impl Into<Value>) -> Self {
        self.wheres
            .push((field.to_string(), WhereClause::Eq(value.into())));
        self
    }

    /// Add a greater-than filter: `WHERE field > value`.
    pub fn where_gt(mut self, field: &str, value: impl Into<Value>) -> Self {
        self.wheres
            .push((field.to_string(), WhereClause::Gt(value.into())));
        self
    }

    /// Add a less-than filter: `WHERE field < value`.
    pub fn where_lt(mut self, field: &str, value: impl Into<Value>) -> Self {
        self.wheres
            .push((field.to_string(), WhereClause::Lt(value.into())));
        self
    }

    /// Add a greater-than-or-equal filter.
    pub fn where_gte(mut self, field: &str, value: impl Into<Value>) -> Self {
        self.wheres
            .push((field.to_string(), WhereClause::Gte(value.into())));
        self
    }

    /// Add a less-than-or-equal filter.
    pub fn where_lte(mut self, field: &str, value: impl Into<Value>) -> Self {
        self.wheres
            .push((field.to_string(), WhereClause::Lte(value.into())));
        self
    }

    /// Add an IN filter: `WHERE field IN (values...)`.
    pub fn where_in(mut self, field: &str, values: Vec<Value>) -> Self {
        self.wheres
            .push((field.to_string(), WhereClause::In(values)));
        self
    }

    /// Add a null check filter.
    pub fn where_is_null(mut self, field: &str, is_null: bool) -> Self {
        self.wheres
            .push((field.to_string(), WhereClause::IsNull(is_null)));
        self
    }

    /// Set the ordering field and direction ("asc" or "desc").
    pub fn order(mut self, field: &str, direction: &str) -> Self {
        self.order_key = Some(field.to_string());
        self.order_dir = Some(direction.to_string());
        self
    }

    /// Limit the number of results.
    pub fn limit(mut self, n: usize) -> Self {
        self.limit = Some(n);
        self
    }

    /// Skip the first N results.
    pub fn offset(mut self, n: usize) -> Self {
        self.offset = Some(n);
        self
    }

    /// Build the query into an InstantDB InstaQL `Value`.
    ///
    /// Produces the JSON format:
    /// ```json
    /// { "reminders": { "$": { "where": {...}, "order": {...}, "limit": N } } }
    /// ```
    pub fn build(&self) -> Value {
        let mut query_opts = serde_json::Map::new();

        // Build where clause
        if !self.wheres.is_empty() {
            let mut where_obj = serde_json::Map::new();
            for (field, clause) in &self.wheres {
                let v = match clause {
                    WhereClause::Eq(v) => value_to_json(v),
                    WhereClause::Gt(v) => {
                        serde_json::json!({ "$gt": value_to_json(v) })
                    }
                    WhereClause::Lt(v) => {
                        serde_json::json!({ "$lt": value_to_json(v) })
                    }
                    WhereClause::Gte(v) => {
                        serde_json::json!({ "$gte": value_to_json(v) })
                    }
                    WhereClause::Lte(v) => {
                        serde_json::json!({ "$lte": value_to_json(v) })
                    }
                    WhereClause::In(vals) => {
                        let arr: Vec<_> = vals.iter().map(value_to_json).collect();
                        serde_json::json!({ "$in": arr })
                    }
                    WhereClause::IsNull(is_null) => {
                        serde_json::json!({ "$isNull": is_null })
                    }
                };
                where_obj.insert(field.clone(), v);
            }
            query_opts.insert("where".to_string(), serde_json::Value::Object(where_obj));
        }

        // Build order clause
        if let (Some(field), Some(dir)) = (&self.order_key, &self.order_dir) {
            query_opts.insert(
                "order".to_string(),
                serde_json::json!({ "field": field, "direction": dir }),
            );
        }

        // Build limit
        if let Some(limit) = self.limit {
            query_opts.insert("limit".to_string(), serde_json::json!(limit));
        }

        // Build offset
        if let Some(offset) = self.offset {
            query_opts.insert("offset".to_string(), serde_json::json!(offset));
        }

        let mut entity_obj = serde_json::Map::new();
        if !query_opts.is_empty() {
            entity_obj.insert("$".to_string(), serde_json::Value::Object(query_opts));
        }

        let mut root = serde_json::Map::new();
        root.insert(
            T::TABLE_NAME.to_string(),
            serde_json::Value::Object(entity_obj),
        );

        json_to_value(&serde_json::Value::Object(root))
    }
}

impl<T: Table> Default for QueryBuilder<T> {
    fn default() -> Self {
        Self::new()
    }
}
