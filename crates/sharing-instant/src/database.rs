//! ┌─────────────────────────────────────────────────────┐
//! │  DATABASE                                            │
//! │  Connection management for InstantDB                  │
//! ├─────────────────────────────────────────────────────┤
//! │                                                      │
//! │  ┌─────────────┐                                     │
//! │  │DefaultDatabase│─── get() ──► Database              │
//! │  │  (global)    │                │                    │
//! │  └─────────────┘                │                    │
//! │                    ┌────────────┼────────────┐       │
//! │                    ▼            ▼            ▼       │
//! │              ┌─────────┐ ┌──────────┐ ┌──────────┐  │
//! │              │  Live   │ │ InMemory │ │   Test   │  │
//! │              │(reactor)│ │ (store)  │ │  (mock)  │  │
//! │              └─────────┘ └──────────┘ └──────────┘  │
//! │                                                      │
//! ├─────────────────────────────────────────────────────┤
//! │  WHY: Mirrors Swift's defaultDatabase dependency.    │
//! │  Context-aware: live uses WebSocket reactor,         │
//! │  tests use in-memory store, previews use ephemeral.  │
//! │                                                      │
//! │  ALTERNATIVES: Global singleton (not testable),      │
//! │  thread-local (not shareable across async tasks).    │
//! │                                                      │
//! │  TESTED BY: tests/database_tests.rs                  │
//! │                                                      │
//! │  CHANGELOG:                                          │
//! │  • v0.1.0 — Initial Database trait + DefaultDatabase │
//! │                                                      │
//! │  HISTORY: git log --oneline --follow -- crates/sharing-instant/src/database.rs │
//! └─────────────────────────────────────────────────────┘

use crate::error::{Result, SharingInstantError};
use crate::table::{json_to_value, value_to_json};
use instant_client::Reactor;
use instant_core::instatx;
use instant_core::value::Value;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;

/// Core database abstraction.
///
/// Provides read and write access to an InstantDB store. Mirrors
/// Swift's `DatabaseReader` / `DatabaseWriter` protocols from GRDB,
/// but adapted for InstantDB's EAV model.
///
/// # Example
///
/// ```
/// use sharing_instant::database::{Database, InMemoryDatabase};
/// use sharing_instant::Value;
///
/// let db = InMemoryDatabase::new();
/// // Write operations go through the database
/// ```
pub trait Database: Send + Sync + 'static {
    /// Execute a read-only query and return results.
    fn query(&self, q: &Value) -> Result<Value>;

    /// Execute a transaction (create/update/delete).
    fn transact(&self, tx_steps: &Value) -> Result<()>;

    /// Subscribe to a query, receiving updates when results change.
    ///
    /// Returns a receiver that yields new values whenever the
    /// query results change in the store.
    fn subscribe(&self, q: &Value) -> Result<tokio::sync::watch::Receiver<Option<Value>>>;
}

/// In-memory database for testing and previews.
///
/// Uses a simple JSON-based store for testing. The production
/// database will use InstantDB's full Store + reactor. This
/// in-memory variant provides the same API surface without
/// network connectivity.
///
/// # Example
///
/// ```
/// use sharing_instant::database::InMemoryDatabase;
///
/// let db = InMemoryDatabase::new();
/// // Use for tests and previews
/// ```
pub struct InMemoryDatabase {
    /// entity_type -> entity_id -> entity_data
    entities: Arc<RwLock<HashMap<String, HashMap<String, serde_json::Value>>>>,
    subscriptions: Arc<RwLock<Vec<SubscriptionEntry>>>,
}

struct SubscriptionEntry {
    query: Value,
    sender: tokio::sync::watch::Sender<Option<Value>>,
}

