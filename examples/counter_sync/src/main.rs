//! Counter sync demo — proves end-to-end real-time sync via LiveDatabase.
//!
//! Run in two terminals to see live sync:
//!   INSTANT_APP_ID=<id> INSTANT_ADMIN_TOKEN=<token> cargo run --bin counter-sync
//!
//! Press +/- to increment/decrement. Both terminals see changes in real-time.

use instant_client::{ConnectionConfig, Reactor};
use sharing_instant::database::{Database, LiveDatabase};
use sharing_instant::table::{json_to_value, value_to_json};
use std::io::{self, Read};
use std::sync::Arc;

const COUNTER_ID: &str = "00000000-0000-0000-0000-000000000001";

fn get_config() -> (String, String) {
    let app_id =
        std::env::var("INSTANT_APP_ID").expect("Set INSTANT_APP_ID env var");
    let admin_token =
        std::env::var("INSTANT_ADMIN_TOKEN").expect("Set INSTANT_ADMIN_TOKEN env var");
    (app_id, admin_token)
}

#[tokio::main]
async fn main() {
    let (app_id, admin_token) = get_config();

    println!("Connecting to InstantDB...");

    // Create reactor + LiveDatabase
    let config = ConnectionConfig::admin(&app_id, &admin_token);
    let reactor = Arc::new(Reactor::new(config));
    reactor.start().await.expect("reactor should start");

    // Wait for InitOk (attrs catalog needed for transact)
    tokio::time::sleep(std::time::Duration::from_millis(1000)).await;

    let handle = tokio::runtime::Handle::current();
    let db = Arc::new(LiveDatabase::new(reactor.clone(), handle));

    // Initialize counter if not exists
    let init_tx = json_to_value(&serde_json::json!([
        ["update", "counters", COUNTER_ID, {"value": 0}]
    ]));
    if let Err(e) = db.transact(&init_tx) {
        eprintln!("Init transact error (may be OK if counter exists): {}", e);
    }

    println!("Connected! Press + to increment, - to decrement, q to quit.");
    println!("Run in another terminal to see live sync.\n");

    // Subscribe to counter updates
    let query = json_to_value(&serde_json::json!({"counters": {}}));
    let mut rx = db.subscribe(&query).expect("subscribe should work");

    // Spawn subscription listener
    tokio::spawn(async move {
        loop {
            if rx.changed().await.is_err() {
                break;
            }
            let val = rx.borrow().clone();
            if let Some(data) = val {
                let json = value_to_json(&data);
                if let Some(counters) = json.get("counters").and_then(|v| v.as_array()) {
                    if let Some(counter) = counters.first() {
                        let value = counter
                            .get("value")
                            .and_then(|v| v.as_i64())
                            .unwrap_or(0);
                        println!("  Counter: {}", value);
                    }
                }
            }
        }
    });

    // Read keypresses
    let db_write = db.clone();
    let stdin = io::stdin();
    let mut bytes = stdin.lock().bytes();

    loop {
        if let Some(Ok(byte)) = bytes.next() {
            match byte {
                b'+' | b'=' => {
                    let query = json_to_value(&serde_json::json!({"counters": {}}));
                    let result = db_write.query(&query).expect("query should work");
                    let json = value_to_json(&result);
                    let current = json
                        .get("counters")
                        .and_then(|v| v.as_array())
                        .and_then(|a| a.first())
                        .and_then(|c| c.get("value"))
                        .and_then(|v| v.as_i64())
                        .unwrap_or(0);

                    let tx = json_to_value(&serde_json::json!([
                        ["update", "counters", COUNTER_ID, {"value": current + 1}]
                    ]));
                    if let Err(e) = db_write.transact(&tx) {
                        eprintln!("Transact error: {}", e);
                    }
                }
                b'-' | b'_' => {
                    let query = json_to_value(&serde_json::json!({"counters": {}}));
                    let result = db_write.query(&query).expect("query should work");
                    let json = value_to_json(&result);
                    let current = json
                        .get("counters")
                        .and_then(|v| v.as_array())
                        .and_then(|a| a.first())
                        .and_then(|c| c.get("value"))
                        .and_then(|v| v.as_i64())
                        .unwrap_or(0);

                    let tx = json_to_value(&serde_json::json!([
                        ["update", "counters", COUNTER_ID, {"value": current - 1}]
                    ]));
                    if let Err(e) = db_write.transact(&tx) {
                        eprintln!("Transact error: {}", e);
                    }
                }
                b'q' | b'Q' => {
                    println!("Shutting down...");
                    reactor.stop().await;
                    break;
                }
                _ => {}
            }
        }
    }
}
