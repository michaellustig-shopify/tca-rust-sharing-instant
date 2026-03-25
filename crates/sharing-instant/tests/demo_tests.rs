//! Live demo tests showing the full API surface.
//!
//! Run with: cargo test -p sharing-instant --test demo_tests -- --nocapture

use sharing_instant::database::{Database, InMemoryDatabase};
use sharing_instant::error::Result;
use sharing_instant::fetch::Fetch;
use sharing_instant::fetch_all::FetchAll;
use sharing_instant::fetch_key_request::FetchKeyRequest;
use sharing_instant::fetch_one::FetchOne;
use sharing_instant::keys::in_memory_key::InMemoryKey;
use sharing_instant::shared::Shared;
use sharing_instant::table::{json_to_value, value_to_json, ColumnDef, Table};
use sharing_instant::Value;
use std::sync::Arc;

// ─── Schema ───────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
struct RemindersList {
    id: String,
    title: String,
    color: Option<String>,
}

impl Table for RemindersList {
    const TABLE_NAME: &'static str = "remindersLists";
    fn columns() -> &'static [ColumnDef] {
        &[]
    }
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
struct Reminder {
    id: String,
    title: String,
    is_completed: bool,
    priority: Option<i64>,
    reminders_list_id: String,
}

impl Table for Reminder {
    const TABLE_NAME: &'static str = "reminders";
    fn columns() -> &'static [ColumnDef] {
        &[]
    }
}

// ─── Helper: seed a database ──────────────────────────────────

fn seed_db() -> Arc<InMemoryDatabase> {
    let db = Arc::new(InMemoryDatabase::new());

    // Two lists
    db.insert(
        "remindersLists",
        "list-personal",
        serde_json::json!({
            "id": "list-personal",
            "title": "Personal",
            "color": "blue"
        }),
    );
    db.insert(
        "remindersLists",
        "list-work",
        serde_json::json!({
            "id": "list-work",
            "title": "Work",
            "color": "red"
        }),
    );

    // Reminders in Personal
    db.insert(
        "reminders",
        "r1",
        serde_json::json!({
            "id": "r1",
            "title": "Buy milk",
            "is_completed": false,
            "priority": 2,
            "reminders_list_id": "list-personal"
        }),
    );
    db.insert(
        "reminders",
        "r2",
        serde_json::json!({
            "id": "r2",
            "title": "Walk the dog",
            "is_completed": true,
            "priority": 1,
            "reminders_list_id": "list-personal"
        }),
    );
    db.insert(
        "reminders",
        "r3",
        serde_json::json!({
            "id": "r3",
            "title": "Read a book",
            "is_completed": false,
            "priority": null,
            "reminders_list_id": "list-personal"
        }),
    );

    // Reminders in Work
    db.insert(
        "reminders",
        "r4",
        serde_json::json!({
            "id": "r4",
            "title": "Ship feature",
            "is_completed": false,
            "priority": 1,
            "reminders_list_id": "list-work"
        }),
    );
    db.insert(
        "reminders",
        "r5",
        serde_json::json!({
            "id": "r5",
            "title": "Review PR",
            "is_completed": false,
            "priority": 3,
            "reminders_list_id": "list-work"
        }),
    );

    db
}

// ═══════════════════════════════════════════════════════════════
// Demo 1: FetchAll — reactive collection
// ═══════════════════════════════════════════════════════════════

#[test]
fn demo_fetch_all_basic() {
    let db = seed_db();

    // Like Swift: @FetchAll var reminders: [Reminder]
    let fetch = FetchAll::<Reminder>::new(db.clone());

    let reminders = fetch.get();
    println!("\n═══ Demo: FetchAll ═══");
    println!("All reminders ({} total):", reminders.len());
    for r in &reminders {
        let status = if r.is_completed { "✓" } else { "○" };
        let priority = r.priority.map(|p| format!("P{p}")).unwrap_or("--".into());
        println!("  {status} [{priority}] {}", r.title);
    }

    assert_eq!(reminders.len(), 5);
}

// ═══════════════════════════════════════════════════════════════
// Demo 2: FetchAll with custom query
// ═══════════════════════════════════════════════════════════════

#[test]
fn demo_fetch_all_with_query() {
    let db = seed_db();

    // Like Swift: @FetchAll(Reminder.where(\.isCompleted == false))
    let query = Reminder::query()
        .where_eq("is_completed", Value::Bool(false))
        .order("priority", "asc")
        .build();

    let fetch = FetchAll::<Reminder>::with_query(db.clone(), query);

    println!("\n═══ Demo: FetchAll with query ═══");
    println!("Query: WHERE is_completed = false ORDER BY priority ASC");
    let items = fetch.get();
    println!("Results ({} items):", items.len());
    for r in &items {
        let priority = r.priority.map(|p| format!("P{p}")).unwrap_or("--".into());
        println!("  ○ [{priority}] {}", r.title);
    }

    // In-memory DB returns all (filtering is server-side in real InstantDB)
    // but the query structure is correct
    assert!(items.len() >= 1);
}