impl InMemoryDatabase {
    /// Create a new empty in-memory database.
    pub fn new() -> Self {
        Self {
            entities: Arc::new(RwLock::new(HashMap::new())),
            subscriptions: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Directly insert an entity for testing convenience.
    ///
    /// # Example
    ///
    /// ```
    /// use sharing_instant::database::InMemoryDatabase;
    ///
    /// let db = InMemoryDatabase::new();
    /// db.insert("reminders", "r1", serde_json::json!({
    ///     "id": "r1",
    ///     "title": "Buy milk",
    ///     "isCompleted": false,
    /// }));
    /// ```
    pub fn insert(&self, entity_type: &str, id: &str, data: serde_json::Value) {
        let mut entities = self.entities.write();
        entities
            .entry(entity_type.to_string())
            .or_default()
            .insert(id.to_string(), data);

        drop(entities);
        self.notify_subscriptions();
    }

    /// Remove an entity by type and ID.
    pub fn remove(&self, entity_type: &str, id: &str) {
        let mut entities = self.entities.write();
        if let Some(table) = entities.get_mut(entity_type) {
            table.remove(id);
        }
        drop(entities);
        self.notify_subscriptions();
    }

    /// Build the query result for a given InstaQL-style query.
    fn execute_query(&self, q: &Value) -> Value {
        let entities = self.entities.read();

        // Parse query: { "tableName": { "$": { "where": {...} } } }
        match q {
            Value::Object(query_obj) => {
                let mut result = std::collections::BTreeMap::new();
                for (table_name, _opts) in query_obj {
                    if let Some(table_data) = entities.get(table_name) {
                        let rows: Vec<Value> =
                            table_data.values().map(|v| json_to_value(v)).collect();
                        result.insert(table_name.clone(), Value::Array(rows));
                    } else {
                        result.insert(table_name.clone(), Value::Array(Vec::new()));
                    }
                }
                Value::Object(result)
            }
            _ => Value::Object(std::collections::BTreeMap::new()),
        }
    }

    /// Re-execute all subscribed queries and push updates.
    fn notify_subscriptions(&self) {
        let subs = self.subscriptions.read();
        for entry in subs.iter() {
            let result = self.execute_query(&entry.query);
            let _ = entry.sender.send(Some(result));
        }
    }
}

impl Default for InMemoryDatabase {
    fn default() -> Self {
        Self::new()
    }
}

impl Database for InMemoryDatabase {
    fn query(&self, q: &Value) -> Result<Value> {
        Ok(self.execute_query(q))
    }

    fn transact(&self, tx_steps: &Value) -> Result<()> {
        // Parse transaction steps: [["update", "tableName", "id", { data }], ...]
        match tx_steps {
            Value::Array(steps) => {
                for step in steps {
                    if let Value::Array(parts) = step {
                        if parts.len() >= 4 {
                            let op = match &parts[0] {
                                Value::String(s) => s.as_str(),
                                _ => continue,
                            };
                            let entity_type = match &parts[1] {
                                Value::String(s) => s.clone(),
                                _ => continue,
                            };
                            let id = match &parts[2] {
                                Value::String(s) => s.clone(),
                                _ => continue,
                            };

                            match op {
                                "update" | "create" | "merge" => {
                                    let data = crate::table::value_to_json(&parts[3]);
                                    self.insert(&entity_type, &id, data);
                                }
                                "delete" => {
                                    self.remove(&entity_type, &id);
                                }
                                _ => {}
                            }
                        }
                    }
                }
                Ok(())
            }
            _ => Err(SharingInstantError::TransactionFailed(
                "tx_steps must be an array".to_string(),
            )),
        }
    }

    fn subscribe(&self, q: &Value) -> Result<tokio::sync::watch::Receiver<Option<Value>>> {
        let initial = self.execute_query(q);
        let (tx, rx) = tokio::sync::watch::channel(Some(initial));

        let mut subs = self.subscriptions.write();
        subs.push(SubscriptionEntry {
            query: q.clone(),
            sender: tx,
        });

        Ok(rx)
    }
}

/// Live database backed by an InstantDB Reactor.
///
/// Bridges the synchronous `Database` trait to the Reactor's async API
/// using `block_in_place + block_on`. Value conversion between
/// `instant_core::value::Value` and `serde_json::Value` happens at
/// the boundary.
///
/// # Example
///
/// ```ignore
/// use instant_client::{ConnectionConfig, Reactor};
/// use sharing_instant::database::LiveDatabase;
/// use std::sync::Arc;
///
/// let reactor = Arc::new(Reactor::new(ConnectionConfig::admin("app-id", "token")));
/// let handle = tokio::runtime::Handle::current();
/// let db = LiveDatabase::new(reactor, handle);
/// ```
pub struct LiveDatabase {
    reactor: Arc<RwLock<Arc<Reactor>>>,
    handle: tokio::runtime::Handle,
}

impl LiveDatabase {
    /// Create a new LiveDatabase wrapping the given Reactor.
    ///
    /// The handle must be a tokio runtime handle for the sync-async bridge.
    pub fn new(reactor: Arc<Reactor>, handle: tokio::runtime::Handle) -> Self {
        Self {
            reactor: Arc::new(RwLock::new(reactor)),
            handle,
        }
    }

    /// Swap the inner Reactor (used by SyncEngine on restart).
    pub fn set_reactor(&self, reactor: Arc<Reactor>) {
        *self.reactor.write() = reactor;
    }

    fn current_reactor(&self) -> Arc<Reactor> {
        self.reactor.read().clone()
    }
}

impl Database for LiveDatabase {
    fn query(&self, q: &Value) -> Result<Value> {
        let json_query = value_to_json(q);
        let reactor = self.current_reactor();

        tokio::task::block_in_place(|| {
            self.handle.block_on(async {
                let mut rx = reactor.subscribe(json_query.clone()).await;

                // Wait for first non-None result, then unsubscribe.
                loop {
                    {
                        let current = rx.borrow().clone();
                        if let Some(data) = current {
                            reactor.unsubscribe(&json_query).await;
                            return Ok(json_to_value(&data));
                        }
                    }
                    rx.changed().await.map_err(|_| {
                        SharingInstantError::QueryFailed(
                            "subscription closed before receiving data".to_string(),
                        )
                    })?;
                }
            })
        })
    }

    fn transact(&self, tx_steps: &Value) -> Result<()> {
        let reactor = self.current_reactor();

        // Convert Value ops into TransactionChunks for instaml::transform.
        // Input format: [["update", "etype", "id", {attrs}], ...]
        let chunks = value_to_transaction_chunks(tx_steps)?;

        tokio::task::block_in_place(|| {
            self.handle.block_on(async {
                reactor
                    .transact_chunks(&chunks)
                    .await
                    .map_err(|e| SharingInstantError::TransactionFailed(e.to_string()))?;
                Ok(())
            })
        })
    }

    fn subscribe(&self, q: &Value) -> Result<tokio::sync::watch::Receiver<Option<Value>>> {
        let json_query = value_to_json(q);
        let reactor = self.current_reactor();

        // Create a bridge channel that converts serde_json::Value → instant_core::value::Value.
        let (bridge_tx, bridge_rx) = tokio::sync::watch::channel(None);

        let handle = self.handle.clone();
        tokio::task::block_in_place(|| {
            handle.block_on(async {
                let mut rx = reactor.subscribe(json_query).await;

                // Forward initial value if present.
                if let Some(data) = rx.borrow().clone() {
                    let _ = bridge_tx.send(Some(json_to_value(&data)));
                }

                // Spawn a task to bridge subsequent updates.
                tokio::spawn(async move {
                    while rx.changed().await.is_ok() {
                        let data = rx.borrow().clone();
                        let converted = data.map(|d| json_to_value(&d));
                        if bridge_tx.send(converted).is_err() {
                            break; // All receivers dropped
                        }
                    }
                });
            });
        });

        Ok(bridge_rx)
    }
}

/// Convert a Value array of high-level ops into TransactionChunks.
///
/// Input: `[["update", "etype", "id", {attrs}], ["delete", "etype", "id", {}], ...]`
/// Each op becomes a TransactionChunk with the appropriate method called.
fn value_to_transaction_chunks(tx_steps: &Value) -> Result<Vec<instatx::TransactionChunk>> {
    let steps = match tx_steps {
        Value::Array(arr) => arr,
        _ => {
            return Err(SharingInstantError::TransactionFailed(
                "tx_steps must be an array".to_string(),
            ))
        }
    };

    let mut chunks = Vec::new();
    for step in steps {
        let parts = match step {
            Value::Array(arr) if arr.len() >= 3 => arr,
            _ => continue,
        };

        let op = match &parts[0] {
            Value::String(s) => s.as_str(),
            _ => continue,
        };
        let etype = match &parts[1] {
            Value::String(s) => s.as_str(),
            _ => continue,
        };
        let id = parts[2].clone();
        let args = if parts.len() > 3 {
            parts[3].clone()
        } else {
            Value::Object(Default::default())
        };

        let chunk = match op {
            "update" | "merge" => {
                if op == "merge" {
                    instatx::tx(etype, id).merge(args)
                } else {
                    instatx::tx(etype, id).update(args)
                }
            }
            "create" => instatx::tx(etype, id).create(args),
            "delete" => instatx::tx(etype, id).delete(),
            "link" => instatx::tx(etype, id).link(args),
            "unlink" => instatx::tx(etype, id).unlink(args),
            other => {
                return Err(SharingInstantError::TransactionFailed(format!(
                    "unknown transaction op: {}",
                    other
                )))
            }
        };
        chunks.push(chunk);
    }

    Ok(chunks)
}

/// Global database holder with context-aware defaults.
///
/// Mirrors Swift's `@Dependency(\.defaultDatabase)` pattern.
/// Set up once at app startup, then accessed from anywhere.
///
/// # Example
///
/// ```
/// use sharing_instant::database::{DefaultDatabase, InMemoryDatabase};
///
/// // In tests:
/// DefaultDatabase::set(InMemoryDatabase::new());
///
/// // Anywhere:
/// let db = DefaultDatabase::get();
/// ```
pub struct DefaultDatabase;

static DEFAULT_DB: std::sync::OnceLock<Arc<dyn Database>> = std::sync::OnceLock::new();

impl DefaultDatabase {
    /// Set the global database instance.
    ///
    /// Should be called once at app startup. Panics if called twice
    /// (use `set_or_replace` for test contexts).
    pub fn set(db: impl Database) {
        let _ = DEFAULT_DB.set(Arc::new(db));
    }

    /// Set the global database from an existing `Arc<dyn Database>`.
    ///
    /// Useful when the database is already wrapped in an Arc (e.g., from SyncEngine).
    pub fn set_arc(db: Arc<dyn Database>) {
        let _ = DEFAULT_DB.set(db);
    }

    /// Get the global database instance.
    ///
    /// Panics if `set()` has not been called.
    pub fn get() -> Arc<dyn Database> {
        DEFAULT_DB
            .get()
            .expect("DefaultDatabase not initialized. Call DefaultDatabase::set() at startup.")
            .clone()
    }

    /// Check if the database has been initialized.
    pub fn is_initialized() -> bool {
        DEFAULT_DB.get().is_some()
    }
}
