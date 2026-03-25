//! ┌─────────────────────────────────────────────────────┐
//! │  FETCH ONE                                           │
//! │  Reactive single-value observation                   │
//! ├─────────────────────────────────────────────────────┤
//! │                                                      │
//! │  FetchOne<T: Table>                                  │
//! │    ├── get()   → Option<T>  (current value)          │
//! │    ├── watch() → Stream     (reactive updates)       │
//! │    └── reader()→ SharedReader  (read-only view)      │
//! │                                                      │
//! │  Returns the first matching row, or None.            │
//! │                                                      │
//! ├─────────────────────────────────────────────────────┤
//! │  WHY: Mirrors Swift's @FetchOne property wrapper.    │
//! │  Specialized for single-value queries like counts,   │
//! │  latest records, or specific lookups.                │
//! │                                                      │
//! │  TESTED BY: tests/fetch_one_tests.rs                 │
//! │                                                      │
//! │  CHANGELOG:                                          │
//! │  • v0.1.0 — Initial FetchOne<T>                      │
//! │                                                      │
//! │  HISTORY: git log --oneline --follow -- crates/sharing-instant/src/fetch_one.rs │
//! └─────────────────────────────────────────────────────┘

use crate::database::Database;
use crate::error::{Result, SharingInstantError};
use crate::shared_reader::SharedReader;
use crate::table::Table;
use instant_core::value::Value;
use parking_lot::RwLock;
use std::sync::Arc;
use tokio::sync::watch;

/// Reactive single-value observer backed by InstantDB subscriptions.
///
/// Mirrors Swift's `@FetchOne` property wrapper. Returns the first
/// matching row from a query, or None if no rows match.
///
/// # Example
///
/// ```
/// use sharing_instant::fetch_one::FetchOne;
/// use sharing_instant::table::{Table, ColumnDef};
/// use sharing_instant::database::InMemoryDatabase;
/// use std::sync::Arc;
///
/// #[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
/// struct Counter {
///     id: String,
///     count: i64,
/// }
///
/// impl Table for Counter {
///     const TABLE_NAME: &'static str = "counters";
///     fn columns() -> &'static [ColumnDef] { &[] }
/// }
///
/// let db = Arc::new(InMemoryDatabase::new());
/// let fetch = FetchOne::<Counter>::new(db);
///
/// assert!(fetch.get().is_none());
/// ```
pub struct FetchOne<T: Table> {
    item: Arc<RwLock<Option<T>>>,
    sender: watch::Sender<Option<T>>,
    receiver: watch::Receiver<Option<T>>,
    db: Arc<dyn Database>,
    query: Value,
    is_loading: Arc<RwLock<bool>>,
    load_error: Arc<RwLock<Option<SharingInstantError>>>,
}

impl<T: Table> FetchOne<T> {
    /// Create a FetchOne that fetches the first row from the table.
    pub fn new(db: Arc<dyn Database>) -> Self {
        let query = T::query().limit(1).build();
        Self::with_query(db, query)
    }

    /// Create a FetchOne with a custom query.
    pub fn with_query(db: Arc<dyn Database>, query: Value) -> Self {
        let (sender, receiver) = watch::channel(None);
        let item = Arc::new(RwLock::new(None));
        let is_loading = Arc::new(RwLock::new(true));
        let load_error = Arc::new(RwLock::new(None));

        let mut fetch = Self {
            item,
            sender,
            receiver,
            db,
            query,
            is_loading,
            load_error,
        };

        fetch.load_sync();
        fetch.setup_subscription();
        fetch
    }

    /// Get the current value, if any.
    pub fn get(&self) -> Option<T> {
        self.item.read().clone()
    }

    /// Get the current value, returning an error if not found.
    ///
    /// Mirrors Swift's non-optional `@FetchOne` variant that throws
    /// `NotFound` when no row exists.
    pub fn require(&self) -> Result<T> {
        self.get().ok_or_else(|| SharingInstantError::NotFound {
            entity: T::TABLE_NAME.to_string(),
            query: "FetchOne".to_string(),
        })
    }

    /// Whether the initial load is still in progress.
    pub fn is_loading(&self) -> bool {
        *self.is_loading.read()
    }

    /// Get a reactive watch receiver.
    pub fn watch(&self) -> watch::Receiver<Option<T>> {
        self.receiver.clone()
    }

    /// Create a read-only SharedReader view.
    pub fn reader(&self) -> SharedReader<Option<T>> {
        SharedReader::from_watch(self.get(), self.receiver.clone())
    }

    fn load_sync(&mut self) {
        *self.is_loading.write() = true;

        match self.db.query(&self.query) {
            Ok(result) => {
                let item = Self::parse_first(&result);
                *self.item.write() = item.clone();
                let _ = self.sender.send(item);
                *self.load_error.write() = None;
            }
            Err(e) => {
                *self.load_error.write() = Some(e);
            }
        }

        *self.is_loading.write() = false;
    }

    fn setup_subscription(&self) {
        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            if let Ok(mut rx) = self.db.subscribe(&self.query) {
                let item = self.item.clone();
                let sender = self.sender.clone();

                handle.spawn(async move {
                    while rx.changed().await.is_ok() {
                        if let Some(result) = rx.borrow().as_ref() {
                            let parsed = Self::parse_first(result);
                            *item.write() = parsed.clone();
                            if sender.send(parsed).is_err() {
                                break;
                            }
                        }
                    }
                });
            }
        }
    }

    fn parse_first(result: &Value) -> Option<T> {
        match result {
            Value::Object(obj) => {
                if let Some(rows) = obj.get(T::TABLE_NAME) {
                    match rows {
                        Value::Array(arr) => arr.first().and_then(|v| T::from_value(v).ok()),
                        Value::Object(row_map) => {
                            row_map.values().next().and_then(|v| T::from_value(v).ok())
                        }
                        _ => None,
                    }
                } else {
                    None
                }
            }
            _ => None,
        }
    }
}
