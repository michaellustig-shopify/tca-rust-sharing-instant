# rust-sharing-instant

Port of Point-Free's [SQLiteData](https://github.com/pointfreeco/sqlite-data) and [Swift Sharing](https://github.com/pointfreeco/swift-sharing) libraries to Rust, with InstantDB as the backing store instead of SQLite.

## What this is

A reactive data persistence library that provides:
- `Shared<V, K>` — Mutable shared state with automatic persistence
- `FetchAll<T>` / `FetchOne<T>` / `Fetch<R>` — Reactive database observations
- `SharedKey` / `SharedReaderKey` — Pluggable persistence strategies
- `SyncEngine` — Real-time sync via InstantDB's WebSocket reactor

## Architecture

| Crate | Purpose |
|-------|---------|
| `sharing-instant` | Core library (traits, types, implementations) |
| `sharing-instant-macros` | `#[derive(Table)]` proc macro |
| `sharing-instant-test` | Test utilities and schema definitions |
| `trinity` | Doc/test/code synchronization tool |

## Dependencies

- **InstantDB** (sibling workspace at `../rust-instantdb/`) — EAV triple store + WebSocket reactor
- **tokio** — Async runtime for subscriptions
- **parking_lot** — Fast read-write locks
- **dashmap** — Concurrent hash map for in-memory key storage

## Build & Test

```bash
cargo build          # Build all crates
cargo test           # Run all tests (~110 tests)
cargo test -p sharing-instant --test table_tests  # Run specific test file
cargo doc --open     # Generate and view documentation
```

## Reference Submodules

- `references/sqlite-data/` — Swift source (upstream)
- `references/swift-sharing/` — Swift sharing source (upstream)

## Key Concepts

### Table (like Swift's @Table)
```rust
impl Table for Reminder {
    const TABLE_NAME: &'static str = "reminders";
    fn columns() -> &'static [ColumnDef] { ... }
}
```

### FetchAll (like Swift's @FetchAll)
```rust
let fetch = FetchAll::<Reminder>::new(db);
let items = fetch.get(); // Vec<Reminder>
let rx = fetch.watch();  // watch::Receiver<Vec<Reminder>>
```

### Shared (like Swift's @Shared)
```rust
let shared = Shared::new(default_value, InMemoryKey::new("key"));
shared.with_lock(|v| *v = new_value); // auto-persists
```

## Versioning

Tracks upstream Swift library versions in `artifacts/`.
Current: v0.1.0 (based on SQLiteData 1.6 + Sharing 2.7).

## Example Recipes

Interactive CLI demos in `examples/`. All share a single InstantDB app by default (credentials saved to `.instant-app.json`). Pass `--ephemeral` for an isolated app.

| Example | Run | API |
|---------|-----|-----|
| `avatar-stack` | `cargo run -p avatar-stack` | `Room<UserPresence>` |
| `todos` | `cargo run -p todos` | `subscribe` + `transact` |
| `reactions` | `cargo run -p reactions` | `TopicChannel<EmojiReaction>` |
| `typing-indicator` | `cargo run -p typing-indicator` | `Room<TypingPresence>` |
| `cursors` | `cargo run -p cursors` | `Room<CursorPresence>` + ASCII grid |
| `merge-tiles` | `cargo run -p merge-tiles` | `subscribe` + `transact` + grid |

### Rules for examples

- **Never clutter the UI with debug output.** No `eprintln!` for debug logs in example binaries. Use the logging crate (writes to InstantDB) or a file behind an env var flag.
- Always show the App ID and dashboard URL on startup.
- Single-keypress input where possible (raw terminal mode).

## Incident Log

Document every bug discovered during development: what broke, why, how it was fixed, and what tests prevent regression. This section is append-only — never remove entries.

### 1. Presence messages fail to deserialize (room-type missing)

**What broke:** Two terminals running `avatar-stack` couldn't see each other. The `[ws] unparsed msg` log showed `refresh-presence` messages arriving from the server but failing to parse as `ServerMessage`.

**Why:** The Clojure server never includes `room-type` in `refresh-presence`, `patch-presence`, or `server-broadcast` messages (confirmed in `session.clj:657-675` and `admin/routes.clj:736-742` — explicit comment: "we did not actually use it"). The JS client never reads it either. But our Rust `ServerMessage` enum declared `room_type: String` as required — serde rejected the field's absence.

