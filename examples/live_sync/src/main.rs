use futures::StreamExt;
use instant_admin::ephemeral::create_ephemeral_app;
use instant_client::async_api::InstantAsync;
use instant_client::connection::ConnectionConfig;
use serde_json::json;
use std::sync::Arc;

fn uuid() -> String {
    uuid::Uuid::new_v4().to_string()
}

#[tokio::main]
async fn main() {
    println!("╔═══════════════════════════════════════════════════════╗");
    println!("║  sharing-instant — Live Sync Demo                    ║");
    println!("║  Rust CLI ↔ Browser, real-time via InstantDB          ║");
    println!("╚═══════════════════════════════════════════════════════╝\n");

    // ── Step 1: Create ephemeral InstantDB app ──
    println!("[1/5] Creating ephemeral InstantDB app...");
    let app = create_ephemeral_app("sharing-instant-demo")
        .await
        .expect("Failed to create ephemeral app");
    println!("      App ID: {}", app.id);
    println!("      Expires: 14 days\n");

    // ── Step 2: Seed data via admin client ──
    println!("[2/5] Seeding data...");
    let admin = instant_admin::AdminClient::new(&app.id, &app.admin_token);

    let proj_personal = uuid();
    let proj_work = uuid();
    let tx = json!([
        ["update", "projects", &proj_personal, {"name": "Personal", "color": "#58a6ff"}],
        ["update", "projects", &proj_work,     {"name": "Work",     "color": "#f85149"}],

        ["update", "todos", uuid(), {"text": "Buy quantum pasta",          "done": false, "ts": 1, "projectId": &proj_personal}],
        ["update", "todos", uuid(), {"text": "Walk Oscar (the dog)",       "done": false, "ts": 2, "projectId": &proj_personal}],
        ["update", "todos", uuid(), {"text": "Prototype treat cannon",     "done": false, "ts": 3, "projectId": &proj_personal}],
        ["update", "todos", uuid(), {"text": "Ship feature branch",        "done": false, "ts": 4, "projectId": &proj_work}],
        ["update", "todos", uuid(), {"text": "Review recursive lasagna PR","done": false, "ts": 5, "projectId": &proj_work}],
    ]);
    let result = admin.transact(&tx).await.expect("Seed transact failed");
    println!("      Seeded: 2 projects, 5 todos");
    println!(
        "      tx-id: {}\n",
        result.get("tx-id").unwrap_or(&json!("?"))
    );

    // ── Step 3: Verify with nested query ──
    println!("[3/5] Verifying nested query...");
    let q = json!({ "projects": {}, "todos": {} });
    let data = admin.query(&q).await.expect("Query failed");
    let proj_count = data
        .get("projects")
        .and_then(|v| v.as_array())
        .map(|a| a.len())
        .unwrap_or(0);
    let todo_count = data
        .get("todos")
        .and_then(|v| v.as_array())
        .map(|a| a.len())
        .unwrap_or(0);
    println!("      {} projects, {} todos\n", proj_count, todo_count);

    // ── Step 4: Generate HTML ──
    println!("[4/5] Generating browser app...");
    let html = generate_html(&app.id);
    std::fs::write("live_sync.html", &html).expect("Failed to write HTML");
    println!("      Written: live_sync.html\n");

    // ── Step 5: Open browser & start CLI ──
    println!("[5/5] Opening browser...");
    let _ = std::process::Command::new("open")
        .arg("live_sync.html")
        .spawn();

    println!();
    println!("══════════════════════════════════════════════════════════");
    println!("  Rust CLI + Browser connected to the same InstantDB.");
    println!("  Changes sync in real-time via WebSocket.");
    println!("══════════════════════════════════════════════════════════\n");

    // ── Connect WebSocket client ──
    let config = ConnectionConfig::admin(&app.id, &app.admin_token);
    let client = InstantAsync::new(config)
        .await
        .expect("Failed to connect WebSocket");

    println!("  Connected to InstantDB WebSocket\n");
    println!("  Commands:");
    println!("    add <text>    — Add a todo (syncs to browser)");
    println!("    done <index>  — Toggle todo done");
    println!("    del <index>   — Delete a todo");
    println!("    list          — Show current todos");
    println!("    quit          — Exit\n");

    // Subscribe to todos via the watch::Receiver directly (more reliable than Stream)
    let todo_rx = client
        .subscribe(&json!({"todos": {}}))
        .await;

    // Wrap into a shared vec we can read from the CLI loop
    let current_todos: Arc<parking_lot::RwLock<Vec<serde_json::Value>>> =
        Arc::new(parking_lot::RwLock::new(Vec::new()));

    // Background watcher: reads from the stream and updates current_todos
    let todos_bg = current_todos.clone();
    let watch_handle = tokio::spawn(async move {
        let mut stream = todo_rx;
        while let Some(data) = stream.next().await {
            if let Some(todos_arr) = data.get("todos").and_then(|v| v.as_array()) {
                let mut sorted = todos_arr.clone();
                sorted.sort_by(|a, b| {
                    let ta = a.get("ts").and_then(|v| v.as_i64()).unwrap_or(0);
                    let tb = b.get("ts").and_then(|v| v.as_i64()).unwrap_or(0);
                    ta.cmp(&tb)
                });
                let count = sorted.len();
                let is_first = todos_bg.read().is_empty();
                *todos_bg.write() = sorted;

                if is_first {
                    println!("  [sync] Initial load: {} todos", count);
                } else {
                    println!("\n  [sync] Update: {} todos", count);
                }
                print_todos(&todos_bg.read());
                print!("\n  > ");
                use std::io::Write;
                std::io::stdout().flush().ok();
            }
        }
        println!("  [sync] Subscription ended");
    });

    // CLI input loop
    let reader = tokio::io::BufReader::new(tokio::io::stdin());
    let mut lines = tokio::io::AsyncBufReadExt::lines(reader);

    loop {
        print!("  > ");
        use std::io::Write;
        std::io::stdout().flush().ok();

        let line = match lines.next_line().await {
            Ok(Some(l)) => l,
            _ => break,
        };
        let line = line.trim().to_string();
        if line.is_empty() {
            continue;
        }

        if line == "quit" || line == "q" {
            break;
        }

        if line == "list" || line == "ls" {
            print_todos(&current_todos.read());
            continue;
        }

        if line.starts_with("add ") {
            let text = &line[4..];
            let id = uuid();
            let ts = chrono_ts();
            let tx = json!([["update", "todos", &id, {"text": text, "done": false, "ts": ts}]]);
            match client.transact(&tx).await {
                Ok(_) => println!("  Added: {}", text),
                Err(e) => println!("  Error: {}", e),
            }
            continue;
        }

        if line.starts_with("done ") {
            if let Ok(idx) = line[5..].trim().parse::<usize>() {
                let todos = current_todos.read();
                if idx > 0 && idx <= todos.len() {
                    let todo = &todos[idx - 1];
                    let id = todo.get("id").and_then(|v| v.as_str()).unwrap_or("");
                    let done = todo.get("done").and_then(|v| v.as_bool()).unwrap_or(false);
                    let id = id.to_string();
                    drop(todos);
                    let tx = json!([["update", "todos", &id, {"done": !done}]]);
                    match client.transact(&tx).await {
                        Ok(_) => println!("  Toggled #{}", idx),
                        Err(e) => println!("  Error: {}", e),
                    }
                } else {
                    println!("  Invalid index (1-{})", todos.len());
                }
            }
            continue;
        }

        if line.starts_with("del ") {
            if let Ok(idx) = line[4..].trim().parse::<usize>() {
                let todos = current_todos.read();
                if idx > 0 && idx <= todos.len() {
                    let todo = &todos[idx - 1];
                    let id = todo
                        .get("id")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    drop(todos);
                    let tx = json!([["delete", "todos", &id, {}]]);
                    match client.transact(&tx).await {
                        Ok(_) => println!("  Deleted #{}", idx),
                        Err(e) => println!("  Error: {}", e),
                    }
                } else {
                    println!("  Invalid index (1-{})", todos.len());
                }
            }
            continue;
        }

        println!("  Unknown command. Try: add, done, del, list, quit");
    }

    watch_handle.abort();
    client.close().await;
    println!("\n  Goodbye!");
}

