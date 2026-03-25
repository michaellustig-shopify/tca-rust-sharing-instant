//! ┌─────────────────────────────────────────────────────┐
//! │  SYNC ENGINE                                         │
//! │  Wraps InstantDB's reactor for high-level sync API   │
//! ├─────────────────────────────────────────────────────┤
//! │                                                      │
//! │  SyncEngine                                          │
//! │    ├── start()  → connect WebSocket via Reactor       │
//! │    ├── stop()   → disconnect                         │
//! │    ├── status() → SyncStatus                         │
//! │    ├── database() → Arc<dyn Database>                │
//! │    └── on_status_change() → Stream<SyncStatus>       │
//! │                                                      │
//! │  Mirrors Swift's SyncEngine observable properties:   │
//! │    isSendingChanges, isReceivingChanges, accountStatus│
//! │                                                      │
//! ├─────────────────────────────────────────────────────┤
//! │  WHY: Provides a familiar API layer on top of        │
//! │  InstantDB's reactor. Developers coming from Swift   │
//! │  expect SyncEngine-like semantics.                   │
//! │                                                      │
//! │  TESTED BY: tests/sync_engine_tests.rs               │
//! │                                                      │
//! │  CHANGELOG:                                          │
//! │  • v0.1.0 — Initial SyncEngine wrapper               │
//! │  • v0.2.0 — Wired to real Reactor + LiveDatabase     │
//! │                                                      │
//! │  HISTORY: git log --oneline --follow -- crates/sharing-instant/src/sync/engine.rs │
//! └─────────────────────────────────────────────────────┘

use crate::connection_state::ConnectionState;
use crate::database::{Database, LiveDatabase};
use crate::rooms::{PresenceData, Room};
use crate::shared_reader::SharedReader;
use crate::topics::TopicChannel;
use instant_client::{ConnectionConfig, Reactor};
use parking_lot::RwLock;
use serde::{de::DeserializeOwned, Serialize};
use std::sync::Arc;
use tokio::sync::watch;

/// Current synchronization status.
///
/// Mirrors Swift's SyncEngine observable properties.
///
/// # Example
///
/// ```
/// use sharing_instant::sync::engine::SyncStatus;
///
/// let status = SyncStatus::default();
/// assert!(!status.is_connected);
/// ```
#[derive(Debug, Clone, Default)]
pub struct SyncStatus {
    /// Whether the WebSocket connection is active.
    pub is_connected: bool,
    /// Whether we're currently sending local changes to the server.
    pub is_sending_changes: bool,
    /// Whether we're currently receiving remote changes.
    pub is_receiving_changes: bool,
    /// The session ID from the server, if connected.
    pub session_id: Option<String>,
    /// Last successful sync timestamp.
    pub last_sync_at: Option<std::time::Instant>,
}

/// High-level sync manager wrapping InstantDB's reactor.
///
/// Owns the Reactor and provides a LiveDatabase for query/transact/subscribe.
/// Supports stop/start cycles — each `start()` creates a fresh Reactor since
/// Reactor can only be started once.
///
/// # Example
///
/// ```
/// # tokio::runtime::Runtime::new().unwrap().block_on(async {
/// use sharing_instant::sync::engine::{SyncEngine, SyncConfig};
///
/// let config = SyncConfig {
///     app_id: "my-app-id".to_string(),
///     ..Default::default()
/// };
///
/// let engine = SyncEngine::new(config);
/// assert!(!engine.status().is_connected);
/// # });
/// ```
pub struct SyncEngine {
    config: SyncConfig,
    /// Current reactor, swapped on each start() cycle.
    reactor: Arc<RwLock<Arc<Reactor>>>,
    live_db: Arc<LiveDatabase>,
    handle: tokio::runtime::Handle,
    status: Arc<RwLock<SyncStatus>>,
    status_sender: watch::Sender<SyncStatus>,
    status_receiver: watch::Receiver<SyncStatus>,
    conn_state_sender: watch::Sender<ConnectionState>,
    conn_state_receiver: watch::Receiver<ConnectionState>,
}

/// Configuration for the sync engine.
///
/// # Example
///
/// ```
/// use sharing_instant::sync::engine::SyncConfig;
///
/// let config = SyncConfig {
///     app_id: "my-app-id".to_string(),
///     ws_uri: "wss://api.instantdb.com/runtime/session".to_string(),
///     ..Default::default()
/// };
/// ```
#[derive(Debug, Clone)]
pub struct SyncConfig {
    /// The InstantDB application ID.
    pub app_id: String,
    /// WebSocket URI for the InstantDB server.
    pub ws_uri: String,
    /// Optional refresh token for authenticated sessions.
    pub refresh_token: Option<String>,
    /// Optional admin token for admin access.
    pub admin_token: Option<String>,
}

impl Default for SyncConfig {
    fn default() -> Self {
        Self {
            app_id: String::new(),
            ws_uri: "wss://api.instantdb.com/runtime/session".to_string(),
            refresh_token: None,
            admin_token: None,
        }
    }
}

