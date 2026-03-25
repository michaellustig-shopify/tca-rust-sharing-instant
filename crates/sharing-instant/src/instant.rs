use crate::error::Result;
use crate::sync::engine::{SyncConfig, SyncEngine};
use crate::table::QueryBuilder;
use crate::{
    AuthCoordinator, AuthState, ConnectionState, Database, FetchAll, FetchOne, Mutator,
    PresenceData, Room, SharedReader, Table, TopicChannel,
};
use serde::{de::DeserializeOwned, Serialize};
use std::sync::Arc;

/// Top-level entry point for the InstantDB Rust SDK.
///
/// Analogous to the JS SDK's `const db = init({ appId })`. Provides
/// typed queries, mutations, rooms, topics, auth, and connection state
/// through a single struct.
///
/// # Example
///
/// ```ignore
/// let db = InstantDB::init(SyncConfig {
///     app_id: "my-app".into(),
///     admin_token: Some("token".into()),
///     ..Default::default()
/// }).await?;
///
/// // Query all todos, reactively
/// let todos = db.use_query::<Todo>();
/// println!("Got {} todos", todos.get().len());
///
/// // Mutations
/// db.tx::<Todo>().create(&todo)?;
/// db.tx::<Todo>().delete("todo-123")?;
///
/// // Rooms + Presence
/// let room = db.room::<Cursor>("editor", "doc-1")?;
/// room.set_presence(&Cursor { x: 100.0, y: 200.0 })?;
///
/// // Topics
/// let typing = db.topic::<TypingEvent>("chat", "room-1", "typing")?;
/// typing.publish(&TypingEvent { user: "Alice".into() })?;
/// ```
pub struct InstantDB {
    engine: SyncEngine,
    auth: AuthCoordinator,
}

impl InstantDB {
    /// Initialize and connect to InstantDB.
    pub async fn init(config: SyncConfig) -> Result<Self> {
        let app_id = config.app_id.clone();
        let engine = SyncEngine::new(config);
        engine.start().await?;

        let auth = AuthCoordinator::new(&app_id);

        Ok(Self { engine, auth })
    }

    /// Create without connecting (for testing or deferred connection).
    pub fn new(config: SyncConfig) -> Self {
        let app_id = config.app_id.clone();
        let engine = SyncEngine::new(config);
        let auth = AuthCoordinator::new(&app_id);
        Self { engine, auth }
    }

    /// Start the connection (if created with `new()`).
    pub async fn connect(&self) -> Result<()> {
        self.engine.start().await
    }

    /// Disconnect from InstantDB.
    pub async fn disconnect(&self) {
        self.engine.stop().await;
    }

    // --- Typed Queries ---

    /// Subscribe to all entities of type T. Returns a reactive FetchAll.
    pub fn use_query<T: Table>(&self) -> FetchAll<T> {
        FetchAll::new(self.engine.database())
    }

    /// Subscribe to entities with a custom query built via the fluent builder.
    pub fn use_query_with<T: Table>(
        &self,
        f: impl FnOnce(QueryBuilder<T>) -> QueryBuilder<T>,
    ) -> FetchAll<T> {
        let query = f(T::query()).build();
        FetchAll::with_query(self.engine.database(), query)
    }

    /// Fetch a single entity by ID.
    pub fn use_one<T: Table>(&self, id: &str) -> FetchOne<T> {
        let query = T::query().where_eq("id", id.to_string()).build();
        FetchOne::with_query(self.engine.database(), query)
    }

    // --- Typed Mutations ---

    /// Get a typed mutator for CRUD operations on an entity type.
    pub fn tx<T: Table>(&self) -> Mutator<T> {
        Mutator::new(self.engine.database())
    }

    // --- Rooms ---

    /// Join a room with typed presence.
    pub fn room<P: PresenceData>(&self, room_type: &str, room_id: &str) -> Result<Room<P>> {
        self.engine.room(room_type, room_id)
    }

    /// Subscribe to a typed topic channel within a room.
    pub fn topic<T: Serialize + DeserializeOwned + Clone + Send + Sync + 'static>(
        &self,
        room_type: &str,
        room_id: &str,
        topic: &str,
    ) -> Result<TopicChannel<T>> {
        self.engine.topic(room_type, room_id, topic)
    }

    // --- Auth ---

    /// Get the auth coordinator for sign-in flows.
    pub fn auth(&self) -> &AuthCoordinator {
        &self.auth
    }

    /// Get a read-only view of the auth state.
    pub fn auth_state(&self) -> SharedReader<AuthState> {
        self.auth.state()
    }

    /// Get a raw watch receiver for auth state changes.
    ///
    /// Useful when you need to `await` state transitions directly
    /// rather than polling the `SharedReader`.
    pub fn watch_auth_state(&self) -> tokio::sync::watch::Receiver<AuthState> {
        self.auth.watch_state()
    }

    // --- Connection ---

    /// Get a read-only view of the connection state.
    pub fn connection_state(&self) -> SharedReader<ConnectionState> {
        self.engine.connection_state()
    }

    // --- Internal access ---

    /// Get the underlying database (for advanced use).
    pub fn database(&self) -> Arc<dyn Database> {
        self.engine.database()
    }

    /// Get the underlying SyncEngine (for advanced use).
    pub fn engine(&self) -> &SyncEngine {
        &self.engine
    }
}
