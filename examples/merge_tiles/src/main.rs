//! Merge Tiles — collaborative 4x4 colored grid.
//!
//! Demonstrates real-time database subscribe + transact with a visual grid.
//! Each tile is a database entity with a color. Multiple users can paint
//! tiles simultaneously and see changes in real-time.

use futures::StreamExt;
use instant_client::async_api::InstantAsync;
use instant_client::connection::ConnectionConfig;
use parking_lot::RwLock;
use recipe_utils::{get_or_create_app, print_join_instructions, BOLD, DIM, RESET};
use serde_json::json;
use std::sync::Arc;

const GRID_SIZE: usize = 4;

const TILE_COLORS: &[(&str, &str)] = &[
    ("red", "\x1b[41m"),
    ("green", "\x1b[42m"),
    ("yellow", "\x1b[43m"),
    ("blue", "\x1b[44m"),
    ("magenta", "\x1b[45m"),
    ("cyan", "\x1b[46m"),
    ("white", "\x1b[47m"),
    ("gray", "\x1b[100m"),
];

fn tile_id(row: usize, col: usize) -> String {
    format!("00000000-0000-0000-{:04x}-{:012x}", row, col)
}

fn color_bg(name: &str) -> &'static str {
    TILE_COLORS
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(_, c)| *c)
        .unwrap_or("\x1b[100m")
}

#[tokio::main]
async fn main() {
    println!("\n  {BOLD}Merge Tiles{RESET} — collaborative 4x4 grid\n");

    let app = get_or_create_app("merge-tiles")
        .await
        .expect("app setup failed");
    print_join_instructions(&app, "merge-tiles");

    let config = ConnectionConfig::admin(&app.id, &app.admin_token);
    let client = InstantAsync::new(config)
        .await
        .expect("WebSocket connect failed");

    // Seed tiles if empty
    let q = json!({"tiles": {}});
    let initial = client.subscribe(&q).await.next().await;
    let needs_seed = initial
        .as_ref()
        .and_then(|v| v.get("tiles"))
        .and_then(|v| v.as_array())
        .map(|a| a.is_empty())
        .unwrap_or(true);

    if needs_seed {
        println!("  Seeding 4x4 grid...");
        let mut steps = Vec::new();
        for row in 0..GRID_SIZE {
            for col in 0..GRID_SIZE {
                let id = tile_id(row, col);
                steps.push(json!(["update", "tiles", &id, {
                    "row": row, "col": col, "color": "gray"
                }]));
            }
        }
        client
            .transact(&json!(steps))
            .await
            .expect("seed transact failed");
    }

    println!("  Connected!\n");
    println!("  {DIM}Colors: red, green, yellow, blue, magenta, cyan, white, gray{RESET}");
    println!("  {DIM}Commands: <row> <col> <color>  (e.g., '1 2 red'), reset, quit{RESET}\n");

    let grid: Arc<RwLock<[[String; GRID_SIZE]; GRID_SIZE]>> =
        Arc::new(RwLock::new(std::array::from_fn(|_| {
            std::array::from_fn(|_| "gray".to_string())
        })));

    // Background watcher
    let grid_bg = grid.clone();
    let mut stream = client.subscribe(&json!({"tiles": {}})).await;
    tokio::spawn(async move {
        while let Some(data) = stream.next().await {
            if let Some(tiles) = data.get("tiles").and_then(|v| v.as_array()) {
                let mut g = grid_bg.write();
                for tile in tiles {
                    let row = tile.get("row").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
                    let col = tile.get("col").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
                    let color = tile.get("color").and_then(|v| v.as_str()).unwrap_or("gray");
                    if row < GRID_SIZE && col < GRID_SIZE {
                        g[row][col] = color.to_string();
                    }
                }
                drop(g);
                render_grid(&grid_bg.read());
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

        if line == "reset" {
            let mut steps = Vec::new();
            for row in 0..GRID_SIZE {
                for col in 0..GRID_SIZE {
                    let id = tile_id(row, col);
                    steps.push(json!(["update", "tiles", &id, {"color": "gray"}]));
                }
            }
            match client.transact(&json!(steps)).await {
                Ok(_) => println!("  Grid reset!"),
                Err(e) => println!("  Error: {e}"),
            }
            recipe_utils::prompt("  > ");
            continue;
        }

        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() == 3 {
            if let (Ok(row), Ok(col)) = (parts[0].parse::<usize>(), parts[1].parse::<usize>()) {
                let color = parts[2];
                if row >= GRID_SIZE || col >= GRID_SIZE {
                    println!("  Row/col must be 0-{}", GRID_SIZE - 1);
                } else if !TILE_COLORS.iter().any(|(n, _)| *n == color) {
                    println!("  Unknown color. Try: red, green, yellow, blue, magenta, cyan, white, gray");
                } else {
                    let id = tile_id(row, col);
                    let tx = json!([["update", "tiles", &id, {"color": color}]]);
                    match client.transact(&tx).await {
                        Ok(_) => {}
                        Err(e) => println!("  Error: {e}"),
                    }
                }
            } else {
                println!("  Usage: <row> <col> <color>  (e.g., '1 2 red')");
            }
        } else {
            println!("  Usage: <row> <col> <color>, reset, quit");
        }
        recipe_utils::prompt("  > ");
    }

    client.close().await;
    println!("\n  Goodbye!");
}

fn render_grid(grid: &[[String; GRID_SIZE]; GRID_SIZE]) {
    print!("\x1b[2J\x1b[H");
    println!("  {BOLD}Merge Tiles{RESET} — 4x4 collaborative grid\n");

    // Column headers
    print!("      ");
    for col in 0..GRID_SIZE {
        print!("  {col} ");
    }
    println!();

    // Top border
    print!("    \u{250c}");
    for i in 0..GRID_SIZE {
        print!("\u{2500}\u{2500}\u{2500}");
        if i < GRID_SIZE - 1 {
            print!("\u{252c}");
        }
    }
    println!("\u{2510}");

    for (row_idx, row) in grid.iter().enumerate() {
        print!("  {row_idx} \u{2502}");
        for color_name in row {
            let bg = color_bg(color_name);
            print!("{bg}   {RESET}\u{2502}");
        }
        println!();

        if row_idx < GRID_SIZE - 1 {
            print!("    \u{251c}");
            for i in 0..GRID_SIZE {
                print!("\u{2500}\u{2500}\u{2500}");
                if i < GRID_SIZE - 1 {
                    print!("\u{253c}");
                }
            }
            println!("\u{2524}");
        }
    }

    // Bottom border
    print!("    \u{2514}");
    for i in 0..GRID_SIZE {
        print!("\u{2500}\u{2500}\u{2500}");
        if i < GRID_SIZE - 1 {
            print!("\u{2534}");
        }
    }
    println!("\u{2518}");
    println!();
}
