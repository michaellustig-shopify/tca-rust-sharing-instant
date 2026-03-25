//! Avatar Stack — who's online, with status.
//!
//! Demonstrates `Room<P>` presence. Each peer publishes their name, color,
//! and status. Single-keypress input (no Enter needed).

use recipe_utils::{
    connect_reactor, get_or_create_app, print_join_instructions, random_color, random_name, BOLD,
    DIM, RESET,
};
use serde::{Deserialize, Serialize};
use sharing_instant::rooms::Room;
use std::io::Read;

#[derive(Clone, Serialize, Deserialize, Debug)]
struct UserPresence {
    name: String,
    color: String,
    status: String,
}

/// Put the terminal into raw mode (no line buffering, no echo).
/// Returns the original termios to restore later.
fn enable_raw_mode() -> libc::termios {
    unsafe {
        let mut orig: libc::termios = std::mem::zeroed();
        libc::tcgetattr(libc::STDIN_FILENO, &mut orig);
        let mut raw = orig;
        raw.c_lflag &= !(libc::ICANON | libc::ECHO);
        raw.c_cc[libc::VMIN] = 1;
        raw.c_cc[libc::VTIME] = 0;
        libc::tcsetattr(libc::STDIN_FILENO, libc::TCSANOW, &raw);
        orig
    }
}

fn restore_terminal(orig: &libc::termios) {
    unsafe {
        libc::tcsetattr(libc::STDIN_FILENO, libc::TCSANOW, orig);
    }
}

#[tokio::main]
async fn main() {
    println!("\n  {BOLD}Avatar Stack{RESET} — who's online\n");

    let app = get_or_create_app("avatar-stack")
        .await
        .expect("app setup failed");
    print_join_instructions(&app, "avatar-stack");

    let reactor = connect_reactor(&app)
        .await
        .expect("reactor connect failed");
    let handle = tokio::runtime::Handle::current();

    let (color_name, color_ansi) = random_color();
    let name = random_name();
    println!("  You are: {color_ansi}{name}{RESET} ({color_name})");
    println!();
    println!("  {DIM}Keys: [o]nline  [a]way  [b]usy  [q]uit{RESET}");
    println!();

    let room = Room::<UserPresence>::join(reactor.clone(), handle.clone(), "avatar", "lobby")
        .expect("failed to join room");

    // Small delay to let join_room complete before setting presence
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;

    let my_presence = UserPresence {
        name: name.clone(),
        color: color_name.to_string(),
        status: "online".to_string(),
    };
    room.set_presence(&my_presence)
        .expect("failed to set presence");

    // Background watcher
    let mut rx = room.watch_presence();
    let my_name = name.clone();
    let my_color_ansi = color_ansi.to_string();
    tokio::spawn(async move {
        while rx.changed().await.is_ok() {
            let state = rx.borrow().clone();

            println!();
            println!("  {DIM}--- presence update ---{RESET}");
            if let Some(ref me) = state.user {
                let icon = status_icon(&me.status);
                println!(
                    "  {my_color_ansi}{icon} {my_name}{RESET} {DIM}(you, {}){RESET}",
                    me.status
                );
            }
            if state.peers.is_empty() {
                println!("  {DIM}(no peers yet){RESET}");
            } else {
                for (_peer_id, p) in &state.peers {
                    let ansi = color_to_ansi(&p.color);
                    let icon = status_icon(&p.status);
                    println!(
                        "  {ansi}{icon} {}{RESET} {DIM}({}){RESET}",
                        p.name, p.status
                    );
                }
            }
        }
    });

    // Raw keypress loop (single char, no Enter)
    let orig_termios = enable_raw_mode();
    let mut stdin = std::io::stdin().lock();
    let mut buf = [0u8; 1];

    loop {
        match stdin.read(&mut buf) {
            Ok(1) => match buf[0] {
                b'q' | b'Q' | 3 => break, // q or Ctrl-C
                b'o' | b'O' => {
                    let p = UserPresence {
                        name: name.clone(),
                        color: color_name.to_string(),
                        status: "online".to_string(),
                    };
                    room.set_presence(&p).expect("failed to set presence");
                    println!("  -> online");
                }
                b'a' | b'A' => {
                    let p = UserPresence {
                        name: name.clone(),
                        color: color_name.to_string(),
                        status: "away".to_string(),
                    };
                    room.set_presence(&p).expect("failed to set presence");
                    println!("  -> away");
                }
                b'b' | b'B' => {
                    let p = UserPresence {
                        name: name.clone(),
                        color: color_name.to_string(),
                        status: "busy".to_string(),
                    };
                    room.set_presence(&p).expect("failed to set presence");
                    println!("  -> busy");
                }
                _ => {}
            },
            _ => break,
        }
    }

    restore_terminal(&orig_termios);
    room.leave();
    reactor.stop().await;
    println!("\n  Goodbye!");
}

fn color_to_ansi(color: &str) -> &'static str {
    match color {
        "red" => "\x1b[31m",
        "green" => "\x1b[32m",
        "yellow" => "\x1b[33m",
        "blue" => "\x1b[34m",
        "magenta" => "\x1b[35m",
        "cyan" => "\x1b[36m",
        _ => "\x1b[37m",
    }
}

fn status_icon(status: &str) -> &'static str {
    match status {
        "online" => "\u{25cf}", // ●
        "away" => "\u{25d1}",   // ◑
        "busy" => "\u{25cb}",   // ○
        _ => "?",
    }
}
