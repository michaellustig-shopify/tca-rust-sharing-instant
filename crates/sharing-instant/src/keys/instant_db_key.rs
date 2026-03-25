//! ┌─────────────────────────────────────────────────────┐
//! │  INSTANT DB KEY                                      │
//! │  Persistence backed by InstantDB                     │
//! ├─────────────────────────────────────────────────────┤
//! │                                                      │
//! │  InstantDbKey<T: Table>                              │
//! │    ├── load()      → query store for entity          │
//! │    ├── save()      → transact update/create          │
//! │    └── subscribe() → watch query for changes         │
//! │                                                      │
//! │  This is the primary persistence strategy,           │
//! │  replacing Swift's SQLite-backed FetchKey.           │
//! │                                                      │
//! ├─────────────────────────────────────────────────────┤
//! │  WHY: InstantDB provides real-time sync for free     │
//! │  via its WebSocket reactor. No separate SyncEngine   │
//! │  needed — just subscribe and the server pushes       │
//! │  changes from other clients.                         │
//! │                                                      │
//! │  ALTERNATIVES: SQLite+manual sync (complex),         │
//! │  file storage (no real-time), in-memory (no persist).│
//! │                                                      │
//! │  TESTED BY: tests/instant_db_key_tests.rs            │
//! │  EDGE CASES: offline mode, reconnection, conflicts,  │
//! │  schema migration, missing entities                   │
//! │                                                      │
//! │  CHANGELOG:                                          │
//! │  • v0.1.0 — Initial InstantDbKey                     │
//! │                                                      │
//! │  HISTORY: git log --oneline --follow -- crates/sharing-instant/src/keys/instant_db_key.rs │
//! └─────────────────────────────────────────────────────┘

use crate::database::Database;
use crate::error::{Result, SharingInstantError};
use crate::shared_key::{SaveContext, SharedKey};
use crate::shared_reader_key::{LoadContext, SharedReaderKey, SharedSubscriber};
use crate::subscription::SharedSubscription;
use crate::table::Table;
use instant_core::value::Value;
use std::sync::Arc;

/// InstantDB-backed persistence key for a single entity.
///
/// Persists a `Table` type to InstantDB. Supports:
/// - Loading by entity ID
/// - Saving (create or update) via transactions
/// - Subscribing to real-time changes via WebSocket
///
/// # Example
///
/// ```ignore
/// use sharing_instant::keys::instant_db_key::InstantDbKey;
/// use sharing_instant::shared::Shared;
///
/// let key = InstantDbKey::<Reminder>::new(
///     "reminder-123",
///     db.clone(),
/// );
///
/// let shared = Shared::new(
///     Reminder { id: "reminder-123".into(), title: "Buy milk".into(), .. },
///     key,
/// );
/// ```
pub struct InstantDbKey<T: Table> {
    entity_id: String,
    db: Arc<dyn Database>,
    _phantom: std::marker::PhantomData<T>,
}

impl<T: Table> InstantDbKey<T> {
    /// Create a new InstantDB key for the given entity ID.
    pub fn new(entity_id: &str, db: Arc<dyn Database>) -> Self {
        Self {
            entity_id: entity_id.to_string(),
            db,
            _phantom: std::marker::PhantomData,
        }
    }

    /// Build the InstaQL query for this specific entity.
    fn entity_query(&self) -> Value {
        let query = serde_json::json!({
            T::TABLE_NAME: {
                "$": {
                    "where": {
                        "id": self.entity_id
                    }
                }
            }
        });
        crate::table::json_to_value(&query)
    }
}

impl<T: Table> SharedReaderKey for InstantDbKey<T> {
    type Value = T;
    type Id = String;

    fn id(&self) -> String {
        format!("instant_db:{}:{}", T::TABLE_NAME, self.entity_id)
    }

    fn load(&self, _context: LoadContext<T>) -> Result<Option<T>> {
        let query = self.entity_query();
        let result = self.db.query(&query)?;

        // Parse the result to find our entity
        match &result {
            Value::Object(obj) => {
                if let Some(rows) = obj.get(T::TABLE_NAME) {
                    match rows {
                        Value::Array(arr) => Ok(arr.first().and_then(|v| T::from_value(v).ok())),
                        Value::Object(map) => {
                            Ok(map.values().next().and_then(|v| T::from_value(v).ok()))
                        }
                        _ => Ok(None),
                    }
                } else {
                    Ok(None)
                }
            }
            _ => Ok(None),
        }
    }

    fn subscribe(
        &self,
        _context: LoadContext<T>,
        subscriber: SharedSubscriber<T>,
    ) -> SharedSubscription {
        let query = self.entity_query();

        match self.db.subscribe(&query) {
            Ok(mut rx) => {
                let table_name = T::TABLE_NAME.to_string();

                // Spawn a task that watches the subscription and pushes
                // parsed updates to the subscriber
                let handle = tokio::spawn(async move {
                    while rx.changed().await.is_ok() {
                        if let Some(result) = rx.borrow().as_ref() {
                            if let Value::Object(obj) = result {
                                if let Some(rows) = obj.get(&table_name) {
                                    let item = match rows {
                                        Value::Array(arr) => {
                                            arr.first().and_then(|v| T::from_value(v).ok())
                                        }
                                        Value::Object(map) => {
                                            map.values().next().and_then(|v| T::from_value(v).ok())
                                        }
                                        _ => None,
                                    };
                                    if let Some(item) = item {
                                        subscriber.yield_value(item);
                                    }
                                }
                            }
                        }
                    }
                });

                SharedSubscription::new(move || {
                    handle.abort();
                })
            }
            Err(_) => SharedSubscription::empty(),
        }
    }
}

impl<T: Table> SharedKey for InstantDbKey<T> {
    fn save(&self, value: &T, _context: SaveContext) -> Result<()> {
        let data = value
            .to_value()
            .map_err(|e| SharingInstantError::SerializationError(e))?;

        // Build transaction: update the entity
        let tx = serde_json::json!([["update", T::TABLE_NAME, self.entity_id, data]]);
        let tx_value = crate::table::json_to_value(&tx);

        self.db.transact(&tx_value)
    }
}