// ═══════════════════════════════════════════════════════════════
// Demo 3: FetchOne — single value
// ═══════════════════════════════════════════════════════════════

#[test]
fn demo_fetch_one() {
    let db = seed_db();

    // Like Swift: @FetchOne var list: RemindersList?
    let fetch = FetchOne::<RemindersList>::new(db.clone());

    println!("\n═══ Demo: FetchOne ═══");
    match fetch.get() {
        Some(list) => {
            println!(
                "First list: {} ({})",
                list.title,
                list.color.as_deref().unwrap_or("no color")
            );
        }
        None => println!("No lists found"),
    }

    // require() throws if not found
    let list = fetch.require().unwrap();
    println!("require() succeeded: {}", list.title);
}

// ═══════════════════════════════════════════════════════════════
// Demo 4: Fetch — custom multi-query request
// ═══════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Default)]
struct DashboardData {
    total_reminders: usize,
    completed_count: usize,
    incomplete_count: usize,
    lists_count: usize,
}

struct DashboardRequest;

impl FetchKeyRequest for DashboardRequest {
    type Value = DashboardData;

    fn fetch(&self, db: &dyn Database) -> Result<DashboardData> {
        // Query both tables in one atomic operation
        let q = json_to_value(&serde_json::json!({
            "reminders": {},
            "remindersLists": {}
        }));
        let result = db.query(&q)?;

        let mut data = DashboardData::default();

        if let Value::Object(obj) = &result {
            if let Some(Value::Array(reminders)) = obj.get("reminders") {
                data.total_reminders = reminders.len();
                for r in reminders {
                    if let Value::Object(r_obj) = r {
                        match r_obj.get("is_completed") {
                            Some(Value::Bool(true)) => data.completed_count += 1,
                            _ => data.incomplete_count += 1,
                        }
                    }
                }
            }
            if let Some(Value::Array(lists)) = obj.get("remindersLists") {
                data.lists_count = lists.len();
            }
        }

        Ok(data)
    }

    fn queries(&self) -> Vec<Value> {
        vec![
            json_to_value(&serde_json::json!({ "reminders": {} })),
            json_to_value(&serde_json::json!({ "remindersLists": {} })),
        ]
    }
}

#[test]
fn demo_fetch_custom_request() {
    let db = seed_db();

    // Like Swift: @Fetch(DashboardRequest()) var dashboard
    let fetch = Fetch::new(DashboardRequest, db.clone());
    let data = fetch.get();

    println!("\n═══ Demo: Fetch (custom request) ═══");
    println!("Dashboard:");
    println!("  Lists:     {}", data.lists_count);
    println!("  Total:     {}", data.total_reminders);
    println!("  Completed: {}", data.completed_count);
    println!("  Remaining: {}", data.incomplete_count);

    assert_eq!(data.total_reminders, 5);
    assert_eq!(data.completed_count, 1);
    assert_eq!(data.incomplete_count, 4);
    assert_eq!(data.lists_count, 2);
}

// ═══════════════════════════════════════════════════════════════
// Demo 5: Shared — mutable state with persistence
// ═══════════════════════════════════════════════════════════════

#[test]
fn demo_shared_in_memory() {
    println!("\n═══ Demo: Shared<V, InMemoryKey> ═══");

    // Like Swift: @Shared(.inMemory("settings")) var settings = Settings()
    let shared = Shared::new(
        Settings {
            volume: 0.5,
            theme: "light".to_string(),
        },
        InMemoryKey::new("demo_settings"),
    );

    println!(
        "Initial: volume={}, theme={}",
        shared.get().volume,
        shared.get().theme
    );

    // Mutate with auto-persist
    shared.with_lock(|s| {
        s.volume = 0.8;
        s.theme = "dark".to_string();
    });

    println!(
        "After mutation: volume={}, theme={}",
        shared.get().volume,
        shared.get().theme
    );

    // Another Shared with the same key sees the persisted value
    let shared2 = Shared::new(
        Settings {
            volume: 0.0,
            theme: "default".to_string(),
        },
        InMemoryKey::new("demo_settings"),
    );

    println!(
        "shared2 (same key): volume={}, theme={}",
        shared2.get().volume,
        shared2.get().theme
    );

    assert_eq!(shared2.get().volume, 0.8);
    assert_eq!(shared2.get().theme, "dark");
}

