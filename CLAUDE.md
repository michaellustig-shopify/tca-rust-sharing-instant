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

## Trinity

Trinity enforces documentation/tests/code synchronization. Run:
```bash
cargo run --bin trinity -- init    # Initialize
cargo run --bin trinity -- check   # Pre-commit check
cargo run --bin trinity -- status  # Show sync state
```
