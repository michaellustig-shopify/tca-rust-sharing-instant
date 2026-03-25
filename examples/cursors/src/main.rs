//! Cursors — presence with random-walk cursor on an ASCII grid.
//!
//! Demonstrates `Room<P>` with a timer-driven presence update.
//! Each peer's cursor moves randomly every 500ms. A watcher renders
//! a 40x20 ASCII grid with peer initials in ANSI colors.

use rand::Rng;
use recipe_utils::{
    connect_reactor, get_or_create_app, print_join_instructions, random_color, random_name, BOLD,
    DIM, RESET,
};
use serde::{Deserialize, Serialize};
use sharing_instant::rooms::Room;
use std::sync::Arc;
use tokio::sync::Mutex;

const WIDTH: i32 = 40;
const HEIGHT: i32 = 20;

#[derive(Clone, Serialize, Deserialize)]
struct CursorPresence {
    name: String,
    color: String,
    x: i32,
    y: i32,
}

#[tokio::main]
async fn main() {
    println!("\n  {BOLD}Cursors{RESET} — random-walk on ASCII grid\n");

    let app = get_or_create_app("cursors")
        .await
        .expect("app setup failed");
    print_join_instructions(&app, "cursors");

    let reactor = connect_reactor(&app).await.expect("reactor connect failed");
    let handle = tokio::runtime::Handle::current();

    let (color_name, _) = random_color();
    let name = random_name();
    println!("  You are: {}{name}{RESET}\n", color_to_ansi(color_name));

    let room = Room::<CursorPresence>::join(reactor.clone(), handle.clone(), "cursors", "canvas")
        .expect("failed to join room");

    let pos = Arc::new(Mutex::new((WIDTH / 2, HEIGHT / 2)));

    // Set initial presence
    {
        let (x, y) = *pos.lock().await;
        room.set_presence(&CursorPresence {
            name: name.clone(),
            color: color_name.to_string(),
            x,
            y,
        })
        .expect("failed to set presence");
    }

    // Random walk timer — move every 500ms
    let walk_room_reactor = reactor.clone();
    let walk_handle = handle.clone();
    let walk_name = name.clone();
    let walk_color = color_name.to_string();
    let walk_pos = pos.clone();
    tokio::spawn(async move {
        let walk_room =
            Room::<CursorPresence>::join(walk_room_reactor, walk_handle, "cursors", "canvas")
                .expect("walk room join failed");

        let mut interval = tokio::time::interval(std::time::Duration::from_millis(500));
        loop {
            interval.tick().await;
            let (dx, dy) = {
                let mut rng = rand::thread_rng();
                (rng.gen_range(-1..=1), rng.gen_range(-1..=1))
            };
            let mut p = walk_pos.lock().await;
            p.0 = (p.0 + dx).clamp(0, WIDTH - 1);
            p.1 = (p.1 + dy).clamp(0, HEIGHT - 1);
            let _ = walk_room.set_presence(&CursorPresence {
                name: walk_name.clone(),
                color: walk_color.clone(),
                x: p.0,
                y: p.1,
            });
        }
    });

    // Background watcher — renders the grid
    let mut rx = room.watch_presence();
    let render_name = name.clone();
    let render_color = color_name.to_string();
    let render_pos = pos.clone();
    tokio::spawn(async move {
        while rx.changed().await.is_ok() {
            let state = rx.borrow().clone();
            let my_pos = *render_pos.lock().await;

            // Collect all cursors (peers + self)
            let mut cursors: Vec<CursorPresence> = state.peers.values().cloned().collect();
            cursors.push(CursorPresence {
                name: render_name.clone(),
                color: render_color.clone(),
                x: my_pos.0,
                y: my_pos.1,
            });

            render_grid(&cursors);
        }
    });

    // CLI loop — just wait for quit
    println!("  {DIM}Cursor moves automatically. Type 'quit' to exit.{RESET}\n");
    let reader = tokio::io::BufReader::new(tokio::io::stdin());
    let mut lines = tokio::io::AsyncBufReadExt::lines(reader);

    loop {
        let line = match lines.next_line().await {
            Ok(Some(l)) => l,
            _ => break,
        };
        if line.trim() == "quit" || line.trim() == "q" {
            break;
        }
    }

    room.leave();
    reactor.stop().await;
    println!("\n  Goodbye!");
}

fn render_grid(cursors: &[CursorPresence]) {
    // Build grid
    let mut grid = vec![vec![(' ', "\x1b[0m"); WIDTH as usize]; HEIGHT as usize];

    for c in cursors {
        let x = c.x.clamp(0, WIDTH - 1) as usize;
        let y = c.y.clamp(0, HEIGHT - 1) as usize;
        let initial = c.name.chars().next().unwrap_or('?');
        let ansi = color_to_ansi(&c.color);
        grid[y][x] = (initial, ansi);
    }

    // Clear screen and draw
    print!("\x1b[2J\x1b[H");
    println!(
        "  {BOLD}Cursors{RESET} — {DIM}{} peer(s){RESET}\n",
        cursors.len()
    );

    // Top border
    print!("  \u{250c}");
    for _ in 0..WIDTH {
        print!("\u{2500}");
    }
    println!("\u{2510}");

    for row in &grid {
        print!("  \u{2502}");
        for (ch, ansi) in row {
            if *ch == ' ' {
                print!("{DIM}\u{00b7}{RESET}");
            } else {
                print!("{ansi}{BOLD}{ch}{RESET}");
            }
        }
        println!("\u{2502}");
    }

    // Bottom border
    print!("  \u{2514}");
    for _ in 0..WIDTH {
        print!("\u{2500}");
    }
    println!("\u{2518}");

    // Legend
    println!();
    for c in cursors {
        let ansi = color_to_ansi(&c.color);
        let initial = c.name.chars().next().unwrap_or('?');
        println!(
            "  {ansi}{BOLD}{initial}{RESET} = {} {DIM}({}, {}){RESET}",
            c.name, c.x, c.y
        );
    }
    println!("\n  {DIM}Type 'quit' to exit.{RESET}");
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
