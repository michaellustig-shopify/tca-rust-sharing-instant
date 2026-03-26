//! Todos — collaborative todo list with real-time sync.
//!
//! Demonstrates `InstantAsync::subscribe` + `transact` for CRUD operations.
//! A background watcher reprints the numbered list on every database change.

use futures::StreamExt;
use instant_client::async_api::InstantAsync;
use instant_client::connection::ConnectionConfig;
use parking_lot::RwLock;
use recipe_utils::{get_or_create_app, print_join_instructions, BOLD, DIM, RESET};
use serde_json::json;
use std::sync::Arc;

fn uuid() -> String {
    uuid::Uuid::new_v4().to_string()
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("time went backwards")
        .as_millis() as i64
}

#[tokio::main]
async fn main() {
    println!("\n  {BOLD}Todos{RESET} — collaborative todo list\n");

    let app = get_or_create_app("todos").await.expect("app setup failed");
    print_join_instructions(&app, "todos");

    // Admin client for writes (REST API).
    // WebSocket client for reactive subscriptions.
    let admin = instant_admin::AdminClient::new(&app.id, &app.admin_token);
    let config = ConnectionConfig::admin(&app.id, &app.admin_token);
    let client = InstantAsync::new(config)
        .await
        .expect("WebSocket connect failed");

    println!("  Connected!\n");
    println!("  {DIM}Commands: add <text>, done <n>, del <n>, list, quit{RESET}\n");

    let current_todos: Arc<RwLock<Vec<serde_json::Value>>> = Arc::new(RwLock::new(Vec::new()));

    // Background watcher
    let todos_bg = current_todos.clone();
    let mut stream = client.subscribe(&json!({"todos": {}})).await;
    tokio::spawn(async move {
        while let Some(data) = stream.next().await {
            if let Some(arr) = data.get("todos").and_then(|v| v.as_array()) {
                let mut sorted = arr.clone();
                sorted.sort_by_key(|t| t.get("ts").and_then(|v| v.as_i64()).unwrap_or(0));
                let count = sorted.len();
                *todos_bg.write() = sorted;
                println!("\n  {DIM}[sync] {count} todos{RESET}");
                print_todos(&todos_bg.read());
                recipe_utils::prompt("  > ");
            }
        }
    });

    // CLI loop
    let reader = tokio::io::BufReader::new(tokio::io::stdin());
    let mut lines = tokio::io::AsyncBufReadExt::lines(reader);

    recipe_utils::prompt("  > ");
    loop {
        let line = match lines.next_line().await {
            Ok(Some(l)) => l,
            _ => break,
        };
        let line = line.trim().to_string();
        if line.is_empty() {
            recipe_utils::prompt("  > ");
            continue;
        }

        if line == "quit" || line == "q" {
            break;
        }

        if line == "list" || line == "ls" {
            print_todos(&current_todos.read());
            recipe_utils::prompt("  > ");
            continue;
        }

        if line.starts_with("add ") {
            let text = &line[4..];
            let id = uuid();
            let tx =
                json!([["update", "todos", &id, {"text": text, "done": false, "ts": now_ms()}]]);
            match admin.transact(&tx).await {
                Ok(_) => println!("  Added: {text}"),
                Err(e) => println!("  Error: {e}"),
            }
            recipe_utils::prompt("  > ");
            continue;
        }

        if line.starts_with("done ") {
            if let Ok(idx) = line[5..].trim().parse::<usize>() {
                let todos = current_todos.read();
                if idx > 0 && idx <= todos.len() {
                    let todo = &todos[idx - 1];
                    let id = todo
                        .get("id")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let done = todo.get("done").and_then(|v| v.as_bool()).unwrap_or(false);
                    drop(todos);
                    let tx = json!([["update", "todos", &id, {"done": !done}]]);
                    match admin.transact(&tx).await {
                        Ok(_) => println!("  Toggled #{idx}"),
                        Err(e) => println!("  Error: {e}"),
                    }
                } else {
                    println!("  Invalid index (1-{})", todos.len());
                }
            }
            recipe_utils::prompt("  > ");
            continue;
        }

        if line.starts_with("del ") {
            if let Ok(idx) = line[4..].trim().parse::<usize>() {
                let todos = current_todos.read();
                if idx > 0 && idx <= todos.len() {
                    let id = todos[idx - 1]
                        .get("id")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    drop(todos);
                    let tx = json!([["delete", "todos", &id, {}]]);
                    match admin.transact(&tx).await {
                        Ok(_) => println!("  Deleted #{idx}"),
                        Err(e) => println!("  Error: {e}"),
                    }
                } else {
                    println!("  Invalid index (1-{})", todos.len());
                }
            }
            recipe_utils::prompt("  > ");
            continue;
        }

        println!("  Unknown command. Try: add, done, del, list, quit");
        recipe_utils::prompt("  > ");
    }

    client.close().await;
    println!("\n  Goodbye!");
}

fn print_todos(todos: &[serde_json::Value]) {
    if todos.is_empty() {
        println!("  {DIM}(no todos yet){RESET}");
        return;
    }
    for (i, todo) in todos.iter().enumerate() {
        let text = todo.get("text").and_then(|v| v.as_str()).unwrap_or("?");
        let done = todo.get("done").and_then(|v| v.as_bool()).unwrap_or(false);
        let mark = if done {
            "\x1b[32m\u{2713}\x1b[0m"
        } else {
            "\u{25cb}"
        };
        let style = if done { "\x1b[9;2m" } else { "" };
        let reset = if done { "\x1b[0m" } else { "" };
        println!("  {}. {} {}{}{}", i + 1, mark, style, text, reset);
    }
}
