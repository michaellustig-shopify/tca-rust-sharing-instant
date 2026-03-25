//! ┌─────────────────────────────────────────────────────┐
//! │  FETCH ALL                                           │
//! │  Reactive collection observation                     │
//! ├─────────────────────────────────────────────────────┤
//! │                                                      │
//! │  FetchAll<T: Table>                                  │
//! │    ├── get()   → &[T]       (current results)        │
//! │    ├── watch() → Stream     (reactive updates)       │
//! │    ├── load()  → reload     (manual refresh)         │
//! │    └── reader()→ SharedReader  (read-only view)      │
//! │                                                      │
//! │  Backed by InstantDB subscribe() → QueryStream       │
//! │                                                      │
//! ├─────────────────────────────────────────────────────┤
//! │  WHY: Mirrors Swift's @FetchAll property wrapper.    │
//! │  Provides reactive, always-up-to-date collection     │
//! │  views backed by InstantDB subscriptions.            │
//! │                                                      │
//! │  ALTERNATIVES: Manual poll (wasteful), event bus     │
//! │  (lossy), CQRS (over-engineered for this).           │
//! │                                                      │
//! │  TESTED BY: tests/fetch_all_tests.rs                 │
//! │  EDGE CASES: empty results, concurrent updates,      │
//! │  subscription reconnection, deserialization errors    │
//! │                                                      │
//! │  CHANGELOG:                                          │
//! │  • v0.1.0 — Initial FetchAll<T>                      │
//! │                                                      │
//! │  HISTORY: git log --oneline --follow -- crates/sharing-instant/src/fetch_all.rs │
//! └─────────────────────────────────────────────────────┘

use crate::database::Database;
use crate::error::{Result, SharingInstantError};
use crate::mutation_callbacks::MutationCallbacks;
use crate::shared_reader::SharedReader;
use crate::table::{json_to_value, value_to_json, Table};
use instant_core::value::Value;
use parking_lot::RwLock;
use std::sync::Arc;
use tokio::sync::watch;

/// Reactive collection observer backed by InstantDB subscriptions.
///
/// Mirrors Swift's `@FetchAll` property wrapper. Subscribes to an
/// InstantDB query and automatically updates when results change.
///
/// # Example
///
/// ```
/// use sharing_instant::fetch_all::FetchAll;
/// use sharing_instant::table::{Table, ColumnDef};
/// use sharing_instant::database::InMemoryDatabase;
/// use std::sync::Arc;
///
/// #[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
/// struct Item {
///     id: String,
///     title: String,
/// }
///
/// impl Table for Item {
///     const TABLE_NAME: &'static str = "items";
///     fn columns() -> &'static [ColumnDef] { &[] }
/// }
///
/// let db = Arc::new(InMemoryDatabase::new());
/// let fetch = FetchAll::<Item>::new(db);
///
/// assert_eq!(fetch.get().len(), 0);
/// ```
pub struct FetchAll<T: Table> {
    items: Arc<RwLock<Vec<T>>>,
    sender: watch::Sender<Vec<T>>,
    receiver: watch::Receiver<Vec<T>>,
    db: Arc<dyn Database>,
    query: Value,
    is_loading: Arc<RwLock<bool>>,
    load_error: Arc<RwLock<Option<SharingInstantError>>>,
}

impl<T: Table> FetchAll<T> {
    /// Create a new FetchAll that fetches all rows from the table.
    ///
    /// Immediately executes the initial query and subscribes to
    /// future changes.
    pub fn new(db: Arc<dyn Database>) -> Self {
        let query = T::query().build();
        Self::with_query(db, query)
    }

    /// Create a FetchAll with a custom query.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let fetch = FetchAll::<Reminder>::with_query(
    ///     db,
    ///     Reminder::query().where_eq("is_completed", false).build(),
    /// );
    /// ```
    pub fn with_query(db: Arc<dyn Database>, query: Value) -> Self {
        let (sender, receiver) = watch::channel(Vec::new());
        let items = Arc::new(RwLock::new(Vec::new()));
        let is_loading = Arc::new(RwLock::new(true));
        let load_error = Arc::new(RwLock::new(None));

        let mut fetch = Self {
            items,
            sender,
            receiver,
            db,
            query,
            is_loading,
            load_error,
        };

        // Initial load
        fetch.load_sync();

        // Set up subscription
        fetch.setup_subscription();

        fetch
    }

    /// Get the current items.
    pub fn get(&self) -> Vec<T> {
        self.items.read().clone()
    }

    /// Whether the initial load is still in progress.
    pub fn is_loading(&self) -> bool {
        *self.is_loading.read()
    }

    /// The most recent load error, if any.
    pub fn load_error(&self) -> Option<String> {
        self.load_error.read().as_ref().map(|e| e.to_string())
    }

    /// Get a reactive watch receiver for the items.
    pub fn watch(&self) -> watch::Receiver<Vec<T>> {
        self.receiver.clone()
    }

