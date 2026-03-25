//! Tests for Mutator<T> and FetchAll mutation methods.

use sharing_instant::database::{Database, InMemoryDatabase};
use sharing_instant::mutations::Mutator;
use sharing_instant::table::{ColumnDef, Table};
use sharing_instant::MutationCallbacks;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
struct Todo {
    id: String,
    title: String,
    is_completed: bool,
}

impl Table for Todo {
    const TABLE_NAME: &'static str = "todos";
    fn columns() -> &'static [ColumnDef] {
        &[]
    }
}

fn setup() -> (Arc<dyn Database>, Mutator<Todo>) {
    let db: Arc<dyn Database> = Arc::new(InMemoryDatabase::new());
    let mutator = Mutator::<Todo>::new(db.clone());
    (db, mutator)
}

#[test]
fn create_and_query() {
    let (db, mutator) = setup();

    mutator
        .create(&Todo {
            id: "t1".into(),
            title: "Buy milk".into(),
            is_completed: false,
        })
        .expect("create should succeed");

    // Query the database to verify.
    let query = sharing_instant::table::json_to_value(&serde_json::json!({ "todos": {} }));
    let result = db.query(&query).expect("query should succeed");

    match result {
        sharing_instant::Value::Object(obj) => {
            let todos = obj.get("todos").expect("todos key");
            match todos {
                sharing_instant::Value::Array(arr) => assert_eq!(arr.len(), 1),
                sharing_instant::Value::Object(map) => assert_eq!(map.len(), 1),
                _ => panic!("expected array or object"),
            }
        }
        _ => panic!("expected object"),
    }
}

#[test]
fn update_existing() {
    let (db, mutator) = setup();

    mutator
        .create(&Todo {
            id: "t1".into(),
            title: "Buy milk".into(),
            is_completed: false,
        })
        .expect("create");

    mutator
        .update(
            "t1",
            &Todo {
                id: "t1".into(),
                title: "Buy eggs".into(),
                is_completed: true,
            },
        )
        .expect("update");

    let query = sharing_instant::table::json_to_value(&serde_json::json!({ "todos": {} }));
    let result = db.query(&query).expect("query");

    // Should still be 1 item but with updated fields.
    match result {
        sharing_instant::Value::Object(obj) => {
            let todos = obj.get("todos").expect("todos key");
            match todos {
                sharing_instant::Value::Array(arr) => assert_eq!(arr.len(), 1),
                sharing_instant::Value::Object(map) => assert_eq!(map.len(), 1),
                _ => panic!("expected array or object"),
            }
        }
        _ => panic!("expected object"),
    }
}

#[test]
fn delete_existing() {
    let (db, mutator) = setup();

    mutator
        .create(&Todo {
            id: "t1".into(),
            title: "Buy milk".into(),
            is_completed: false,
        })
        .expect("create");

    mutator.delete("t1").expect("delete");

    let query = sharing_instant::table::json_to_value(&serde_json::json!({ "todos": {} }));
    let result = db.query(&query).expect("query");

    match result {
        sharing_instant::Value::Object(obj) => {
            let todos = obj.get("todos").expect("todos key");
            match todos {
                sharing_instant::Value::Array(arr) => assert!(arr.is_empty()),
                sharing_instant::Value::Object(map) => assert!(map.is_empty()),
                _ => panic!("expected array or object"),
            }
        }
        _ => panic!("expected object"),
    }
}

#[test]
fn create_with_callbacks_fires_success() {
    let (_db, mutator) = setup();
    let called = Arc::new(AtomicBool::new(false));
    let called_clone = called.clone();

    let cb = MutationCallbacks::<()>::new()
        .on_success(move |_| called_clone.store(true, Ordering::SeqCst));

    mutator
        .create_with_callbacks(
            &Todo {
                id: "t1".into(),
                title: "Test".into(),
                is_completed: false,
            },
            cb,
        )
        .expect("create_with_callbacks");

    assert!(called.load(Ordering::SeqCst));
}

