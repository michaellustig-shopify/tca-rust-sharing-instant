//! Tests for link integrity violations (orphaned references).
//!
//! Maps Swift's ReferenceViolationTests.swift to InstantDB link integrity.
//!
//! All tests remain #[ignore] — blocked by rust-instantdb link API.
//! Test bodies simulate orphaned references via field-level category_id
//! references and manual cleanup patterns.

use sharing_instant::database::{Database, InMemoryDatabase};
use sharing_instant::fetch_all::FetchAll;
use sharing_instant::table::{json_to_value, ColumnDef, Table};
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
struct Category {
    id: String,
    name: String,
}

impl Table for Category {
    const TABLE_NAME: &'static str = "categories";
    fn columns() -> &'static [ColumnDef] {
        &[]
    }
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
struct Product {
    id: String,
    name: String,
    category_id: Option<String>,
}

impl Table for Product {
    const TABLE_NAME: &'static str = "products";
    fn columns() -> &'static [ColumnDef] {
        &[]
    }
}

#[test]
#[ignore = "BLOCKED: rust-instantdb link() API not yet exposed in Rust client"]
fn orphaned_link_after_parent_delete() {
    // Deleting a parent record leaves child records with a dangling reference.
    let db = Arc::new(InMemoryDatabase::new());

    db.insert(
        "categories",
        "c1",
        serde_json::json!({"id": "c1", "name": "Electronics"}),
    );
    db.insert(
        "products",
        "p1",
        serde_json::json!({"id": "p1", "name": "Laptop", "category_id": "c1"}),
    );

    // Verify the relationship before deletion
    let products_before = FetchAll::<Product>::new(db.clone()).get();
    assert_eq!(products_before[0].category_id.as_deref(), Some("c1"));

    // Delete the category (parent)
    db.remove("categories", "c1");

    // Product still exists but has an orphaned reference
    let categories = FetchAll::<Category>::new(db.clone()).get();
    let products = FetchAll::<Product>::new(db).get();

    assert_eq!(categories.len(), 0, "category should be deleted");
    assert_eq!(products.len(), 1, "product should still exist");
    assert_eq!(
        products[0].category_id.as_deref(),
        Some("c1"),
        "product still points to deleted category (orphaned reference)"
    );
}

#[test]
#[ignore = "BLOCKED: rust-instantdb link() API not yet exposed in Rust client"]
fn orphaned_link_query_returns_null() {
    // Querying through a broken link returns null/empty in the linked field.
    // Simulated by checking that the referenced category doesn't exist.
    let db = Arc::new(InMemoryDatabase::new());

    // Create product with reference, but no category
    db.insert(
        "products",
        "p1",
        serde_json::json!({"id": "p1", "name": "Orphan Product", "category_id": "c-missing"}),
    );

    let products = FetchAll::<Product>::new(db.clone()).get();
    let categories = FetchAll::<Category>::new(db).get();

    assert_eq!(products.len(), 1);
    let referenced_id = products[0]
        .category_id
        .as_deref()
        .expect("should have category_id");

    // Attempt to resolve the reference — it points to nothing
    let resolved = categories.iter().find(|c| c.id == referenced_id);
    assert!(
        resolved.is_none(),
        "orphaned link should resolve to nothing"
    );

    // In production with InstantDB link queries:
    //   let result = db.query(&json_to_value(&serde_json::json!({
    //       "products": {"$": {"where": {"id": "p1"}}, "category": {}}
    //   })));
    //   // The nested "category" would be null/empty for the orphaned link
}

#[test]
#[ignore = "BLOCKED: rust-instantdb link() API not yet exposed in Rust client"]
fn relink_orphaned_record() {
    // After a reference becomes orphaned, update it to point to a new parent.
    let db = Arc::new(InMemoryDatabase::new());

    // Create product referencing a category that will be deleted
    db.insert(
        "categories",
        "c1",
        serde_json::json!({"id": "c1", "name": "Old Category"}),
    );
    db.insert(
        "products",
        "p1",
        serde_json::json!({"id": "p1", "name": "Product", "category_id": "c1"}),
    );

    // Delete original category, orphaning the product
    db.remove("categories", "c1");
    let orphaned = FetchAll::<Product>::new(db.clone()).get();
    assert_eq!(orphaned[0].category_id.as_deref(), Some("c1"), "orphaned");

    // Create a new category and relink the product
    db.insert(
        "categories",
        "c2",
        serde_json::json!({"id": "c2", "name": "New Category"}),
    );
    let tx = json_to_value(&serde_json::json!([
        ["update", "products", "p1", {"id": "p1", "name": "Product", "category_id": "c2"}]
    ]));
    db.transact(&tx).expect("relink should succeed");

    // Verify the relink
    let relinked = FetchAll::<Product>::new(db.clone()).get();
    assert_eq!(
        relinked[0].category_id.as_deref(),
        Some("c2"),
        "product should now reference new category"
    );

    // Verify the new category exists
    let categories = FetchAll::<Category>::new(db).get();
    assert_eq!(categories.len(), 1);
    assert_eq!(categories[0].id, "c2");
}

