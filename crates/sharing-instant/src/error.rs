//! ┌─────────────────────────────────────────────────────┐
//! │  ERROR TYPES                                         │
//! │  Unified error hierarchy for sharing-instant          │
//! ├─────────────────────────────────────────────────────┤
//! │                                                      │
//! │  SharingInstantError                                 │
//! │    ├── NotFound          (fetch returned no rows)    │
//! │    ├── ConnectionFailed  (WebSocket/network)         │
//! │    ├── QueryFailed       (bad query or schema)       │
//! │    ├── TransactionFailed (write rejected)            │
//! │    ├── SerializationError(Value ↔ Rust type)         │
//! │    ├── SubscriptionError (stream broken)             │
//! │    ├── KeyError          (SharedKey load/save)       │
//! │    ├── RoomError         (room join/leave/presence)  │
//! │    ├── TopicError        (topic sub/publish)         │
//! │    └── AuthError         (auth flow failures)        │
//! │                                                      │
//! ├─────────────────────────────────────────────────────┤
//! │  WHY: Single error type avoids scattered error       │
//! │  handling. Maps cleanly to Swift's error patterns.   │
//! │                                                      │
//! │  TESTED BY: tests/error_tests.rs                     │
//! │                                                      │
//! │  CHANGELOG:                                          │
//! │  • v0.1.0 — Initial error types                      │
//! │                                                      │
//! │  HISTORY: git log --oneline --follow -- crates/sharing-instant/src/error.rs │
//! └─────────────────────────────────────────────────────┘

use thiserror::Error;

/// Unified error type for all sharing-instant operations.
///
/// Maps to Swift's error handling patterns where `withErrorReporting`
/// catches and reports database errors without crashing.
///
/// # Example
///
/// ```
/// use sharing_instant::error::SharingInstantError;
///
/// let err = SharingInstantError::NotFound {
///     entity: "Reminder".to_string(),
///     query: "id = abc123".to_string(),
/// };
/// assert!(matches!(err, SharingInstantError::NotFound { .. }));
/// ```
#[derive(Error, Debug)]
pub enum SharingInstantError {
    /// A fetch query returned no results when at least one was expected.
    ///
    /// Analogous to Swift's `@FetchOne` throwing when the non-optional
    /// variant finds no matching row.
    #[error("not found: {entity} matching {query}")]
    NotFound { entity: String, query: String },

    /// The database connection could not be established.
    ///
    /// For InstantDB, this typically means the WebSocket connection
    /// to the server failed or the app ID is invalid.
    #[error("connection failed: {0}")]
    ConnectionFailed(String),

    /// A query was malformed or referenced unknown schema elements.
    #[error("query failed: {0}")]
    QueryFailed(String),

    /// A transaction (create/update/delete) was rejected by the server.
    #[error("transaction failed: {0}")]
    TransactionFailed(String),

    /// Could not convert between Rust types and InstantDB `Value`.
    #[error("serialization error: {0}")]
    SerializationError(String),

    /// A reactive subscription stream encountered an error.
    #[error("subscription error: {0}")]
    SubscriptionError(String),

    /// A `SharedKey` load or save operation failed.
    #[error("key error: {0}")]
    KeyError(String),

    /// A room operation (join, leave, set_presence) failed.
    #[error("room error: {0}")]
    RoomError(String),

    /// A topic operation (subscribe, publish, broadcast) failed.
    #[error("topic error: {0}")]
    TopicError(String),

    /// An authentication operation failed.
    #[error("auth error: {0}")]
    AuthError(String),
}

/// Result type alias for sharing-instant operations.
pub type Result<T> = std::result::Result<T, SharingInstantError>;
