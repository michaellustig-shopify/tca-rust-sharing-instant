use crate::database::Database;
use crate::error::{Result, SharingInstantError};
use crate::mutation_callbacks::MutationCallbacks;
use crate::table::Table;
use instant_core::value::Value;
use std::sync::Arc;

/// Typed mutator for CRUD operations on a `Table` type.
///
/// Builds InstantDB transaction steps and executes them through the
/// Database trait. Each method produces `[["op", TABLE_NAME, id, {attrs}]]`
/// arrays that the Database (InMemory or Live) processes.
///
/// # Example
///
/// ```
/// use sharing_instant::mutations::Mutator;
/// use sharing_instant::database::InMemoryDatabase;
/// use sharing_instant::table::{Table, ColumnDef};
/// use std::sync::Arc;
///
/// #[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
/// struct Todo {
///     id: String,
///     title: String,
///     is_completed: bool,
/// }
///
/// impl Table for Todo {
///     const TABLE_NAME: &'static str = "todos";
///     fn columns() -> &'static [ColumnDef] { &[] }
/// }
///
/// let db: Arc<dyn sharing_instant::Database> = Arc::new(InMemoryDatabase::new());
/// let mutator = Mutator::<Todo>::new(db);
///
/// mutator.create(&Todo {
///     id: "t1".into(),
///     title: "Buy milk".into(),
///     is_completed: false,
/// }).unwrap();
/// ```
pub struct Mutator<T: Table> {
    db: Arc<dyn Database>,
    _phantom: std::marker::PhantomData<T>,
}

impl<T: Table> Mutator<T> {
    /// Create a new mutator backed by the given database.
    pub fn new(db: Arc<dyn Database>) -> Self {
        Self {
            db,
            _phantom: std::marker::PhantomData,
        }
    }

    /// Create a new entity.
    pub fn create(&self, item: &T) -> Result<()> {
        let (id, attrs) = self.extract_id_and_attrs(item)?;
        let steps = self.build_steps("update", &id, attrs);
        self.db.transact(&steps)
    }

    /// Create with mutation callbacks.
    pub fn create_with_callbacks(&self, item: &T, callbacks: MutationCallbacks<()>) -> Result<()> {
        Self::with_callbacks_inner(self.create(item), callbacks)
    }

    /// Update an existing entity by ID.
    pub fn update(&self, id: &str, item: &T) -> Result<()> {
        let attrs = item
            .to_value()
            .map_err(|e| SharingInstantError::SerializationError(e))?;
        let steps = self.build_steps("update", id, attrs);
        self.db.transact(&steps)
    }

    /// Update with mutation callbacks.
    pub fn update_with_callbacks(
        &self,
        id: &str,
        item: &T,
        callbacks: MutationCallbacks<()>,
    ) -> Result<()> {
        Self::with_callbacks_inner(self.update(id, item), callbacks)
    }

    /// Delete an entity by ID.
    pub fn delete(&self, id: &str) -> Result<()> {
        let steps = Value::Array(vec![Value::Array(vec![
            Value::String("delete".to_string()),
            Value::String(T::TABLE_NAME.to_string()),
            Value::String(id.to_string()),
            Value::Object(Default::default()),
        ])]);
        self.db.transact(&steps)
    }

    /// Delete with mutation callbacks.
    pub fn delete_with_callbacks(&self, id: &str, callbacks: MutationCallbacks<()>) -> Result<()> {
        Self::with_callbacks_inner(self.delete(id), callbacks)
    }

    /// Create a link between two entities.
    pub fn link(&self, id: &str, field: &str, target_id: &str) -> Result<()> {
        let mut link_data = std::collections::BTreeMap::new();
        link_data.insert(field.to_string(), Value::String(target_id.to_string()));

        let steps = Value::Array(vec![Value::Array(vec![
            Value::String("link".to_string()),
            Value::String(T::TABLE_NAME.to_string()),
            Value::String(id.to_string()),
            Value::Object(link_data),
        ])]);
        self.db.transact(&steps)
    }

    /// Create a link with mutation callbacks.
    pub fn link_with_callbacks(
        &self,
        id: &str,
        field: &str,
        target_id: &str,
        callbacks: MutationCallbacks<()>,
    ) -> Result<()> {
        Self::with_callbacks_inner(self.link(id, field, target_id), callbacks)
    }

    /// Remove a link between two entities.
    pub fn unlink(&self, id: &str, field: &str, target_id: &str) -> Result<()> {
        let mut link_data = std::collections::BTreeMap::new();
        link_data.insert(field.to_string(), Value::String(target_id.to_string()));

        let steps = Value::Array(vec![Value::Array(vec![
            Value::String("unlink".to_string()),
            Value::String(T::TABLE_NAME.to_string()),
            Value::String(id.to_string()),
            Value::Object(link_data),
        ])]);
        self.db.transact(&steps)
    }

    /// Remove a link with mutation callbacks.
    pub fn unlink_with_callbacks(
        &self,
        id: &str,
        field: &str,
        target_id: &str,
        callbacks: MutationCallbacks<()>,
    ) -> Result<()> {
        Self::with_callbacks_inner(self.unlink(id, field, target_id), callbacks)
    }

    /// Shared callback dispatch for synchronous mutation results.
    fn with_callbacks_inner(result: Result<()>, callbacks: MutationCallbacks<()>) -> Result<()> {
        match result {
            Ok(()) => {
                callbacks.fire_success(());
                Ok(())
            }
            Err(e) => {
                let msg = e.to_string();
                callbacks.fire_error(e);
                Err(SharingInstantError::TransactionFailed(msg))
            }
        }
    }

    /// Extract the "id" field and full attrs from a Table item.
    fn extract_id_and_attrs(&self, item: &T) -> Result<(String, Value)> {
        let value = item
            .to_value()
            .map_err(|e| SharingInstantError::SerializationError(e))?;

        // Extract the id from the serialized value.
        let id = match &value {
            Value::Object(obj) => obj
                .get("id")
                .and_then(|v| match v {
                    Value::String(s) => Some(s.clone()),
                    _ => None,
                })
                .ok_or_else(|| {
                    SharingInstantError::SerializationError(
                        "Table item must have a string 'id' field".to_string(),
                    )
                })?,
            _ => {
                return Err(SharingInstantError::SerializationError(
                    "Table item must serialize to an object".to_string(),
                ))
            }
        };

        Ok((id, value))
    }

    /// Build a transaction step array: [["op", TABLE_NAME, id, {attrs}]]
    fn build_steps(&self, op: &str, id: &str, attrs: Value) -> Value {
        Value::Array(vec![Value::Array(vec![
            Value::String(op.to_string()),
            Value::String(T::TABLE_NAME.to_string()),
            Value::String(id.to_string()),
            attrs,
        ])])
    }
}

impl<T: Table> Clone for Mutator<T> {
    fn clone(&self) -> Self {
        Self {
            db: self.db.clone(),
            _phantom: std::marker::PhantomData,
        }
    }
}
