//! Reactions — emoji broadcast with TopicChannel.
//!
//! Demonstrates `TopicChannel<T>` pub/sub. Type an emoji name to broadcast it
//! to all peers. A background watcher prints received reactions in real-time.

use recipe_utils::{
    connect_reactor, get_or_create_app, print_join_instructions, random_name, BOLD, DIM, RESET,
};
use serde::{Deserialize, Serialize};
use sharing_instant::topics::TopicChannel;

#[derive(Clone, Serialize, Deserialize)]
struct EmojiReaction {
    emoji: String,
    sender: String,
}

const EMOJI_MAP: &[(&str, &str)] = &[
    ("fire", "\u{1f525}"),
    ("wave", "\u{1f44b}"),
    ("confetti", "\u{1f389}"),
    ("heart", "\u{2764}\u{fe0f}"),
    ("rocket", "\u{1f680}"),
    ("thumbsup", "\u{1f44d}"),
    ("clap", "\u{1f44f}"),
    ("100", "\u{1f4af}"),
];

fn emoji_char(name: &str) -> Option<&'static str> {
    EMOJI_MAP.iter().find(|(n, _)| *n == name).map(|(_, c)| *c)
}

#[tokio::main]
async fn main() {
    println!("\n  {BOLD}Reactions{RESET} — emoji broadcast\n");

    let app = get_or_create_app("reactions")
        .await
        .expect("app setup failed");
    print_join_instructions(&app, "reactions");

    let reactor = connect_reactor(&app).await.expect("reactor connect failed");
    let handle = tokio::runtime::Handle::current();

    let name = random_name();
    println!("  You are: {BOLD}{name}{RESET}\n");

    let channel = TopicChannel::<EmojiReaction>::subscribe(
        reactor.clone(),
        handle,
        "reactions",
        "lobby",
        "emoji",
    )
    .expect("failed to subscribe to topic");

    println!("  {DIM}Available emojis:{RESET}");
    for (name, ch) in EMOJI_MAP {
        println!("    {name:>10}  {ch}");
    }
    println!("\n  {DIM}Type an emoji name to broadcast it. Type 'quit' to exit.{RESET}\n");

    // Background watcher
    let mut rx = channel.watch();
    tokio::spawn(async move {
        while rx.changed().await.is_ok() {
            let events = rx.borrow().clone();
            if let Some(event) = events.last() {
                let ch = emoji_char(&event.data.emoji).unwrap_or("?");
                println!(
                    "\n  {BOLD}{ch}{RESET}  {DIM}from{RESET} {}",
                    event.data.sender
                );
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

        if emoji_char(&line).is_some() {
            let reaction = EmojiReaction {
                emoji: line.clone(),
                sender: name.clone(),
            };
            match channel.publish(&reaction) {
                Ok(_) => {}
                Err(e) => println!("  Error: {e}"),
            }
        } else {
            println!(
                "  Unknown emoji. Try: fire, wave, confetti, heart, rocket, thumbsup, clap, 100"
            );
        }
        recipe_utils::prompt("  > ");
    }

    reactor.stop().await;
    println!("\n  Goodbye!");
}
