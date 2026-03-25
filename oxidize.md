# Oxidize: SQLiteData + Swift Sharing → Rust (sharing-instant)

**Upstream:** [sqlite-data](https://github.com/pointfreeco/sqlite-data) v1.6 + [swift-sharing](https://github.com/pointfreeco/swift-sharing) v2.7
**Port Version:** 0.1.0-oxidize.0 (audit — first oxidize-managed revision)
**Started:** 2026-03-22
**Audited:** 2026-03-24
**Last Updated:** 2026-03-24
**Current Phase:** Phase 0 (Audit) → entering Phase 3 next (test gaps + capability matrix)
**Ralph Loop Iteration:** 0

## Phase Checklist

### Phase 0: Audit (retroactive)
- [x] Workspace structure analyzed: 6 members (4 crates + 2 examples), 83 .rs files
- [x] Test inventory: 414 total tests, 346 passing, 2 failing (live_sync WebSocket), 66 ignored
- [x] Documentation audit: ~54% ASCII art headers on source files, 43 doc tests passing
- [x] Upstream comparison: core reactive persistence ported, CloudKit replaced by InstantDB
- [x] Capability matrix built (see below)
- [x] Existing feedback reviewed: none (no judgment/ or docs/feedback/ directories)
- [x] Platform bindings: none (pure Rust library)
- [x] Examples: 2 examples (live_sync, counter_sync), both compile
- [x] SRS/Artifacts: artifacts/v0.1.0/SRS.md exists and is comprehensive
- [x] Trinity: skeleton only — compiles but non-functional (stubs only)
- [x] Git history: 0 commits (all files staged but not committed)
- [ ] **JUDGMENT PANEL** — not yet run

### Phase 1: Reconnaissance
- [x] Found upstream repositories (cloned as submodules)
- [x] Analyzed structure (33 source files, 6 submodules)
- [x] Identified test files (44 integration + 43 doc tests)
- [x] Produced structured summary (via audit)
- [x] Capability matrix built (see Capability Summary below)
- [ ] **JUDGMENT PANEL** — not yet run

### Phase 2: Initialize Rust Workspace
- [x] Workspace Cargo.toml with 6 member crates
- [x] Standard deps configured (tokio, serde, parking_lot, dashmap, thiserror, insta)
- [x] .gitignore present
- [x] CLAUDE.md present and comprehensive
- [x] progress.md present
- [x] tasks.md present
- [x] Trinity binary crate created (skeleton)
- [x] 2 runnable examples (live_sync, counter_sync)
- [ ] **JUDGMENT PANEL** — not yet run

### Phase 3: Port Tests First
- [x] 44/44 integration test files exist
- [x] 346 tests passing, 2 failing (WebSocket integration), 66 ignored
- [ ] Cross-reference tests against capability matrix — some capabilities untested
- [ ] Missing test coverage for new callback/OperationState APIs (just added)
- [ ] **JUDGMENT PANEL** — not yet run

### Phase 4: Generate Versioned SRS
- [x] artifacts/v0.1.0/SRS.md exists (functional requirements matrix)
- [ ] Formal verification matrix (requirement → test mapping) — NOT PRESENT
- [ ] Architecture document with module diagrams — NOT PRESENT
- [ ] **JUDGMENT PANEL** — not yet run

### Phase 5: File Headers
- [~] 18/33 source files have ASCII art headers (54%)
- [ ] All 33 source files need headers
- [ ] 44 test files have //! module docs (good)
- [ ] **JUDGMENT PANEL** — not yet run

### Phase 6: CLAUDE.md & Trinity Build
- [x] CLAUDE.md exists and is comprehensive
- [ ] Trinity crate — skeleton only, non-functional
- [ ] Trinity init — NOT run (no .trinity/ directory)
- [ ] Pre-commit hooks — NOT installed
- [ ] **JUDGMENT PANEL** — not yet run

### Phase 7: Teaching Curriculum
- [ ] docs/curriculum/ directory exists but is EMPTY
- [ ] No lessons, no exercises
- [ ] **JUDGMENT PANEL** — not yet run

### Phase 8: Neo4j Knowledge Ingestion
- [ ] Not started
- [ ] **JUDGMENT PANEL** — not yet run

## Capability Summary

### Fully Ported (✅)
- `Shared<V, K>` mutable wrapper with auto-persistence
- `SharedReader<V>` read-only view with `map()` derived state
- `SharedKey` / `SharedReaderKey` traits (persistence abstraction)
- `FetchAll<T>` / `FetchOne<T>` / `Fetch<R>` reactive observers
- `FetchKeyRequest` trait for custom queries
- `SharedSubscription` RAII cancel handle
- `Database` trait + `InMemoryDatabase` + `DefaultDatabase`
- `Table` trait + `ColumnDef` + `QueryBuilder` (fluent API)
- `InMemoryKey` / `FileStorageKey` / `InstantDbKey` persistence strategies
- `SharingInstantError` unified error type
- `ConnectionState` enum
- `SyncEngine` + `SyncConfig` + `SyncStatus`

### New in Rust (not in Swift)
- `AuthCoordinator` + `AuthState` + `AuthUser` (InstantDB auth)
- `Room<P>` + `PresenceData` + `PresenceState` (real-time collaboration)
- `TopicChannel<T>` + `TopicEvent<T>` + `PublishHandle` (pub/sub)
- `Mutator<T>` typed CRUD with transaction building
- `MutationCallbacks<T>` (TanStack-style lifecycle hooks)
- `OperationState<T>` typed state machine for async ops
- `InstantDB` top-level entry point

### Intentionally Omitted (🚫 N/A)
- CloudKit sync infrastructure (replaced by InstantDB WebSocket)
- SwiftUI Binding/State integration (Rust is framework-agnostic)
- UserDefaults/AppStorageKey (Apple-specific)
- Combine Publisher (replaced by tokio watch channels)

### Planned but Not Yet Implemented (⚠️)
- `#[derive(Table)]` full proc macro (generates column metadata)
- WHERE filtering in InMemoryDatabase queries
- File watching for FileStorageKey (via `notify` crate)
- Reference deduplication (`PersistentReferences` global cache)
- Async load/save for Shared and FetchAll
- Auto-reconnection with exponential backoff
- Offline-first caching policy
- Pagination/cursor support

## Gaps Identified by Audit

1. **Trinity non-functional** — skeleton only, no pre-commit enforcement
2. **15/33 source files missing ASCII art headers** (46% uncovered)
3. **No formal verification matrix** (requirement → test mapping)
4. **No teaching curriculum** (docs/curriculum/ empty)
5. **No judgment directory** — no prior model reviews
6. **2 failing tests** in live_sync WebSocket integration
7. **18 clippy warnings** (minor: derivable_impls, unused imports, redundant closures)
8. **No git commits** — all files staged but never committed
9. **Proc macro incomplete** — generates table name but not column metadata
10. **Callback/OperationState APIs** just added (2026-03-24) — need capability matrix update

## Recommended Entry Point

**Enter Ralph loop at Phase 3** — tests are the most incomplete phase relative to the capability matrix. Core code is solid, but cross-cutting behaviors (offline mode, retry, batch operations) lack test coverage. After Phase 3, proceed to Phase 5 (headers), Phase 4 (verification matrix), Phase 6 (Trinity), Phase 7 (curriculum).

Trigger Judgment Panel on existing work before proceeding.

## Metrics
- Tests total: 414 (346 passing, 2 failing, 66 ignored)
- Source files: 33
- Test files: 45 (44 integration + 1 example)
- Doc tests: 43 passing, 13 ignored
- Modules ported: 11 core + 5 InstantDB-specific additions
- Doc coverage: ~54% ASCII headers, ~82% /// doc comments
- Clippy warnings: 18 (all minor)
- Judgment rounds completed: 0 (pre-oxidize work)
- Oxidize version: 0.1.0-oxidize.0
