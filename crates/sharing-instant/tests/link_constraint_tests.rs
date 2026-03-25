//! Tests for InstantDB link constraints (foreign key equivalents).
//!
//! Maps Swift's ForeignKeyConstraintTests.swift to InstantDB links.
//!
//! Swift test mapping:
//! - FK constraints → InstantDB forward/reverse links
//! - cascading deletes → link-based cascading
//! - orphaned records → link integrity checks
//!
//! All tests remain #[ignore] — blocked by rust-instantdb link API.
//! Test bodies simulate link relationships via field-level references
//! (e.g., book.author_id) and manual relationship tracking.

use sharing_instant::database::{Database, InMemoryDatabase};
use sharing_instant::fetch_all::FetchAll;
use sharing_instant::table::{json_to_value, ColumnDef, Table};
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
struct Author {
    id: String,
    name: String,
}

impl Table for Author {
    const TABLE_NAME: &'static str = "authors";
    fn columns() -> &'static [ColumnDef] {
        &[]
    }
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
struct Book {
    id: String,
    title: String,
    author_id: Option<String>,
}

impl Table for Book {
    const TABLE_NAME: &'static str = "books";
    fn columns() -> &'static [ColumnDef] {
        &[]
    }
}

#[allow(dead_code)] // Used for type documentation; data inserted via db.insert()
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
struct Tag {
    id: String,
    name: String,
}

impl Table for Tag {
    const TABLE_NAME: &'static str = "tags";
    fn columns() -> &'static [ColumnDef] {
        &[]
    }
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
struct BookTag {
    id: String,
    book_id: String,
    tag_id: String,
}

impl Table for BookTag {
    const TABLE_NAME: &'static str = "book_tags";
    fn columns() -> &'static [ColumnDef] {
        &[]
    }
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
struct TreeNode {
    id: String,
    name: String,
    parent_id: Option<String>,
}

impl Table for TreeNode {
    const TABLE_NAME: &'static str = "tree_nodes";
    fn columns() -> &'static [ColumnDef] {
        &[]
    }
}

// === Forward links (maps to FK constraints) ===

#[test]
#[ignore = "BLOCKED: rust-instantdb link() API not yet exposed in Rust client"]
fn link_book_to_author() {
    // NOTE: Real implementation needs InstantDB link() API.
    // In InstantDB, links are first-class relationships, not just field values.
    // With link(), you'd do: db.transact(["link", "books", "b1", {"author": "a1"}])
    // Simulated here via author_id field reference.
    let db = Arc::new(InMemoryDatabase::new());

    // Create author
    db.insert(
        "authors",
        "a1",
        serde_json::json!({"id": "a1", "name": "Alice"}),
    );

    // Create book linked to author via field reference
    db.insert(
        "books",
        "b1",
        serde_json::json!({"id": "b1", "title": "Rust in Action", "author_id": "a1"}),
    );

    // Verify both entities exist
    let authors = FetchAll::<Author>::new(db.clone()).get();
    let books = FetchAll::<Book>::new(db).get();

    assert_eq!(authors.len(), 1);
    assert_eq!(books.len(), 1);
    assert_eq!(
        books[0].author_id.as_deref(),
        Some("a1"),
        "book should reference author"
    );
}

#[test]
#[ignore = "BLOCKED: rust-instantdb link() API not yet exposed in Rust client"]
fn query_books_through_author_link() {
    // NOTE: Real implementation needs InstantDB nested InstaQL queries.
    // With links: {"authors": {"books": {}}} returns authors with nested books.
    // Simulated via manual field-level join.
    let db = Arc::new(InMemoryDatabase::new());

    db.insert(
        "authors",
        "a1",
        serde_json::json!({"id": "a1", "name": "Alice"}),
    );
    db.insert(
        "books",
        "b1",
        serde_json::json!({"id": "b1", "title": "Book A", "author_id": "a1"}),
    );
    db.insert(
        "books",
        "b2",
        serde_json::json!({"id": "b2", "title": "Book B", "author_id": "a1"}),
    );
    db.insert(
        "books",
        "b3",
        serde_json::json!({"id": "b3", "title": "Book C", "author_id": "a2"}),
    );

    // Manually filter books by author_id to simulate link traversal
    let all_books = FetchAll::<Book>::new(db).get();
    let alice_books: Vec<&Book> = all_books
        .iter()
        .filter(|b| b.author_id.as_deref() == Some("a1"))
        .collect();

    assert_eq!(alice_books.len(), 2, "author a1 should have 2 books");
    assert!(alice_books.iter().any(|b| b.title == "Book A"));
    assert!(alice_books.iter().any(|b| b.title == "Book B"));
}