#[test]
fn mutator_is_cloneable() {
    let (_db, mutator) = setup();
    let mutator2 = mutator.clone();

    mutator
        .create(&Todo {
            id: "t1".into(),
            title: "First".into(),
            is_completed: false,
        })
        .expect("create via original");

    mutator2
        .create(&Todo {
            id: "t2".into(),
            title: "Second".into(),
            is_completed: false,
        })
        .expect("create via clone");
}

#[test]
fn link_with_callbacks_fires_success() {
    let (_db, mutator) = setup();

    // Create two items to link.
    mutator
        .create(&Todo {
            id: "t1".into(),
            title: "First".into(),
            is_completed: false,
        })
        .expect("create t1");
    mutator
        .create(&Todo {
            id: "t2".into(),
            title: "Second".into(),
            is_completed: false,
        })
        .expect("create t2");

    let called = Arc::new(AtomicBool::new(false));
    let c = called.clone();
    let cb = MutationCallbacks::<()>::new().on_success(move |_| c.store(true, Ordering::SeqCst));

    mutator
        .link_with_callbacks("t1", "related", "t2", cb)
        .expect("link_with_callbacks");

    assert!(called.load(Ordering::SeqCst));
}

#[test]
fn unlink_with_callbacks_fires_success() {
    let (_db, mutator) = setup();

    mutator
        .create(&Todo {
            id: "t1".into(),
            title: "First".into(),
            is_completed: false,
        })
        .expect("create t1");
    mutator
        .create(&Todo {
            id: "t2".into(),
            title: "Second".into(),
            is_completed: false,
        })
        .expect("create t2");

    // Link first, then unlink with callbacks.
    mutator.link("t1", "related", "t2").expect("link");

    let called = Arc::new(AtomicBool::new(false));
    let c = called.clone();
    let cb = MutationCallbacks::<()>::new().on_success(move |_| c.store(true, Ordering::SeqCst));

    mutator
        .unlink_with_callbacks("t1", "related", "t2", cb)
        .expect("unlink_with_callbacks");

    assert!(called.load(Ordering::SeqCst));
}

#[tokio::test]
async fn fetch_all_mutation_methods() {
    use sharing_instant::FetchAll;

    let db: Arc<dyn Database> = Arc::new(InMemoryDatabase::new());
    let fetch = FetchAll::<Todo>::new(db);

    // Create via FetchAll
    fetch
        .create(&Todo {
            id: "t1".into(),
            title: "Via FetchAll".into(),
            is_completed: false,
        })
        .expect("create via FetchAll");

    // Delete via FetchAll
    fetch.delete("t1").expect("delete via FetchAll");
}

#[tokio::test]
async fn fetch_all_create_with_callbacks() {
    use sharing_instant::FetchAll;

    let db: Arc<dyn Database> = Arc::new(InMemoryDatabase::new());
    let fetch = FetchAll::<Todo>::new(db);

    let called = Arc::new(AtomicBool::new(false));
    let c = called.clone();
    let cb = MutationCallbacks::<()>::new().on_success(move |_| c.store(true, Ordering::SeqCst));

    fetch
        .create_with_callbacks(
            &Todo {
                id: "t1".into(),
                title: "With callback".into(),
                is_completed: false,
            },
            cb,
        )
        .expect("create_with_callbacks via FetchAll");

    assert!(called.load(Ordering::SeqCst));
}

#[tokio::test]
async fn fetch_all_delete_with_callbacks() {
    use sharing_instant::FetchAll;

    let db: Arc<dyn Database> = Arc::new(InMemoryDatabase::new());
    let fetch = FetchAll::<Todo>::new(db);

    fetch
        .create(&Todo {
            id: "t1".into(),
            title: "To delete".into(),
            is_completed: false,
        })
        .expect("create");

    let called = Arc::new(AtomicBool::new(false));
    let c = called.clone();
    let cb = MutationCallbacks::<()>::new().on_success(move |_| c.store(true, Ordering::SeqCst));

    fetch
        .delete_with_callbacks("t1", cb)
        .expect("delete_with_callbacks via FetchAll");

    assert!(called.load(Ordering::SeqCst));
}