#[test]
#[ignore = "BLOCKED: rust-instantdb link() API not yet exposed in Rust client"]
fn batch_delete_with_dependent_records() {
    // Delete multiple categories and verify all dependent products are handled.
    let db = Arc::new(InMemoryDatabase::new());

    // Create categories
    db.insert(
        "categories",
        "c1",
        serde_json::json!({"id": "c1", "name": "Electronics"}),
    );
    db.insert(
        "categories",
        "c2",
        serde_json::json!({"id": "c2", "name": "Books"}),
    );

    // Create products referencing those categories
    db.insert(
        "products",
        "p1",
        serde_json::json!({"id": "p1", "name": "Laptop", "category_id": "c1"}),
    );
    db.insert(
        "products",
        "p2",
        serde_json::json!({"id": "p2", "name": "Phone", "category_id": "c1"}),
    );
    db.insert(
        "products",
        "p3",
        serde_json::json!({"id": "p3", "name": "Novel", "category_id": "c2"}),
    );

    assert_eq!(FetchAll::<Category>::new(db.clone()).get().len(), 2);
    assert_eq!(FetchAll::<Product>::new(db.clone()).get().len(), 3);

    // Batch delete all categories
    let tx = json_to_value(&serde_json::json!([
        ["delete", "categories", "c1", {}],
        ["delete", "categories", "c2", {}]
    ]));
    db.transact(&tx).expect("batch delete should succeed");

    // Categories are gone
    assert_eq!(FetchAll::<Category>::new(db.clone()).get().len(), 0);

    // Products remain but all have orphaned references
    let products = FetchAll::<Product>::new(db.clone()).get();
    assert_eq!(products.len(), 3, "products remain without cascade");

    // All products now have dangling category_ids
    for product in &products {
        let cat_id = product
            .category_id
            .as_deref()
            .expect("product should have category_id");
        let category_exists = FetchAll::<Category>::new(db.clone())
            .get()
            .iter()
            .any(|c| c.id == cat_id);
        assert!(
            !category_exists,
            "product {} references deleted category {}",
            product.id, cat_id
        );
    }
}

#[test]
#[ignore = "BLOCKED: rust-instantdb link() API not yet exposed in Rust client"]
fn concurrent_delete_and_link_creation() {
    // Race condition: one client deletes a category while another links a
    // product to it. With InMemoryDatabase (single-process), we simulate
    // this as sequential operations.
    let db = Arc::new(InMemoryDatabase::new());

    db.insert(
        "categories",
        "c1",
        serde_json::json!({"id": "c1", "name": "Doomed Category"}),
    );

    // Client A deletes the category
    db.remove("categories", "c1");

    // Client B (unaware of deletion) creates a product referencing it
    let tx = json_to_value(&serde_json::json!([
        ["create", "products", "p1", {"id": "p1", "name": "Late Product", "category_id": "c1"}]
    ]));
    db.transact(&tx)
        .expect("create succeeds — no referential integrity in InMemoryDatabase");

    // Result: product exists with a dangling reference
    let products = FetchAll::<Product>::new(db.clone()).get();
    let categories = FetchAll::<Category>::new(db).get();

    assert_eq!(products.len(), 1, "product was created");
    assert_eq!(categories.len(), 0, "category was already deleted");
    assert_eq!(
        products[0].category_id.as_deref(),
        Some("c1"),
        "product has dangling reference to deleted category"
    );

    // In production with InstantDB link enforcement:
    //   - Server would reject the link creation if the target entity doesn't exist
    //   - Or the link would be created and then cleaned up on conflict resolution
}
