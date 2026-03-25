//! ┌─────────────────────────────────────────────────────┐
//! │  FETCH                                               │
//! │  Custom multi-query reactive observation              │
//! ├─────────────────────────────────────────────────────┤
//! │                                                      │
//! │  Fetch<R: FetchKeyRequest>                           │
//! │    ├── get()   → &R::Value  (combined result)        │
//! │    ├── watch() → Stream     (reactive updates)       │
//! │    └── reader()→ SharedReader  (read-only view)      │
//! │                                                      │
//! │  Combines multiple queries into a single atomic      │
//! │  observation. Updates when ANY dependency changes.    │
//! │                                                      │
//! ├─────────────────────────────────────────────────────┤
//! │  WHY: Mirrors Swift's @Fetch property wrapper.       │
//! │  Enables complex views that depend on multiple       │
//! │  database queries without manual orchestration.      │
//! │                                                      │
//! │  TESTED BY: tests/fetch_tests.rs                     │
//! │                                                      │
//! │  CHANGELOG:                                          │
//! │  • v0.1.0 — Initial Fetch<R>                         │
//! │                                                      │
//! │  HISTORY: git log --oneline --follow -- crates/sharing-instant/src/fetch.rs │
//! └─────────────────────────────────────────────────────┘

use crate::database::Database;
use crate::error::{Result, SharingInstantError};
use crate::fetch_key_request::FetchKeyRequest;
use crate::shared_reader::SharedReader;
use parking_lot::RwLock;
use std::sync::Arc;
use tokio::sync::watch;

/// Custom multi-query reactive observer.
///
/// Mirrors Swift's `@Fetch` property wrapper. Takes a `FetchKeyRequest`
/// that combines multiple queries, and re-fetches when any of the
/// underlying queries change.
///
/// # Example
///
/// ```
/// use sharing_instant::fetch::Fetch;
/// use sharing_instant::fetch_key_request::FetchKeyRequest;
/// use sharing_instant::database::{Database, InMemoryDatabase};
/// use std::sync::Arc;
///
/// #[derive(Debug, Clone, Default)]
/// struct Stats {
///     total: usize,
///     active: usize,
/// }
///
/// struct StatsRequest;
///
/// impl FetchKeyRequest for StatsRequest {
///     type Value = Stats;
///
///     fn fetch(&self, _db: &dyn Database) -> sharing_instant::error::Result<Stats> {
///         Ok(Stats::default())
///     }
///
///     fn queries(&self) -> Vec<sharing_instant::Value> {
///         vec![]
///     }
/// }
///
/// let db = Arc::new(InMemoryDatabase::new());
/// let fetch = Fetch::new(StatsRequest, db);
///
/// assert_eq!(fetch.get().total, 0);
/// ```
pub struct Fetch<R: FetchKeyRequest> {
    value: Arc<RwLock<R::Value>>,
    sender: watch::Sender<R::Value>,
    receiver: watch::Receiver<R::Value>,
    request: Arc<R>,
    db: Arc<dyn Database>,
    is_loading: Arc<RwLock<bool>>,
    load_error: Arc<RwLock<Option<SharingInstantError>>>,
}

impl<R: FetchKeyRequest> Fetch<R>
where
    R::Value: Default,
{
    /// Create a new Fetch with the given request and database.
    pub fn new(request: R, db: Arc<dyn Database>) -> Self {
        let default = R::Value::default();
        let (sender, receiver) = watch::channel(default.clone());
        let value = Arc::new(RwLock::new(default));
        let is_loading = Arc::new(RwLock::new(true));
        let load_error = Arc::new(RwLock::new(None));
        let request = Arc::new(request);

        let mut fetch = Self {
            value,
            sender,
            receiver,
            request,
            db,
            is_loading,
            load_error,
        };

        fetch.load_sync();
        fetch.setup_subscriptions();
        fetch
    }

    /// Get the current combined result.
    pub fn get(&self) -> R::Value {
        self.value.read().clone()
    }

    /// Whether the initial load is still in progress.
    pub fn is_loading(&self) -> bool {
        *self.is_loading.read()
    }

    /// Get a reactive watch receiver.
    pub fn watch(&self) -> watch::Receiver<R::Value> {
        self.receiver.clone()
    }

    /// Create a read-only SharedReader view.
    pub fn reader(&self) -> SharedReader<R::Value> {
        SharedReader::from_watch(self.get(), self.receiver.clone())
    }

    fn load_sync(&mut self) {
        *self.is_loading.write() = true;

        match self.request.fetch(self.db.as_ref()) {
            Ok(result) => {
                *self.value.write() = result.clone();
                let _ = self.sender.send(result);
                *self.load_error.write() = None;
            }
            Err(e) => {
                *self.load_error.write() = Some(e);
            }
        }

        *self.is_loading.write() = false;
    }

    fn setup_subscriptions(&self) {
        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            let queries = self.request.queries();
            for q in queries {
                if let Ok(mut rx) = self.db.subscribe(&q) {
                    let value = self.value.clone();
                    let sender = self.sender.clone();
                    let request = self.request.clone();
                    let db = self.db.clone();

                    handle.spawn(async move {
                        while rx.changed().await.is_ok() {
                            if let Ok(result) = request.fetch(db.as_ref()) {
                                *value.write() = result.clone();
                                if sender.send(result).is_err() {
                                    break;
                                }
                            }
                        }
                    });
                }
            }
        }
    }
}
