# Tasks

## Current Phase: Phase 0 (Audit) → entering Phase 3

## Done
- [x] Initialize git repo and Swift submodules — 2026-03-22
- [x] Design architecture and type mapping — 2026-03-22
- [x] Initialize Rust workspace with member crates — 2026-03-22
- [x] Implement core types and traits — 2026-03-22
- [x] Port tests first (110+ tests passing) — 2026-03-22
- [x] Generate versioned SRS — 2026-03-22
- [x] Write CLAUDE.md, progress.md — 2026-03-22
- [x] Add ergonomic callbacks + OperationState across all domains — 2026-03-24
  - MutationCallbacks convenience constructors (error_only, success_only, settled_only)
  - link/unlink_with_callbacks on Mutator + FetchAll
  - PublishHandle + OperationState tracking in TopicChannel
  - Room presence_operation_state + set_presence_with_callbacks
  - AuthCoordinator: 6 _with_callbacks variants + watch_state + error-reset bug fix
  - InstantDB.watch_auth_state() + PublishHandle re-export
- [x] Oxidize Phase 0 audit — 2026-03-24

## In Progress
- [ ] Build Trinity crate (full implementation)

## Up Next (prioritized by oxidize audit)

### Phase 3: Test Gaps (highest priority)
- [ ] Cross-reference all tests against capability matrix
- [ ] Fix 2 failing live_sync WebSocket tests
- [ ] Add tests for cross-cutting behaviors (offline state, reconnection)
- [ ] Write #[ignore] tests for planned-but-unimplemented capabilities

### Phase 5: File Headers
- [ ] Add ASCII art headers to remaining 15/33 source files

### Phase 4: Verification Matrix
- [ ] Build formal requirement → test mapping in artifacts/
- [ ] Architecture document with module dependency diagrams

### Phase 6: Trinity
- [ ] Implement trinity init (scan codebase, install pre-commit hook)
- [ ] Implement trinity check (3 parallel agents: docs↔code, tests↔code, SRS↔code)
- [ ] Implement trinity status (show sync state)
- [ ] Create .trinity/ directory with state.json

### Code Improvements
- [ ] Implement `#[derive(Table)]` proc macro fully (generate columns from struct fields)
- [ ] Add `where` filtering to InMemoryDatabase queries
- [ ] Add file watching to FileStorageKey (via `notify` crate)
- [ ] Reference deduplication (PersistentReferences global cache)
- [ ] Async load/save for Shared and FetchAll
- [ ] Auto-reconnection with exponential backoff for SyncEngine
- [ ] Fix 18 clippy warnings (derivable_impls, unused imports, redundant closures)

### Phase 7: Curriculum
- [ ] Teaching curriculum in `docs/curriculum/`

### Phase 8: Neo4j
- [ ] Neo4j knowledge ingestion

## Discovered During Audit
- [ ] AuthCoordinator error paths now reset to Unauthenticated (bug fix applied 2026-03-24)
- [ ] No git commits exist — need initial commit before proceeding
- [ ] Proc macro generates table name but not column metadata (partial)
- [ ] Capability matrix needs formal artifacts/capability-matrix.md file
