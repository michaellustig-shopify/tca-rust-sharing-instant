//! ┌─────────────────────────────────────────────────────┐
//! │  SHARING INSTANT                                     │
//! │  Reactive data persistence powered by InstantDB      │
//! ├─────────────────────────────────────────────────────┤
//! │                                                      │
//! │  ┌──────────┐    ┌──────────┐    ┌──────────────┐   │
//! │  │ Shared<T>│───►│SharedKey │───►│ InstantDB    │   │
//! │  │ wrapper  │    │ trait    │    │ EAV Store    │   │
//! │  └──────────┘    └──────────┘    └──────┬───────┘   │
//! │                                         │            │
//! │  ┌──────────┐    ┌──────────┐    ┌──────▼───────┐   │
//! │  │FetchAll  │───►│Subscribe │───►│ WebSocket    │   │
//! │  │FetchOne  │    │ Stream   │    │ Reactor      │   │
//! │  │Fetch     │    │          │    │ (real-time)  │   │
//! │  └──────────┘    └──────────┘    └──────────────┘   │
//! │                                                      │
//! ├─────────────────────────────────────────────────────┤
//! │  WHY: Port of Point-Free's SQLiteData + Sharing      │
//! │  libraries to Rust, with InstantDB as the backing    │
//! │  store instead of SQLite. InstantDB provides         │
//! │  real-time sync for free via WebSocket reactor.       │
//! │                                                      │
//! │  ALTERNATIVES: SQLite via rusqlite (no real-time),   │
//! │  SurrealDB (too heavy), custom EAV (reinventing).    │
//! │                                                      │
//! │  TESTED BY: tests/ directory mirrors Swift tests      │
//! │  EDGE CASES: offline mode, reconnection, conflicts   │
//! │                                                      │
//! │  CHANGELOG:                                          │
//! │  • v0.1.0 — Initial port from Swift SQLiteData 1.6   │
//! │                                                      │
//! │  HISTORY: git log --oneline --follow -- crates/sharing-instant/ │
//! └─────────────────────────────────────────────────────┘

pub mod auth;
pub mod connection_state;
pub mod database;
pub mod error;
pub mod fetch;
pub mod fetch_all;
pub mod fetch_key_request;
pub mod fetch_one;
pub mod instant;
pub mod keys;
pub mod mutation_callbacks;
pub mod mutations;
pub mod operation_state;
pub mod rooms;
pub mod shared;
pub mod shared_key;
pub mod shared_reader;
pub mod shared_reader_key;
pub mod subscription;
pub mod sync;
pub mod table;
pub mod topics;

// Re-exports for ergonomic API
pub use auth::{AuthCoordinator, AuthState, AuthUser};
pub use connection_state::ConnectionState;
pub use database::{Database, DefaultDatabase, LiveDatabase};
pub use error::SharingInstantError;
pub use fetch::Fetch;
pub use fetch_all::FetchAll;
pub use fetch_key_request::FetchKeyRequest;
pub use fetch_one::FetchOne;
pub use instant::InstantDB;
pub use keys::in_memory_key::InMemoryKey;
pub use keys::instant_db_key::InstantDbKey;
pub use mutation_callbacks::MutationCallbacks;
pub use mutations::Mutator;
pub use operation_state::OperationState;
pub use rooms::{PresenceData, PresenceState, Room};
pub use shared::Shared;
pub use shared_key::SharedKey;
pub use shared_reader::SharedReader;
pub use shared_reader_key::SharedReaderKey;
pub use subscription::SharedSubscription;
pub use table::Table;
pub use topics::{PublishHandle, TopicChannel, TopicEvent};

// Re-export macros
pub use sharing_instant_macros::Table as DeriveTable;

// Re-export InstantDB types we rely on
pub use instant_core::value::Value;