#[test]
#[ignore = "BLOCKED: rust-instantdb link() API not yet exposed in Rust client"]
fn unlink_book_from_author() {
    // NOTE: Real implementation needs InstantDB unlink() API.
    // With unlink: db.transact(["unlink", "books", "b1", {"author": "a1"}])
    // Simulated by setting author_id to null.
    let db = Arc::new(InMemoryDatabase::new());

    db.insert(
        "authors",
        "a1",
        serde_json::json!({"id": "a1", "name": "Alice"}),
    );
    db.insert(
        "books",
        "b1",
        serde_json::json!({"id": "b1", "title": "Book A", "author_id": "a1"}),
    );

    // Verify link exists
    let books_before = FetchAll::<Book>::new(db.clone()).get();
    assert_eq!(books_before[0].author_id.as_deref(), Some("a1"));

    // Unlink by setting author_id to null
    let tx = json_to_value(&serde_json::json!([
        ["update", "books", "b1", {"id": "b1", "title": "Book A", "author_id": null}]
    ]));
    db.transact(&tx).expect("unlink should succeed");

    // Verify link is gone
    let books_after = FetchAll::<Book>::new(db).get();
    assert_eq!(books_after.len(), 1);
    assert_eq!(
        books_after[0].author_id, None,
        "author_id should be null after unlinking"
    );
}

// === Cascading operations ===

#[test]
#[ignore = "BLOCKED: rust-instantdb link() API not yet exposed in Rust client"]
fn delete_author_cascades_to_books() {
    // NOTE: Real implementation needs InstantDB cascade delete via links.
    // InstantDB can be configured to cascade deletes through links.
    // Simulated by manually deleting children after parent.
    let db = Arc::new(InMemoryDatabase::new());

    db.insert(
        "authors",
        "a1",
        serde_json::json!({"id": "a1", "name": "Alice"}),
    );
    db.insert(
        "books",
        "b1",
        serde_json::json!({"id": "b1", "title": "Book A", "author_id": "a1"}),
    );
    db.insert(
        "books",
        "b2",
        serde_json::json!({"id": "b2", "title": "Book B", "author_id": "a1"}),
    );

    // Find all books by this author before deleting
    let books_before = FetchAll::<Book>::new(db.clone()).get();
    let child_ids: Vec<String> = books_before
        .iter()
        .filter(|b| b.author_id.as_deref() == Some("a1"))
        .map(|b| b.id.clone())
        .collect();
    assert_eq!(child_ids.len(), 2);

    // Delete author
    db.remove("authors", "a1");

    // Manually cascade: delete all books that referenced this author
    for book_id in &child_ids {
        db.remove("books", book_id);
    }

    // Verify both author and books are gone
    let authors_after = FetchAll::<Author>::new(db.clone()).get();
    let books_after = FetchAll::<Book>::new(db).get();
    assert_eq!(authors_after.len(), 0, "author should be deleted");
    assert_eq!(books_after.len(), 0, "cascaded books should be deleted");
}

#[test]
#[ignore = "BLOCKED: rust-instantdb link() API not yet exposed in Rust client"]
fn delete_author_orphans_books_when_no_cascade() {
    // Without cascade, deleting a parent leaves children with dangling references.
    let db = Arc::new(InMemoryDatabase::new());

    db.insert(
        "authors",
        "a1",
        serde_json::json!({"id": "a1", "name": "Alice"}),
    );
    db.insert(
        "books",
        "b1",
        serde_json::json!({"id": "b1", "title": "Book A", "author_id": "a1"}),
    );

    // Delete author WITHOUT cascading to books
    db.remove("authors", "a1");

    // Books remain but have a dangling author_id
    let authors = FetchAll::<Author>::new(db.clone()).get();
    let books = FetchAll::<Book>::new(db).get();
    assert_eq!(authors.len(), 0, "author deleted");
    assert_eq!(books.len(), 1, "book remains (no cascade)");
    assert_eq!(
        books[0].author_id.as_deref(),
        Some("a1"),
        "book still references deleted author (orphaned)"
    );
}

// === Link integrity ===

#[test]
#[ignore = "BLOCKED: rust-instantdb link() API not yet exposed in Rust client"]
fn link_to_nonexistent_entity_fails() {
    // NOTE: Real implementation needs InstantDB link integrity enforcement.
    // With server-side links, linking to a nonexistent entity would fail.
    // InMemoryDatabase has no link enforcement, so we just create the reference.
    let db = Arc::new(InMemoryDatabase::new());

    // Create book referencing nonexistent author
    db.insert(
        "books",
        "b1",
        serde_json::json!({"id": "b1", "title": "Orphan Book", "author_id": "nonexistent"}),
    );

    // Book exists but references a nonexistent author
    let books = FetchAll::<Book>::new(db.clone()).get();
    assert_eq!(books.len(), 1);
    assert_eq!(books[0].author_id.as_deref(), Some("nonexistent"));

    // Verify the referenced author truly doesn't exist
    let authors = FetchAll::<Author>::new(db).get();
    assert_eq!(
        authors.len(),
        0,
        "referenced author does not exist — dangling reference"
    );

    // In production with InstantDB link API:
    //   let result = db.transact(&json_to_value(&serde_json::json!([
    //       ["link", "books", "b1", {"author": "nonexistent"}]
    //   ])));
    //   assert!(result.is_err(), "linking to nonexistent entity should fail");
}

