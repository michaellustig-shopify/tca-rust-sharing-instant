//! Typing Indicator — presence with idle timeout.
//!
//! Demonstrates `Room<P>` presence with a 2-second idle timer.
//! Any keystroke sets `is_typing = true`. After 2 seconds of inactivity
//! the timer fires and sets `is_typing = false`.

use recipe_utils::{
    connect_reactor, get_or_create_app, print_join_instructions, random_name, BOLD, DIM, RESET,
};
use serde::{Deserialize, Serialize};
use sharing_instant::rooms::Room;
use std::sync::Arc;
use tokio::sync::Notify;

#[derive(Clone, Serialize, Deserialize)]
struct TypingPresence {
    name: String,
    is_typing: bool,
}

#[tokio::main]
async fn main() {
    println!("\n  {BOLD}Typing Indicator{RESET} — who's typing?\n");

    let app = get_or_create_app("typing-indicator")
        .await
        .expect("app setup failed");
    print_join_instructions(&app, "typing-indicator");

    let reactor = connect_reactor(&app).await.expect("reactor connect failed");
    let handle = tokio::runtime::Handle::current();

    let name = random_name();
    println!("  You are: {BOLD}{name}{RESET}");
    println!("  {DIM}Start typing to show typing indicator. Press Enter to send. Type 'quit' to exit.{RESET}\n");

    let room = Room::<TypingPresence>::join(reactor.clone(), handle.clone(), "typing", "chat")
        .expect("failed to join room");

    room.set_presence(&TypingPresence {
        name: name.clone(),
        is_typing: false,
    })
    .expect("failed to set presence");

    // Background watcher — prints who's typing
    let mut rx = room.watch_presence();
    tokio::spawn(async move {
        while rx.changed().await.is_ok() {
            let state = rx.borrow().clone();
            let typing: Vec<&str> = state
                .peers
                .values()
                .filter(|p| p.is_typing)
                .map(|p| p.name.as_str())
                .collect();

            if typing.is_empty() {
                print!("\r\x1b[K  {DIM}(no one typing){RESET}");
            } else if typing.len() == 1 {
                print!("\r\x1b[K  {BOLD}{}{RESET} is typing...", typing[0]);
            } else {
                let names = typing.join(", ");
                print!("\r\x1b[K  {BOLD}{names}{RESET} are typing...");
            }
            recipe_utils::flush();
        }
    });

    // Idle timer: notified on each keystroke, fires 2s after last keystroke
    let keystroke_notify = Arc::new(Notify::new());
    let idle_name = name.clone();
    let idle_room = room.watch_presence(); // just need Arc<Reactor> access via room
    drop(idle_room);

    // We need a separate handle to set presence from the idle task
    let idle_reactor = reactor.clone();
    let idle_handle = handle.clone();
    let idle_notify = keystroke_notify.clone();

    // Spawn idle timer — uses a fresh Room reference via reactor
    let idle_name2 = idle_name.clone();
    tokio::spawn(async move {
        let idle_room = Room::<TypingPresence>::join(idle_reactor, idle_handle, "typing", "chat")
            .expect("idle room join failed");

        loop {
            // Wait for a keystroke notification
            idle_notify.notified().await;
            // Then wait 2 seconds — if another keystroke comes in, restart
            loop {
                let timeout =
                    tokio::time::timeout(std::time::Duration::from_secs(2), idle_notify.notified())
                        .await;

                if timeout.is_err() {
                    // Timed out — no new keystrokes for 2s, set idle
                    let _ = idle_room.set_presence(&TypingPresence {
                        name: idle_name2.clone(),
                        is_typing: false,
                    });
                    break;
                }
                // Got another keystroke, loop again to reset the 2s timer
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

        if line == "quit" || line == "q" {
            break;
        }

        if !line.is_empty() {
            // User typed something and pressed Enter — mark typing then idle
            room.set_presence(&TypingPresence {
                name: name.clone(),
                is_typing: true,
            })
            .expect("failed to set presence");
            keystroke_notify.notify_one();

            println!("\r\x1b[K  {DIM}You said:{RESET} {line}");
        }

        recipe_utils::prompt("  > ");
    }

    room.leave();
    reactor.stop().await;
    println!("\n  Goodbye!");
}
