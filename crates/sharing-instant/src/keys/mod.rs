//! Built-in persistence key implementations.
//!
//! - [`instant_db_key`]: Persists to InstantDB (the primary strategy).
//! - [`in_memory_key`]: In-process sharing without persistence.
//! - [`file_storage_key`]: File system persistence.

pub mod file_storage_key;
pub mod in_memory_key;
pub mod instant_db_key;