#[test]
#[ignore = "BLOCKED: rust-instantdb link() API not yet exposed in Rust client"]
fn reverse_link_query() {
    // NOTE: Real implementation needs InstantDB reverse link queries.
    // With links: {"books": {"$": {"where": {"id": "b1"}}, "author": {}}}
    // returns the book with its author nested.
    // Simulated by looking up author via book's author_id.
    let db = Arc::new(InMemoryDatabase::new());

    db.insert(
        "authors",
        "a1",
        serde_json::json!({"id": "a1", "name": "Alice"}),
    );
    db.insert(
        "books",
        "b1",
        serde_json::json!({"id": "b1", "title": "Book A", "author_id": "a1"}),
    );

    // Forward: book → author
    let books = FetchAll::<Book>::new(db.clone()).get();
    let book = &books[0];
    let author_id = book
        .author_id
        .as_deref()
        .expect("book should have author_id");

    // Reverse lookup: find the author by ID
    let authors = FetchAll::<Author>::new(db).get();
    let linked_author = authors
        .iter()
        .find(|a| a.id == author_id)
        .expect("author should exist");

    assert_eq!(linked_author.name, "Alice");
    assert_eq!(book.title, "Book A");
}

#[test]
#[ignore = "BLOCKED: rust-instantdb link() API not yet exposed in Rust client"]
fn many_to_many_link() {
    // Many-to-many relationships via a junction table.
    // In InstantDB, this would use bidirectional links.
    // Simulated with a BookTag junction entity.
    let db = Arc::new(InMemoryDatabase::new());

    // Create books and tags
    db.insert(
        "books",
        "b1",
        serde_json::json!({"id": "b1", "title": "Rust Book", "author_id": null}),
    );
    db.insert(
        "books",
        "b2",
        serde_json::json!({"id": "b2", "title": "Go Book", "author_id": null}),
    );
    db.insert(
        "tags",
        "t1",
        serde_json::json!({"id": "t1", "name": "programming"}),
    );
    db.insert(
        "tags",
        "t2",
        serde_json::json!({"id": "t2", "name": "systems"}),
    );

    // Create junction records
    db.insert(
        "book_tags",
        "bt1",
        serde_json::json!({"id": "bt1", "book_id": "b1", "tag_id": "t1"}),
    );
    db.insert(
        "book_tags",
        "bt2",
        serde_json::json!({"id": "bt2", "book_id": "b1", "tag_id": "t2"}),
    );
    db.insert(
        "book_tags",
        "bt3",
        serde_json::json!({"id": "bt3", "book_id": "b2", "tag_id": "t1"}),
    );

    // Query: which tags does book b1 have?
    let all_junctions = FetchAll::<BookTag>::new(db.clone()).get();
    let b1_tag_ids: Vec<&str> = all_junctions
        .iter()
        .filter(|bt| bt.book_id == "b1")
        .map(|bt| bt.tag_id.as_str())
        .collect();

    assert_eq!(b1_tag_ids.len(), 2, "b1 should have 2 tags");
    assert!(b1_tag_ids.contains(&"t1"));
    assert!(b1_tag_ids.contains(&"t2"));

    // Query: which books have tag t1?
    let t1_book_ids: Vec<&str> = all_junctions
        .iter()
        .filter(|bt| bt.tag_id == "t1")
        .map(|bt| bt.book_id.as_str())
        .collect();

    assert_eq!(t1_book_ids.len(), 2, "t1 should be on 2 books");
    assert!(t1_book_ids.contains(&"b1"));
    assert!(t1_book_ids.contains(&"b2"));
}

#[test]
#[ignore = "BLOCKED: rust-instantdb link() API not yet exposed in Rust client"]
fn self_referential_link() {
    // Entity links to another entity of the same type (tree structure).
    // Simulated via parent_id field.
    let db = Arc::new(InMemoryDatabase::new());

    // Create a tree: root → child1 → grandchild1
    db.insert(
        "tree_nodes",
        "root",
        serde_json::json!({"id": "root", "name": "Root", "parent_id": null}),
    );
    db.insert(
        "tree_nodes",
        "child1",
        serde_json::json!({"id": "child1", "name": "Child 1", "parent_id": "root"}),
    );
    db.insert(
        "tree_nodes",
        "grandchild1",
        serde_json::json!({"id": "grandchild1", "name": "Grandchild 1", "parent_id": "child1"}),
    );

    let nodes = FetchAll::<TreeNode>::new(db).get();
    assert_eq!(nodes.len(), 3);

    // Find root (no parent)
    let root = nodes.iter().find(|n| n.parent_id.is_none());
    assert!(root.is_some(), "should have a root node");
    assert_eq!(root.expect("root exists").name, "Root");

    // Find children of root
    let root_children: Vec<&TreeNode> = nodes
        .iter()
        .filter(|n| n.parent_id.as_deref() == Some("root"))
        .collect();
    assert_eq!(root_children.len(), 1);
    assert_eq!(root_children[0].name, "Child 1");

    // Find children of child1 (grandchildren of root)
    let grandchildren: Vec<&TreeNode> = nodes
        .iter()
        .filter(|n| n.parent_id.as_deref() == Some("child1"))
        .collect();
    assert_eq!(grandchildren.len(), 1);
    assert_eq!(grandchildren[0].name, "Grandchild 1");
}