**How fixed:** Changed `room_type` to `Option<String>` with `#[serde(default)]` on all three variants in `instant-client/src/protocol.rs`. Updated the reactor handlers to use `room_id` suffix matching (already the pattern for RefreshPresence, but ServerBroadcast was using the empty room_type in its key lookup).

**Tests:** 4 deserialization tests in `protocol.rs` covering the exact JSON shapes the server sends — with and without `room-type`. `two_peers_see_each_other_presence` integration test verifies end-to-end.

### 2. PatchPresence reads wrong field name (data vs edits)

**What broke:** Incremental presence updates were silently empty — the `data` field was always null.

**Why:** The server sends `{op: "patch-presence", room-id: ..., edits: [...]}` but our `PatchPresence` struct had a field named `data`. Serde deserialized the missing `data` as null while the actual edits in `edits` were ignored.

**How fixed:** Renamed the field from `data` to `edits` in `protocol.rs` and updated the reactor handler.

**Tests:** `deserialize_patch_presence_without_room_type` test uses the exact server payload shape with `edits` field.

### 3. Room presence envelope not unwrapped (data field extraction)

**What broke:** Even after fixing deserialization, peers showed as "PARSE FAILED" — the `UserPresence` struct couldn't deserialize from the raw presence envelope.

**Why:** The server sends `{session_id: {peer-id: ..., instance-id: ..., user: null, data: {actual_payload}}}`. The Room listener was trying to deserialize the entire envelope as `UserPresence` instead of extracting the nested `data` field. The JS client does this extraction in `_setPresencePeers()`.

**How fixed:** Room listener in `room.rs` now extracts `envelope.get("data")` before deserializing. Also filters self vs peers using `reactor.session_id()`.

**Tests:** `two_peers_see_each_other_presence` — two reactors on the same app verify bidirectional presence with correct field values.

### 4. TopicChannel doesn't join the room (broadcasts not routed)

**What broke:** The `reactions` example — publishing emojis between two terminals produced no events on the receiving side.

**Why:** The JS client's `subscribeTopic()` calls `this.joinRoom()` internally (line 2605 in `Reactor.js`). Our `TopicChannel::subscribe` only called `reactor.subscribe_topic()` which creates local state but doesn't send `join-room` to the server. The server only routes `server-broadcast` to sessions that have joined the room.

**How fixed:** Added `reactor.join_room()` call inside `TopicChannel::subscribe`, before `subscribe_topic`.

**Tests:** `two_peers_topic_channel_broadcast` — A publishes, B receives via `TopicChannel::watch`.

### 5. WebSocket transact rejects high-level ops (Validation failed for tx-steps)

**What broke:** `todos` and `merge_tiles` panicked with `ServerError("Validation failed for tx-steps")` when creating/updating entities.

**Why:** The WebSocket `transact` endpoint expects **low-level** steps (`add-triple`, `deep-merge-triple`, `retract-triple`, etc.) that the client produces by transforming high-level ops using the attrs catalog from `InitOk`. The **admin REST API** does this transform server-side, but the WebSocket endpoint does not. Our examples were sending raw high-level ops (`["update", "todos", id, {...}]`) through the WebSocket.

**How fixed:** Switched from `client.transact()` (raw steps) to `client.transact_chunks()` which calls `instaml::transform()` with the attrs catalog. Uses `instant_core::instatx::tx()` builder: `tx("todos", id).update(attrs)`.

**Gotcha:** `transact_chunks` requires the attrs catalog from `InitOk`. `InstantAsync::new` returns before `InitOk` arrives. Examples must subscribe first (which waits for the connection) or sleep before calling `transact_chunks`.

**Tests:** `transact_chunks_creates_and_queries` — creates, reads, and deletes via the WebSocket transact path.

### Process rule

When you hit a bug:
1. Add `eprintln!` logging gated behind `INSTANT_LOG=1` — never to stdout/stderr unconditionally
2. Find the root cause by comparing against the upstream JS client and Clojure server
3. Fix in the correct layer (instant-client-rs for protocol bugs, sharing-instant for API bugs)
4. Write a regression test that would have caught it
5. Document it here

## Trinity

Trinity enforces documentation/tests/code synchronization. Run:
```bash
cargo run --bin trinity -- init    # Initialize
cargo run --bin trinity -- check   # Pre-commit check
cargo run --bin trinity -- status  # Show sync state
```
