//! ┌─────────────────────────────────────────────────────┐
//! │  FETCH KEY REQUEST                                   │
//! │  Custom multi-query fetch abstraction                 │
//! ├─────────────────────────────────────────────────────┤
//! │                                                      │
//! │  FetchKeyRequest                                     │
//! │    └── fetch(db) → Value                             │
//! │                                                      │
//! │  Allows combining multiple queries into a single     │
//! │  atomic fetch, like Swift's FetchKeyRequest protocol. │
//! │                                                      │
//! ├─────────────────────────────────────────────────────┤
//! │  WHY: Mirrors Swift's FetchKeyRequest. Enables       │
//! │  complex queries that combine multiple data sources  │
//! │  into a single reactive observation.                 │
//! │                                                      │
//! │  TESTED BY: tests/fetch_tests.rs                     │
//! │                                                      │
//! │  CHANGELOG:                                          │
//! │  • v0.1.0 — Initial trait definition                 │
//! │                                                      │
//! │  HISTORY: git log --oneline --follow -- crates/sharing-instant/src/fetch_key_request.rs │
//! └─────────────────────────────────────────────────────┘

use crate::database::Database;
use crate::error::Result;
use std::sync::Arc;

/// Custom fetch request for combining multiple queries.
///
/// Mirrors Swift's `FetchKeyRequest` protocol. Use this when you
/// need to fetch multiple related pieces of data in a single
/// atomic operation.
///
/// # Example
///
/// ```
/// use sharing_instant::fetch_key_request::FetchKeyRequest;
/// use sharing_instant::database::{Database, InMemoryDatabase};
/// use std::sync::Arc;
///
/// #[derive(Debug, Clone, Default)]
/// struct DashboardData {
///     total_reminders: usize,
///     completed_count: usize,
/// }
///
/// struct DashboardRequest;
///
/// impl FetchKeyRequest for DashboardRequest {
///     type Value = DashboardData;
///
///     fn fetch(&self, _db: &dyn Database) -> sharing_instant::error::Result<DashboardData> {
///         // In practice, combine multiple queries here
///         Ok(DashboardData::default())
///     }
///
///     fn queries(&self) -> Vec<sharing_instant::Value> {
///         vec![]
///     }
/// }
/// ```
pub trait FetchKeyRequest: Send + Sync + 'static {
    /// The combined result type of the fetch.
    type Value: Send + Sync + Clone + 'static;

    /// Execute the fetch against the database.
    ///
    /// This is where you combine multiple queries into a single
    /// result. The database reference provides both read and write
    /// access (though fetches should typically be read-only).
    fn fetch(&self, db: &dyn Database) -> Result<Self::Value>;

    /// Return the queries that this request depends on.
    ///
    /// Used by the subscription system to know which InstantDB
    /// queries to watch for changes. When any of these queries
    /// produce new results, the full `fetch()` is re-executed.
    fn queries(&self) -> Vec<instant_core::value::Value>;
}
