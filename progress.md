# Progress

## Saturday, March 22nd 2026 — Initial Port

### Workspace Setup
- Initialized Rust workspace with 4 crates: `sharing-instant`, `sharing-instant-macros`, `sharing-instant-test`, `trinity`
- Added git submodules for Swift references: `sqlite-data`, `swift-sharing`
- Configured workspace dependencies linking to sibling `rust-instantdb`

### Core Types Implemented
- `Table` trait + `ColumnDef` + `QueryBuilder` with fluent API
- `Shared<V, K>` mutable wrapper with auto-persistence
- `SharedReader<V>` read-only view with `map()` derived state
- `SharedKey` / `SharedReaderKey` traits (persistence abstraction)
- `FetchAll<T>` reactive collection observer
- `FetchOne<T>` reactive single-value observer
- `Fetch<R>` custom multi-query observer
- `FetchKeyRequest` trait for custom queries
- `SharedSubscription` RAII cancel handle
- `Database` trait + `InMemoryDatabase` + `DefaultDatabase`
- `SyncEngine` + `SyncConfig` + `SyncStatus`
- `InMemoryKey`, `InstantDbKey`, `FileStorageKey` persistence strategies
- `SharingInstantError` unified error type

### Tests: 110+ passing
- `table_tests.rs` — 23 tests (Value conversion, serialization, query builder)
- `database_tests.rs` — 10 tests (CRUD, transactions, subscriptions)
- `shared_tests.rs` — 9 tests (mutation, persistence, cloning, reader)
- `fetch_all_tests.rs` — 6 tests (empty, with data, loading, reader)
- `fetch_one_tests.rs` — 6 tests (empty, require, loading, reader)
- `fetch_tests.rs` — 4 tests (custom request, data, loading)
- `in_memory_key_tests.rs` — 6 tests (load, save, identity, sharing)
- `sync_engine_tests.rs` — 5 tests (status, start, stop, notifications)
- `subscription_tests.rs` — 4 tests (cancel, drop, double cancel, empty)
- `error_tests.rs` — 6 tests (display, debug for all variants)
- Doc tests — 31 passing

### Artifacts
- `artifacts/v0.1.0/SRS.md` — Software Requirement Specification
- `CLAUDE.md` — Project documentation
- ASCII art file headers on all `.rs` files

## Monday, March 24th 2026 — Ergonomic Callbacks + OperationState

### Callback Coverage Across All Domains
- `MutationCallbacks` convenience constructors: `error_only()`, `success_only()`, `settled_only()`
- `Mutator<T>`: added `link_with_callbacks()`, `unlink_with_callbacks()`, extracted `with_callbacks_inner()` helper
- `FetchAll<T>`: added 5 `_with_callbacks` delegate methods (create, update, delete, link, unlink)
- `TopicChannel`: new `PublishHandle` type tracking `OperationState<()>`, both `publish()` and `publish_with_callbacks()` now return `Result<PublishHandle>`
- `Room<P>`: added `presence_op_sender/receiver` fields, `set_presence_with_callbacks()`, `presence_operation_state()`
- `AuthCoordinator`: **bug fix** — error paths now reset to `Unauthenticated` instead of staying stuck on `Loading`. Added `watch_state()` and 6 `_with_callbacks` variants
- `InstantDB`: added `watch_auth_state()`, re-exported `PublishHandle`

### New Tests: 18 added
- 3 convenience constructor tests (mutation_callbacks_tests.rs)
- 4 link/unlink + FetchAll callback tests (mutations_tests.rs)
- 6 PublishHandle/OperationState tests (publish_handle_tests.rs — new file)
- 1 presence OperationState channel test (presence_tests.rs)
- 4 auth tests: error reset, watch_state, callback variants (auth_state_tests.rs)

### Test Count: 414 total (346 passing, 2 failing, 66 ignored)

## Monday, March 24th 2026 — Oxidize Phase 0 Audit

### Audit Results
- Ran 4 parallel audit agents: workspace structure, test inventory, upstream comparison, Trinity/git/docs
- Generated `oxidize.md` master checklist with accurate phase state
- Updated `tasks.md` with prioritized gaps from audit
- Built capability matrix comparing Rust port vs Swift upstream

### Key Findings
- Core reactive persistence: fully ported
- CloudKit: intentionally replaced by InstantDB (architectural decision)
- 5 new Rust-only modules: Auth, Rooms/Presence, Topics, MutationCallbacks, OperationState
- Trinity: skeleton only (compiles, non-functional)
- 15/33 source files missing ASCII art headers
- No formal verification matrix or teaching curriculum
- No git commits yet (all staged)
- Recommended: enter Ralph loop at Phase 3 (test gap analysis)
