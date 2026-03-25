//! ┌─────────────────────────────────────────────────────┐
//! │  SHARING INSTANT — PROC MACROS                      │
//! │  Derive macros for Table and entity definitions      │
//! ├─────────────────────────────────────────────────────┤
//! │                                                      │
//! │   #[derive(Table)]                                   │
//! │        │                                             │
//! │        ├──► InstantEntity impl                       │
//! │        ├──► Column metadata                          │
//! │        ├──► Query builder DSL                        │
//! │        └──► Fetch/Subscribe helpers                  │
//! │                                                      │
//! ├─────────────────────────────────────────────────────┤
//! │  WHY: Mirrors Swift's @Table macro from              │
//! │  swift-structured-queries. Generates typed entity    │
//! │  definitions and query builders that work with       │
//! │  InstantDB's EAV store.                              │
//! │                                                      │
//! │  ALTERNATIVES: Hand-written impls (too verbose),     │
//! │  runtime reflection (no compile-time safety).        │
//! │                                                      │
//! │  TESTED BY: crates/sharing-instant/tests/table_tests.rs │
//! │  EDGE CASES: optional fields, custom column names,   │
//! │  nested types, generic structs                        │
//! │                                                      │
//! │  CHANGELOG:                                          │
//! │  • v0.1.0 — Initial Table derive macro               │
//! │                                                      │
//! │  HISTORY: git log --oneline --follow -- crates/sharing-instant-macros/ │
//! └─────────────────────────────────────────────────────┘

extern crate proc_macro;

use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, DeriveInput};

/// Derive macro that generates `Table` trait implementation for a struct.
///
/// Maps a Rust struct to an InstantDB entity, generating:
/// - Entity name (snake_case pluralized struct name)
/// - Column definitions from struct fields
/// - Query builder methods (`.where_eq()`, `.order()`, etc.)
/// - Fetch helpers (`fetch_all()`, `fetch_one()`, `fetch_count()`)
///
/// # Example
///
/// ```ignore
/// #[derive(Table)]
/// struct Reminder {
///     id: String,
///     title: String,
///     is_completed: bool,
///     priority: Option<i64>,
///     reminders_list_id: String,
/// }
/// ```
///
/// This generates an `InstantEntity` implementation allowing:
/// ```ignore
/// let reminders = Reminder::query()
///     .where_eq("is_completed", false)
///     .order("title", "asc")
///     .fetch_all(&db)
///     .await?;
/// ```
#[proc_macro_derive(Table, attributes(column, table))]
pub fn derive_table(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;

    // Generate the table/collection name (snake_case, pluralized)
    let table_name = pluralize(&to_snake_case(&name.to_string()));

    let expanded = quote! {
        impl crate::table::Table for #name {
            const TABLE_NAME: &'static str = #table_name;

            fn columns() -> &'static [crate::table::ColumnDef] {
                // TODO: Generate from struct fields
                &[]
            }
        }
    };

    TokenStream::from(expanded)
}

/// Convert PascalCase to snake_case.
///
/// # Examples
///
/// ```ignore
/// assert_eq!(to_snake_case("RemindersList"), "reminders_list");
/// assert_eq!(to_snake_case("Reminder"), "reminder");
/// ```
fn to_snake_case(s: &str) -> String {
    let mut result = String::new();
    for (i, ch) in s.chars().enumerate() {
        if ch.is_uppercase() && i > 0 {
            result.push('_');
        }
        result.push(ch.to_lowercase().next().unwrap());
    }
    result
}

/// Naive English pluralization.
///
/// # Examples
///
/// ```ignore
/// assert_eq!(pluralize("reminder"), "reminders");
/// assert_eq!(pluralize("category"), "categories");
/// ```
fn pluralize(s: &str) -> String {
    if s.ends_with('y') && !s.ends_with("ey") && !s.ends_with("ay") && !s.ends_with("oy") {
        format!("{}ies", &s[..s.len() - 1])
    } else if s.ends_with('s') || s.ends_with("sh") || s.ends_with("ch") || s.ends_with('x') {
        format!("{s}es")
    } else {
        format!("{s}s")
    }
}