impl SyncConfig {
    /// Convert to an instant-client ConnectionConfig.
    fn to_connection_config(&self) -> ConnectionConfig {
        let config = if let Some(ref admin_token) = self.admin_token {
            ConnectionConfig::admin(&self.app_id, admin_token)
        } else if let Some(ref refresh_token) = self.refresh_token {
            ConnectionConfig::user(&self.app_id, refresh_token)
        } else {
            // Anonymous connection with just app_id.
            ConnectionConfig::admin(&self.app_id, "")
        };
        config.with_ws_uri(&self.ws_uri)
    }
}

impl SyncEngine {
    /// Create a new sync engine with the given config.
    ///
    /// Creates the Reactor and LiveDatabase immediately. Call `start()`
    /// to establish the WebSocket connection.
    pub fn new(config: SyncConfig) -> Self {
        let handle = tokio::runtime::Handle::try_current()
            .expect("SyncEngine::new() must be called from within a tokio runtime");

        let conn_config = config.to_connection_config();
        let reactor = Arc::new(Reactor::new(conn_config));
        let live_db = Arc::new(LiveDatabase::new(reactor.clone(), handle.clone()));

        let status = SyncStatus::default();
        let (status_sender, status_receiver) = watch::channel(status.clone());
        let (conn_state_sender, conn_state_receiver) =
            watch::channel(ConnectionState::Disconnected);

        Self {
            config,
            reactor: Arc::new(RwLock::new(reactor)),
            live_db,
            handle,
            status: Arc::new(RwLock::new(status)),
            status_sender,
            status_receiver,
            conn_state_sender,
            conn_state_receiver,
        }
    }

    /// Get the current sync status.
    pub fn status(&self) -> SyncStatus {
        self.status.read().clone()
    }

    /// Get a reactive receiver for status changes.
    pub fn on_status_change(&self) -> watch::Receiver<SyncStatus> {
        self.status_receiver.clone()
    }

    /// Get the database backed by this engine's Reactor.
    pub fn database(&self) -> Arc<dyn Database> {
        self.live_db.clone()
    }

    /// Get a read-only view of the connection state.
    pub fn connection_state(&self) -> SharedReader<ConnectionState> {
        let current = self.conn_state_receiver.borrow().clone();
        SharedReader::from_watch(current, self.conn_state_receiver.clone())
    }

    /// Get the current Reactor (for room/topic operations).
    pub fn reactor(&self) -> Arc<Reactor> {
        self.reactor.read().clone()
    }

    /// Get the tokio runtime handle.
    pub fn handle(&self) -> tokio::runtime::Handle {
        self.handle.clone()
    }

    /// Get the config.
    pub fn config(&self) -> &SyncConfig {
        &self.config
    }

    /// Start the sync engine (connect to InstantDB via WebSocket).
    ///
    /// If the engine was previously stopped, creates a fresh Reactor
    /// since Reactor can only be started once.
    pub async fn start(&self) -> crate::error::Result<()> {
        let _ = self.conn_state_sender.send(ConnectionState::Connecting);

        // Create a fresh reactor each time start() is called.
        // This handles the stop/start cycle since Reactor::start() consumes
        // its internal outgoing_rx on first call.
        let conn_config = self.config.to_connection_config();
        let new_reactor = Arc::new(Reactor::new(conn_config));

        // Swap the reactor in LiveDatabase so existing references use the new one.
        self.live_db.set_reactor(new_reactor.clone());
        *self.reactor.write() = new_reactor.clone();

        new_reactor.start().await.map_err(|e| {
            let msg = e.to_string();
            let _ = self
                .conn_state_sender
                .send(ConnectionState::Error(msg.clone()));
            crate::error::SharingInstantError::ConnectionFailed(msg)
        })?;

        let _ = self.conn_state_sender.send(ConnectionState::Connected);

        // Poll for session_id to confirm connection.
        let session_id = new_reactor.session_id().await;

        if let Some(ref sid) = session_id {
            let _ = self.conn_state_sender.send(ConnectionState::Authenticated {
                session_id: sid.clone(),
            });
        }

        let mut status = self.status.write();
        status.is_connected = true;
        status.session_id = session_id;
        status.last_sync_at = Some(std::time::Instant::now());
        let new_status = status.clone();
        let _ = self.status_sender.send(new_status);

        Ok(())
    }

    /// Join a room with typed presence.
    pub fn room<P: PresenceData>(
        &self,
        room_type: &str,
        room_id: &str,
    ) -> crate::error::Result<Room<P>> {
        Room::join(self.reactor(), self.handle.clone(), room_type, room_id)
    }

    /// Subscribe to a typed topic channel within a room.
    pub fn topic<T: Serialize + DeserializeOwned + Clone + Send + Sync + 'static>(
        &self,
        room_type: &str,
        room_id: &str,
        topic: &str,
    ) -> crate::error::Result<TopicChannel<T>> {
        TopicChannel::subscribe(
            self.reactor(),
            self.handle.clone(),
            room_type,
            room_id,
            topic,
        )
    }

    /// Stop the sync engine (disconnect from InstantDB).
    pub async fn stop(&self) {
        let reactor = self.reactor.read().clone();
        reactor.stop().await;

        let _ = self.conn_state_sender.send(ConnectionState::Disconnected);

        let mut status = self.status.write();
        status.is_connected = false;
        status.session_id = None;
        let new_status = status.clone();
        let _ = self.status_sender.send(new_status);
    }
}