fn chrono_ts() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("time went backwards")
        .as_millis() as i64
}

fn print_todos(todos: &[serde_json::Value]) {
    if todos.is_empty() {
        println!("  (no todos yet — waiting for sync...)");
        return;
    }
    for (i, todo) in todos.iter().enumerate() {
        let text = todo.get("text").and_then(|v| v.as_str()).unwrap_or("?");
        let done = todo.get("done").and_then(|v| v.as_bool()).unwrap_or(false);
        let mark = if done { "\x1b[32m✓\x1b[0m" } else { "○" };
        let style = if done { "\x1b[9;2m" } else { "" };
        let reset = if done { "\x1b[0m" } else { "" };
        println!("  {}. {} {}{}{}", i + 1, mark, style, text, reset);
    }
}

fn generate_html(app_id: &str) -> String {
    format!(
        r##"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<title>sharing-instant — Live Sync</title>
<style>
:root {{
  --bg: #0d1117; --surface: #161b22; --border: #30363d;
  --text: #e6edf3; --dim: #8b949e; --accent: #58a6ff;
  --green: #3fb950; --orange: #d29922; --red: #f85149;
  --purple: #bc8cff; --mono: 'SF Mono','Fira Code',monospace;
}}
* {{ box-sizing:border-box; margin:0; padding:0; }}
body {{ background:var(--bg); color:var(--text); font-family:var(--mono);
       font-size:13px; line-height:1.6; max-width:700px; margin:0 auto; padding:24px; }}
h1 {{ color:var(--accent); font-size:18px; }}
.sub {{ color:var(--dim); font-size:11px; margin-bottom:16px; }}
.status {{ display:inline-flex; align-items:center; gap:6px; padding:4px 10px;
           background:var(--surface); border:1px solid var(--border);
           border-radius:6px; font-size:11px; margin-bottom:16px; }}
.dot {{ width:8px; height:8px; border-radius:50%; }}
.dot.ok {{ background:var(--green); animation:pulse 2s infinite; }}
.dot.no {{ background:var(--red); }}
@keyframes pulse {{ 0%,100% {{ opacity:1 }} 50% {{ opacity:.4 }} }}
.card {{ background:var(--surface); border:1px solid var(--border);
         border-radius:8px; padding:16px; margin-bottom:12px; }}
.card h2 {{ color:var(--purple); font-size:14px; margin-bottom:10px; }}
.todo {{ display:flex; align-items:center; gap:10px; padding:8px 0;
         border-bottom:1px solid #1e2430; cursor:default; }}
.todo:last-child {{ border:none; }}
.todo input {{ width:18px; height:18px; accent-color:var(--accent); cursor:pointer; }}
.todo.done label {{ text-decoration:line-through; color:var(--dim); }}
.todo .del {{ margin-left:auto; background:none; border:none;
              color:var(--red); cursor:pointer; opacity:.5; font-size:14px;
              font-family:var(--mono); }}
.todo .del:hover {{ opacity:1; }}
.add-row {{ display:flex; gap:8px; margin-top:12px; }}
.add-row input {{ flex:1; background:#0d1117; border:1px solid var(--border);
                  color:var(--text); padding:8px 12px; border-radius:6px;
                  font-family:var(--mono); font-size:13px; outline:none; }}
.add-row input:focus {{ border-color:var(--accent); }}
.add-row button {{ background:var(--accent); color:#0d1117; border:none;
                   padding:8px 16px; border-radius:6px; cursor:pointer;
                   font-family:var(--mono); font-weight:bold; }}
.add-row button:hover {{ opacity:.9; }}
.log {{ background:#0d1117; border:1px solid var(--border); border-radius:6px;
        padding:10px; max-height:200px; overflow-y:auto; font-size:11px; }}
.log-entry {{ padding:3px 0; border-bottom:1px solid #1a1f2e; display:flex; gap:8px; }}
.log-entry:last-child {{ border:none; }}
.log-ts {{ color:var(--dim); min-width:70px; }}
.log-op {{ color:var(--orange); min-width:50px; }}
.nested {{ background:#0d1117; border:1px solid var(--border); border-radius:6px;
           padding:10px; font-size:11px; white-space:pre-wrap; overflow-x:auto;
           max-height:300px; overflow-y:auto; }}
.hint {{ color:var(--dim); font-size:11px; margin:8px 0; }}
.count {{ color:var(--accent); font-weight:bold; }}
</style>
</head>
<body>

<h1>sharing-instant — Live Sync</h1>
<div class="sub">Browser ↔ Rust CLI — same InstantDB app, real-time WebSocket sync</div>

<div class="status">
  <div class="dot no" id="dot"></div>
  <span id="status-text">connecting...</span>
</div>

<div class="card">
  <h2>Todos <span class="count" id="todo-count">(0)</span></h2>
  <div id="todo-list"><div style="color:var(--dim)">connecting...</div></div>
  <div class="add-row">
    <input id="new-todo" placeholder="Add from browser..." onkeydown="if(event.key==='Enter')window.addTodo()">
    <button onclick="window.addTodo()">Add</button>
  </div>
  <div class="hint">
    Add here or type <code style="background:#1a1f2e;padding:1px 6px;border-radius:3px;color:var(--green)">add &lt;text&gt;</code> in the Rust CLI — both sync instantly.
  </div>
</div>

<div class="card">
  <h2>Nested Query (projects → todos)</h2>
  <div class="nested" id="nested-out">loading...</div>
</div>

<div class="card">
  <h2>Sync Log</h2>
  <div class="log" id="sync-log"><div style="color:var(--dim)">waiting for connection...</div></div>
</div>

<script type="module">
import {{ init, tx, id }} from 'https://esm.sh/@instantdb/core@0.17.4';

const APP_ID = '{app_id}';
const db = init({{ appId: APP_ID }});
const logs = [];
let updateCount = 0;

function log(op, detail) {{
  const ts = new Date().toLocaleTimeString('en-US', {{ hour12:false }});
  logs.unshift({{ ts, op, detail }});
  if (logs.length > 50) logs.pop();
  document.getElementById('sync-log').innerHTML = logs.map(e =>
    `<div class="log-entry"><span class="log-ts">${{e.ts}}</span><span class="log-op">${{e.op}}</span><span>${{e.detail}}</span></div>`
  ).join('');
}}

// ── Todos subscription ──
db.subscribeQuery({{ todos: {{}} }}, (r) => {{
  if (r.error) {{ log('ERROR', r.error.message); return; }}
  const todos = (r.data?.todos || []).sort((a,b) => (a.ts||0) - (b.ts||0));
  updateCount++;

  document.getElementById('todo-count').textContent = `(${{todos.length}})`;
  document.getElementById('todo-list').innerHTML = todos.length === 0
    ? '<div style="color:var(--dim)">(no todos)</div>'
    : todos.map(t => `
    <div class="todo ${{t.done ? 'done' : ''}}">
      <input type="checkbox" id="cb-${{t.id}}" ${{t.done ? 'checked' : ''}}
             onchange="window._toggle('${{t.id}}', ${{!t.done}})">
      <label for="cb-${{t.id}}">${{t.text}}</label>
      <button class="del" onclick="window._del('${{t.id}}')">&times;</button>
    </div>
  `).join('');

  // Mark connected on first data
  if (updateCount === 1) {{
    document.getElementById('dot').className = 'dot ok';
    document.getElementById('status-text').textContent = 'connected — syncing with Rust CLI';
    log('CONNECT', `app ${{APP_ID.slice(0,8)}}...`);
  }}
  log('SYNC', `${{todos.length}} todos (update #${{updateCount}})`);
}});

// ── Nested query subscription ──
db.subscribeQuery({{ projects: {{}}, todos: {{}} }}, (r) => {{
  if (r.error) return;
  const projects = r.data?.projects || [];
  const todos = r.data?.todos || [];
  // Group todos by projectId
  const grouped = projects.map(p => ({{
    ...p,
    todos: todos.filter(t => t.projectId === p.id).sort((a,b) => (a.ts||0)-(b.ts||0))
  }}));
  document.getElementById('nested-out').textContent = JSON.stringify(grouped, null, 2);
}});

// ── Actions ──
window.addTodo = function() {{
  const input = document.getElementById('new-todo');
  const text = input.value.trim();
  if (!text) return;
  db.transact(tx.todos[id()].update({{ text, done: false, ts: Date.now() }}));
  input.value = '';
  log('ADD', text);
}};

window._toggle = function(todoId, done) {{
  db.transact(tx.todos[todoId].update({{ done }}));
  log('TOGGLE', done ? 'done' : 'undone');
}};

window._del = function(todoId) {{
  db.transact(tx.todos[todoId].delete());
  log('DELETE', todoId.slice(0,8));
}};
</script>
</body>
</html>"##
    )
}
