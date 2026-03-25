use instant_admin::ephemeral::{create_ephemeral_app, EphemeralApp};
use instant_client::{ConnectionConfig, Reactor};
use rand::Rng;
use std::path::PathBuf;
use std::sync::Arc;

const ADJECTIVES: &[&str] = &[
    "swift", "brave", "calm", "eager", "fair", "keen", "bold", "warm", "wise", "cool", "glad",
    "kind", "neat", "pure", "rich", "safe",
];

const NOUNS: &[&str] = &[
    "fox", "owl", "elk", "bee", "cat", "dog", "ant", "bat", "emu", "yak", "hen", "jay", "ram",
    "cod", "ape", "gnu",
];

const COLORS: &[(&str, &str)] = &[
    ("red", "\x1b[31m"),
    ("green", "\x1b[32m"),
    ("yellow", "\x1b[33m"),
    ("blue", "\x1b[34m"),
    ("magenta", "\x1b[35m"),
    ("cyan", "\x1b[36m"),
];

/// ANSI reset code.
pub const RESET: &str = "\x1b[0m";

/// ANSI dim code.
pub const DIM: &str = "\x1b[2m";

/// ANSI bold code.
pub const BOLD: &str = "\x1b[1m";

/// Path to the shared app credentials file.
fn shared_app_path() -> PathBuf {
    // Walk up from CWD looking for the workspace Cargo.toml,
    // but fall back to CWD if not found.
    let mut dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    loop {
        let candidate = dir.join("Cargo.toml");
        if candidate.exists() {
            if let Ok(contents) = std::fs::read_to_string(&candidate) {
                if contents.contains("[workspace]") {
                    return dir.join(".instant-app.json");
                }
            }
        }
        if !dir.pop() {
            break;
        }
    }
    PathBuf::from(".instant-app.json")
}

fn save_app(app: &EphemeralApp) {
    let json = format!(
        r#"{{"id":"{}","admin_token":"{}","title":"{}"}}"#,
        app.id, app.admin_token, app.title
    );
    if let Err(e) = std::fs::write(shared_app_path(), json) {
        eprintln!("  {DIM}Warning: could not save app credentials: {e}{RESET}");
    }
}

fn load_app() -> Option<EphemeralApp> {
    let path = shared_app_path();
    let contents = std::fs::read_to_string(path).ok()?;
    // Minimal JSON parsing — avoid adding serde dep to recipe_utils
    let id = extract_json_field(&contents, "id")?;
    let admin_token = extract_json_field(&contents, "admin_token")?;
    let title = extract_json_field(&contents, "title").unwrap_or_default();
    Some(EphemeralApp {
        id,
        admin_token,
        title,
    })
}

fn extract_json_field(json: &str, key: &str) -> Option<String> {
    let pattern = format!("\"{}\":\"", key);
    let start = json.find(&pattern)? + pattern.len();
    let end = json[start..].find('"')? + start;
    Some(json[start..end].to_string())
}

/// Generate a random two-word name like "swift-fox".
pub fn random_name() -> String {
    let mut rng = rand::thread_rng();
    let adj = ADJECTIVES[rng.gen_range(0..ADJECTIVES.len())];
    let noun = NOUNS[rng.gen_range(0..NOUNS.len())];
    format!("{}-{}", adj, noun)
}

/// Pick a random ANSI color. Returns (name, ansi_code).
pub fn random_color() -> (&'static str, &'static str) {
    let mut rng = rand::thread_rng();
    COLORS[rng.gen_range(0..COLORS.len())]
}

/// Returns true if `--ephemeral` was passed on the command line.
pub fn is_ephemeral() -> bool {
    std::env::args().any(|a| a == "--ephemeral")
}

/// Get the shared app, or create one if needed.
///
/// **Default**: loads credentials from `.instant-app.json` in the workspace root.
/// If the file doesn't exist yet, creates an ephemeral app and saves it so all
/// examples (and all terminals) share the same InstantDB app.
///
/// **`--ephemeral`**: creates a fresh ephemeral app without saving. Use this
/// when you want an isolated app that won't interfere with other examples.
///
/// **Env vars**: `INSTANT_APP_ID` + `INSTANT_ADMIN_TOKEN` override everything.
pub async fn get_or_create_app(name: &str) -> Result<EphemeralApp, String> {
    // 1. Env vars always win
    if let (Ok(app_id), Ok(admin_token)) = (
        std::env::var("INSTANT_APP_ID"),
        std::env::var("INSTANT_ADMIN_TOKEN"),
    ) {
        return Ok(EphemeralApp {
            id: app_id,
            admin_token,
            title: name.to_string(),
        });
    }

    // 2. --ephemeral → fresh app, don't save
    if is_ephemeral() {
        println!("  {DIM}Creating ephemeral app (--ephemeral)...{RESET}");
        let app = create_ephemeral_app(name)
            .await
            .map_err(|e| format!("Failed to create ephemeral app: {e}"))?;
        println!("  {DIM}App ID: {} (ephemeral, not saved){RESET}", &app.id[..8]);
        return Ok(app);
    }

    // 3. Try loading shared app from file
    if let Some(app) = load_app() {
        println!("  {DIM}Using shared app: {}{RESET}", &app.id[..8]);
        return Ok(app);
    }

    // 4. First run — create and save
    println!("  {DIM}First run — creating shared app...{RESET}");
    let app = create_ephemeral_app("shared-recipes")
        .await
        .map_err(|e| format!("Failed to create ephemeral app: {e}"))?;
    save_app(&app);
    println!(
        "  {DIM}Saved to {} (all examples will share this app){RESET}",
        shared_app_path().display()
    );
    Ok(app)
}

/// Print app info (ID + dashboard link) and join instructions.
pub fn print_join_instructions(app: &EphemeralApp, bin_name: &str) {
    println!("  {DIM}App ID:{RESET}     {}", app.id);
    println!(
        "  {DIM}Dashboard:{RESET}  https://instantdb.com/dash?s=main&t=explorer&app={}",
        app.id
    );

    if is_ephemeral() {
        println!("\n  {DIM}To join from another terminal:{RESET}");
        println!(
            "  INSTANT_APP_ID={} INSTANT_ADMIN_TOKEN={} cargo run -p {bin_name}\n",
            app.id, app.admin_token
        );
    } else {
        println!("\n  {DIM}To join from another terminal:{RESET}");
        println!("  cargo run -p {bin_name}\n");
    }
}

/// Create a Reactor connected to the app and start it.
pub async fn connect_reactor(app: &EphemeralApp) -> Result<Arc<Reactor>, String> {
    let config = ConnectionConfig::admin(&app.id, &app.admin_token);
    let reactor = Arc::new(Reactor::new(config));
    reactor
        .start()
        .await
        .map_err(|e| format!("Reactor start failed: {e}"))?;
    // Brief pause for InitOk (attrs catalog)
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    Ok(reactor)
}

/// Flush stdout.
pub fn flush() {
    use std::io::Write;
    std::io::stdout().flush().ok();
}

/// Print a prompt marker and flush.
pub fn prompt(marker: &str) {
    print!("{marker}");
    flush();
}