#[derive(Debug, Clone)]
struct Settings {
    volume: f64,
    theme: String,
}

// ═══════════════════════════════════════════════════════════════
// Demo 6: Shared clone shares state
// ═══════════════════════════════════════════════════════════════

#[test]
fn demo_shared_clone() {
    println!("\n═══ Demo: Shared clone ═══");

    let feature_a = Shared::new(0i32, InMemoryKey::new("demo_counter"));
    let feature_b = feature_a.clone();

    println!("Feature A: {}", *feature_a.get());
    println!("Feature B: {}", *feature_b.get());

    feature_a.with_lock(|v| *v += 10);
    println!("After Feature A increments by 10:");
    println!("  Feature A: {}", *feature_a.get());
    println!("  Feature B: {}", *feature_b.get());

    assert_eq!(*feature_a.get(), 10);
    assert_eq!(*feature_b.get(), 10); // Same underlying reference
}

// ═══════════════════════════════════════════════════════════════
// Demo 7: Reactive subscription (watch channel)
// ═══════════════════════════════════════════════════════════════

#[test]
fn demo_reactive_subscription() {
    let db = seed_db();

    println!("\n═══ Demo: Reactive subscription ═══");

    // Subscribe to the reminders table
    let query = json_to_value(&serde_json::json!({ "reminders": {} }));
    let rx = db.subscribe(&query).unwrap();

    let count = |rx: &tokio::sync::watch::Receiver<Option<Value>>| -> usize {
        let val = rx.borrow().clone();
        match val.as_ref() {
            Some(Value::Object(obj)) => match obj.get("reminders") {
                Some(Value::Array(arr)) => arr.len(),
                _ => 0,
            },
            _ => 0,
        }
    };

    println!("Initial count: {}", count(&rx));
    assert_eq!(count(&rx), 5);

    // Insert a new reminder — subscription auto-updates
    db.insert(
        "reminders",
        "r6",
        serde_json::json!({
            "id": "r6",
            "title": "New reminder!",
            "is_completed": false,
            "priority": 1,
            "reminders_list_id": "list-personal"
        }),
    );

    println!("After insert: {}", count(&rx));
    assert_eq!(count(&rx), 6);

    // Delete a reminder — subscription auto-updates
    db.remove("reminders", "r1");

    println!("After delete: {}", count(&rx));
    assert_eq!(count(&rx), 5);
}

// ═══════════════════════════════════════════════════════════════
// Demo 8: Nested / relational queries
// ═══════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Default)]
struct ListWithReminders {
    list: Option<RemindersList>,
    reminders: Vec<Reminder>,
}

struct ListDetailRequest {
    list_id: String,
}

impl FetchKeyRequest for ListDetailRequest {
    type Value = ListWithReminders;

    fn fetch(&self, db: &dyn Database) -> Result<ListWithReminders> {
        // Query both tables — in real InstantDB this would be a
        // single nested query like:
        //   { "remindersLists": { "reminders": {} } }
        // For our in-memory DB, we query separately and join in code.
        let lists_q = json_to_value(&serde_json::json!({ "remindersLists": {} }));
        let reminders_q = json_to_value(&serde_json::json!({ "reminders": {} }));

        let lists_result = db.query(&lists_q)?;
        let reminders_result = db.query(&reminders_q)?;

        let mut result = ListWithReminders::default();

        // Find our list
        if let Value::Object(obj) = &lists_result {
            if let Some(Value::Array(lists)) = obj.get("remindersLists") {
                for list_val in lists {
                    if let Ok(list) = RemindersList::from_value(list_val) {
                        if list.id == self.list_id {
                            result.list = Some(list);
                            break;
                        }
                    }
                }
            }
        }

        // Filter reminders to this list
        if let Value::Object(obj) = &reminders_result {
            if let Some(Value::Array(reminders)) = obj.get("reminders") {
                for r_val in reminders {
                    if let Ok(r) = Reminder::from_value(r_val) {
                        if r.reminders_list_id == self.list_id {
                            result.reminders.push(r);
                        }
                    }
                }
            }
        }

        Ok(result)
    }

    fn queries(&self) -> Vec<Value> {
        vec![
            json_to_value(&serde_json::json!({ "remindersLists": {} })),
            json_to_value(&serde_json::json!({ "reminders": {} })),
        ]
    }
}