    /// Create a read-only SharedReader view.
    pub fn reader(&self) -> SharedReader<Vec<T>> {
        SharedReader::from_watch(self.get(), self.receiver.clone())
    }

    /// Manually reload data from the database.
    pub fn load(&mut self) -> Result<()> {
        self.load_sync();
        if let Some(err) = self.load_error.read().as_ref() {
            Err(SharingInstantError::QueryFailed(err.to_string()))
        } else {
            Ok(())
        }
    }

    /// Synchronous load implementation.
    fn load_sync(&mut self) {
        *self.is_loading.write() = true;

        match self.db.query(&self.query) {
            Ok(result) => {
                let items = Self::parse_results(&result);
                *self.items.write() = items.clone();
                let _ = self.sender.send(items);
                *self.load_error.write() = None;
            }
            Err(e) => {
                *self.load_error.write() = Some(e);
            }
        }

        *self.is_loading.write() = false;
    }

    /// Set up a subscription to the query for reactive updates.
    fn setup_subscription(&self) {
        // Only set up async subscription if a tokio runtime is available
        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            if let Ok(mut rx) = self.db.subscribe(&self.query) {
                let items = self.items.clone();
                let sender = self.sender.clone();

                handle.spawn(async move {
                    while rx.changed().await.is_ok() {
                        if let Some(result) = rx.borrow().as_ref() {
                            let parsed = Self::parse_results(result);
                            *items.write() = parsed.clone();
                            if sender.send(parsed).is_err() {
                                break;
                            }
                        }
                    }
                });
            }
        }
    }

    // --- Mutation methods (delegate to Mutator<T>) ---

    /// Create a new entity. The subscription automatically picks up the change.
    pub fn create(&self, item: &T) -> Result<()> {
        crate::mutations::Mutator::<T>::new(self.db.clone()).create(item)
    }

    /// Update an existing entity by ID.
    pub fn update(&self, id: &str, item: &T) -> Result<()> {
        crate::mutations::Mutator::<T>::new(self.db.clone()).update(id, item)
    }

    /// Delete an entity by ID.
    pub fn delete(&self, id: &str) -> Result<()> {
        crate::mutations::Mutator::<T>::new(self.db.clone()).delete(id)
    }

    /// Create a link between two entities.
    pub fn link(&self, id: &str, field: &str, target_id: &str) -> Result<()> {
        crate::mutations::Mutator::<T>::new(self.db.clone()).link(id, field, target_id)
    }

    /// Remove a link between two entities.
    pub fn unlink(&self, id: &str, field: &str, target_id: &str) -> Result<()> {
        crate::mutations::Mutator::<T>::new(self.db.clone()).unlink(id, field, target_id)
    }

    /// Create with mutation callbacks.
    pub fn create_with_callbacks(&self, item: &T, callbacks: MutationCallbacks<()>) -> Result<()> {
        crate::mutations::Mutator::<T>::new(self.db.clone()).create_with_callbacks(item, callbacks)
    }

    /// Update with mutation callbacks.
    pub fn update_with_callbacks(
        &self,
        id: &str,
        item: &T,
        callbacks: MutationCallbacks<()>,
    ) -> Result<()> {
        crate::mutations::Mutator::<T>::new(self.db.clone())
            .update_with_callbacks(id, item, callbacks)
    }

    /// Delete with mutation callbacks.
    pub fn delete_with_callbacks(&self, id: &str, callbacks: MutationCallbacks<()>) -> Result<()> {
        crate::mutations::Mutator::<T>::new(self.db.clone()).delete_with_callbacks(id, callbacks)
    }

    /// Create a link with mutation callbacks.
    pub fn link_with_callbacks(
        &self,
        id: &str,
        field: &str,
        target_id: &str,
        callbacks: MutationCallbacks<()>,
    ) -> Result<()> {
        crate::mutations::Mutator::<T>::new(self.db.clone())
            .link_with_callbacks(id, field, target_id, callbacks)
    }

    /// Remove a link with mutation callbacks.
    pub fn unlink_with_callbacks(
        &self,
        id: &str,
        field: &str,
        target_id: &str,
        callbacks: MutationCallbacks<()>,
    ) -> Result<()> {
        crate::mutations::Mutator::<T>::new(self.db.clone())
            .unlink_with_callbacks(id, field, target_id, callbacks)
    }

    /// Parse InstantDB query results into typed items.
    fn parse_results(result: &Value) -> Vec<T> {
        // InstantDB returns: { "tableName": [{ ... }, { ... }] }
        match result {
            Value::Object(obj) => {
                if let Some(rows) = obj.get(T::TABLE_NAME) {
                    match rows {
                        Value::Array(arr) => {
                            arr.iter().filter_map(|v| T::from_value(v).ok()).collect()
                        }
                        Value::Object(row_map) => row_map
                            .values()
                            .filter_map(|v| T::from_value(v).ok())
                            .collect(),
                        _ => Vec::new(),
                    }
                } else {
                    Vec::new()
                }
            }
            _ => Vec::new(),
        }
    }
}
