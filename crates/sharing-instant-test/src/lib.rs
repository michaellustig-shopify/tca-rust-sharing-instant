//! ┌─────────────────────────────────────────────────────┐
//! │  SHARING INSTANT TEST SUPPORT                        │
//! │  Test utilities and helpers                           │
//! ├─────────────────────────────────────────────────────┤
//! │                                                      │
//! │  Provides:                                           │
//! │    • Test schema definitions (Reminder, etc.)         │
//! │    • InMemoryDatabase setup helpers                   │
//! │    • Assertion utilities for queries                  │
//! │    • Mock sync engine for offline tests               │
//! │                                                      │
//! │  Mirrors Swift's SQLiteDataTestSupport module.        │
//! │                                                      │
//! ├─────────────────────────────────────────────────────┤
//! │  WHY: Shared test infrastructure avoids duplication   │
//! │  and provides consistent test patterns.               │
//! │                                                      │
//! │  TESTED BY: N/A (this IS the test support)           │
//! │                                                      │
//! │  CHANGELOG:                                          │
//! │  • v0.1.0 — Initial test support crate               │
//! │                                                      │
//! │  HISTORY: git log --oneline --follow -- crates/sharing-instant-test/ │
//! └─────────────────────────────────────────────────────┘

use sharing_instant::database::InMemoryDatabase;
use sharing_instant::table::{ColumnDef, Table};
use std::sync::Arc;

/// A test Reminder type mirroring the Swift test schema.
///
/// ```
/// use sharing_instant_test::Reminder;
/// use sharing_instant::table::Table;
///
/// assert_eq!(Reminder::TABLE_NAME, "reminders");
/// ```
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Reminder {
    pub id: String,
    pub title: String,
    pub is_completed: bool,
    pub priority: Option<i64>,
    pub reminders_list_id: String,
}

impl Table for Reminder {
    const TABLE_NAME: &'static str = "reminders";

    fn columns() -> &'static [ColumnDef] {
        &[
            ColumnDef {
                name: "id",
                rust_type: "String",
                value_type: "string",
                is_optional: false,
                is_primary_key: true,
                is_unique: true,
                is_indexed: true,
            },
            ColumnDef {
                name: "title",
                rust_type: "String",
                value_type: "string",
                is_optional: false,
                is_primary_key: false,
                is_unique: false,
                is_indexed: false,
            },
            ColumnDef {
                name: "isCompleted",
                rust_type: "bool",
                value_type: "boolean",
                is_optional: false,
                is_primary_key: false,
                is_unique: false,
                is_indexed: false,
            },
            ColumnDef {
                name: "priority",
                rust_type: "Option<i64>",
                value_type: "number",
                is_optional: true,
                is_primary_key: false,
                is_unique: false,
                is_indexed: false,
            },
            ColumnDef {
                name: "remindersListId",
                rust_type: "String",
                value_type: "string",
                is_optional: false,
                is_primary_key: false,
                is_unique: false,
                is_indexed: false,
            },
        ]
    }
}

/// A test RemindersList type mirroring the Swift test schema.
///
/// ```
/// use sharing_instant_test::RemindersList;
/// use sharing_instant::table::Table;
///
/// assert_eq!(RemindersList::TABLE_NAME, "reminders_lists");
/// ```
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct RemindersList {
    pub id: String,
    pub title: String,
    pub color: Option<String>,
}

impl Table for RemindersList {
    const TABLE_NAME: &'static str = "reminders_lists";

    fn columns() -> &'static [ColumnDef] {
        &[
            ColumnDef {
                name: "id",
                rust_type: "String",
                value_type: "string",
                is_optional: false,
                is_primary_key: true,
                is_unique: true,
                is_indexed: true,
            },
            ColumnDef {
                name: "title",
                rust_type: "String",
                value_type: "string",
                is_optional: false,
                is_primary_key: false,
                is_unique: false,
                is_indexed: false,
            },
            ColumnDef {
                name: "color",
                rust_type: "Option<String>",
                value_type: "string",
                is_optional: true,
                is_primary_key: false,
                is_unique: false,
                is_indexed: false,
            },
        ]
    }
}

/// Create a fresh in-memory database for testing.
///
/// ```
/// use sharing_instant_test::test_db;
/// let db = test_db();
/// ```
pub fn test_db() -> Arc<InMemoryDatabase> {
    Arc::new(InMemoryDatabase::new())
}