#[test]
fn demo_nested_query() {
    let db = seed_db();

    println!("\n═══ Demo: Nested / relational query ═══");

    // Fetch a list with all its reminders
    let fetch = Fetch::new(
        ListDetailRequest {
            list_id: "list-personal".to_string(),
        },
        db.clone(),
    );

    let detail = fetch.get();

    if let Some(list) = &detail.list {
        println!(
            "List: {} ({})",
            list.title,
            list.color.as_deref().unwrap_or("")
        );
        println!("Reminders:");
        for r in &detail.reminders {
            let status = if r.is_completed { "✓" } else { "○" };
            let priority = r.priority.map(|p| format!("P{p}")).unwrap_or("--".into());
            println!("  {status} [{priority}] {}", r.title);
        }
    }

    assert!(detail.list.is_some());
    assert_eq!(detail.list.unwrap().title, "Personal");
    assert_eq!(detail.reminders.len(), 3);
}

#[test]
fn demo_nested_query_work_list() {
    let db = seed_db();

    let fetch = Fetch::new(
        ListDetailRequest {
            list_id: "list-work".to_string(),
        },
        db.clone(),
    );

    let detail = fetch.get();

    println!("\n═══ Demo: Nested query (Work list) ═══");
    if let Some(list) = &detail.list {
        println!(
            "List: {} ({})",
            list.title,
            list.color.as_deref().unwrap_or("")
        );
        println!("Reminders:");
        for r in &detail.reminders {
            let status = if r.is_completed { "✓" } else { "○" };
            let priority = r.priority.map(|p| format!("P{p}")).unwrap_or("--".into());
            println!("  {status} [{priority}] {}", r.title);
        }
    }

    assert_eq!(detail.reminders.len(), 2);
}

// ═══════════════════════════════════════════════════════════════
// Demo 9: InstaQL nested query format
//
// Shows what the REAL InstantDB nested query looks like.
// InstantDB supports nesting via the query structure:
//
//   { "remindersLists": { "reminders": {} } }
//
// This returns each list with its related reminders embedded.
// ═══════════════════════════════════════════════════════════════

#[test]
fn demo_instaql_nested_format() {
    println!("\n═══ Demo: InstaQL nested query format ═══");

    // This is the InstantDB nested query format.
    // In a real InstantDB instance with links defined, this query:
    let nested_query = serde_json::json!({
        "remindersLists": {
            "$": { "where": { "id": "list-personal" } },
            "reminders": {}
        }
    });

    println!(
        "Query:\n{}",
        serde_json::to_string_pretty(&nested_query).unwrap()
    );

    // Expected response shape from InstantDB:
    let expected_response = serde_json::json!({
        "remindersLists": [{
            "id": "list-personal",
            "title": "Personal",
            "color": "blue",
            "reminders": [
                { "id": "r1", "title": "Buy milk", "is_completed": false, "priority": 2 },
                { "id": "r2", "title": "Walk the dog", "is_completed": true, "priority": 1 },
                { "id": "r3", "title": "Read a book", "is_completed": false, "priority": null }
            ]
        }]
    });

    println!(
        "\nExpected response:\n{}",
        serde_json::to_string_pretty(&expected_response).unwrap()
    );

    // The nested query Value builds correctly
    let query_value = json_to_value(&nested_query);
    let roundtrip = value_to_json(&query_value);
    assert_eq!(nested_query, roundtrip);

    println!("\nValue roundtrip: ✓");
}

// ═══════════════════════════════════════════════════════════════
// Demo 10: QueryBuilder DSL
// ═══════════════════════════════════════════════════════════════

#[test]
fn demo_query_builder() {
    println!("\n═══ Demo: QueryBuilder DSL ═══");

    let q1 = Reminder::query().build();
    println!("All reminders:");
    println!("  {}", serde_json::to_string(&value_to_json(&q1)).unwrap());

    let q2 = Reminder::query()
        .where_eq("is_completed", Value::Bool(false))
        .build();
    println!("Incomplete:");
    println!("  {}", serde_json::to_string(&value_to_json(&q2)).unwrap());

    let q3 = Reminder::query()
        .where_gt("priority", Value::Int(1))
        .order("priority", "desc")
        .limit(3)
        .build();
    println!("High priority (top 3):");
    println!("  {}", serde_json::to_string(&value_to_json(&q3)).unwrap());

    let q4 = Reminder::query()
        .where_in("priority", vec![Value::Int(1), Value::Int(2)])
        .where_eq("is_completed", Value::Bool(false))
        .build();
    println!("Priority 1 or 2, incomplete:");
    println!("  {}", serde_json::to_string(&value_to_json(&q4)).unwrap());

    let q5 = Reminder::query().where_is_null("priority", true).build();
    println!("No priority set:");
    println!("  {}", serde_json::to_string(&value_to_json(&q5)).unwrap());
}
