# rust-sharing-instant

Rust port of Point-Free's [swift-sharing](https://github.com/pointfreeco/swift-sharing) and [SQLiteData](https://github.com/pointfreeco/sqlite-data) libraries, with **InstantDB** as the real-time backing store instead of SQLite.

## What This Is

A reactive data persistence library for Rust applications:

- **`Shared<V, K>`** -- Mutable shared state with automatic persistence
- **`FetchAll<T>` / `FetchOne<T>` / `Fetch<R>`** -- Reactive database observations
- **`SharedKey` / `SharedReaderKey`** -- Pluggable persistence strategies
- **`SyncEngine`** -- Real-time sync via InstantDB's WebSocket reactor
- **`#[derive(Table)]`** -- Proc macro for defining database tables

## Quick Start

```bash
# Build all crates
cargo build

# Run all tests (~110 tests)
cargo test

# Run examples
cargo run --example live_sync
cargo run --example counter_sync
cargo run --example todos
cargo run --example merge_tiles_tui

# Generate docs
cargo doc --open
```

## Crate Structure

| Crate | Purpose |
|-------|---------|
| `sharing-instant` | Core library (traits, types, implementations) |
| `sharing-instant-macros` | `#[derive(Table)]` proc macro |
| `sharing-instant-test` | Test utilities and schema definitions |

## Examples

- `live_sync` -- Real-time sync demo
- `counter_sync` -- Synced counter across clients
- `auth_demo` -- Authentication flow
- `todos` -- Persistent todo list
- `merge_tiles_tui` -- Collaborative tile merging (TUI)
- `merge_tiles_gui` -- Same, with GUI
- `showcase` -- All examples in one launcher

## Usage with TCA

This crate is used as a sibling dependency by the [TCA Rust Port](https://github.com/michaellustig-shopify/tca-rust-port) for persistent shared state across features.

## Credits

- [Point-Free](https://www.pointfree.co/) -- original swift-sharing and SQLiteData
- [InstantDB](https://instantdb.com/) -- real-time database backend

## License

MIT
