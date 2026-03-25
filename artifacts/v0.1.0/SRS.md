# Software Requirement Specification — sharing-instant v0.1.0

**Port of:** Point-Free's [SQLiteData](https://github.com/pointfreeco/sqlite-data) v1.6 + [Swift Sharing](https://github.com/pointfreeco/swift-sharing) v2.7
**Backing store:** [InstantDB](https://github.com/mlustig/rust-instantdb) (Rust port)
**Date:** 2026-03-22

---

## 1. Overview

`sharing-instant` is a Rust library that provides reactive data persistence powered by InstantDB. It ports the API patterns from Point-Free's SQLiteData and Swift Sharing libraries, replacing SQLite with InstantDB's EAV (Entity-Attribute-Value) triple store and replacing iCloud sync with InstantDB's built-in WebSocket-based real-time synchronization.

## 2. Functional Requirements

### 2.1 Table Definition (FR-TABLE)

| ID | Requirement | Swift Equivalent | Status |
|----|-------------|------------------|--------|
| FR-TABLE-01 | Structs can be annotated with `#[derive(Table)]` to generate entity metadata | `@Table` macro | Partial |
| FR-TABLE-02 | Table name is auto-derived as pluralized snake_case | `@Table` naming | Done |
| FR-TABLE-03 | Columns have type metadata (string, number, boolean, etc.) | `@Column` | Done |
| FR-TABLE-04 | Optional fields map to nullable attributes | `Optional<T>` columns | Done |
| FR-TABLE-05 | Primary key field identified | `@Column(primaryKey:)` | Done |
| FR-TABLE-06 | Unique and indexed constraints expressible | `@Column(unique:)` | Done |

### 2.2 Reactive Observation (FR-FETCH)

| ID | Requirement | Swift Equivalent | Status |
|----|-------------|------------------|--------|
| FR-FETCH-01 | `FetchAll<T>` observes all rows of type T | `@FetchAll` | Done |
| FR-FETCH-02 | `FetchOne<T>` observes a single row | `@FetchOne` | Done |
| FR-FETCH-03 | `Fetch<R>` supports custom multi-query requests | `@Fetch` | Done |
| FR-FETCH-04 | All fetch types provide `is_loading` state | `isLoading` | Done |
| FR-FETCH-05 | All fetch types provide `load_error` | `loadError` | Done |
| FR-FETCH-06 | All fetch types produce `SharedReader` views | `sharedReader` | Done |
| FR-FETCH-07 | All fetch types provide `watch()` reactive streams | Observation | Done |
| FR-FETCH-08 | Subscriptions auto-update when data changes | ValueObservation | Done |

### 2.3 Shared State (FR-SHARED)

| ID | Requirement | Swift Equivalent | Status |
|----|-------------|------------------|--------|
| FR-SHARED-01 | `Shared<V, K>` wraps a value with automatic persistence | `@Shared` | Done |
| FR-SHARED-02 | `SharedReader<V>` provides read-only access | `SharedReader` | Done |
| FR-SHARED-03 | `with_lock()` provides exclusive mutable access | `.wrappedValue` set | Done |
| FR-SHARED-04 | Mutations auto-persist via `SharedKey::save()` | Auto-persist | Done |
| FR-SHARED-05 | External changes propagate via subscription | External changes | Done |
| FR-SHARED-06 | Clone shares the same underlying reference | Reference sharing | Done |
| FR-SHARED-07 | `SharedReader::map()` creates derived state | `dynamicMemberLookup` | Done |

### 2.4 Persistence Keys (FR-KEY)

| ID | Requirement | Swift Equivalent | Status |
|----|-------------|------------------|--------|
| FR-KEY-01 | `SharedReaderKey` trait for read-only persistence | `SharedReaderKey` protocol | Done |
| FR-KEY-02 | `SharedKey` trait extends with write capability | `SharedKey` protocol | Done |
| FR-KEY-03 | `InMemoryKey` for in-process sharing | `InMemoryKey` | Done |
| FR-KEY-04 | `InstantDbKey` for InstantDB persistence | N/A (new) | Done |
| FR-KEY-05 | `FileStorageKey` for local file persistence | `FileStorageKey` | Done |
| FR-KEY-06 | Key identity enables reference deduplication | `ID` associated type | Done |

### 2.5 Database (FR-DB)

| ID | Requirement | Swift Equivalent | Status |
|----|-------------|------------------|--------|
| FR-DB-01 | `Database` trait abstracts read/write/subscribe | `DatabaseReader/Writer` | Done |
| FR-DB-02 | `InMemoryDatabase` for testing | In-memory GRDB | Done |
| FR-DB-03 | `DefaultDatabase` global holder | `@Dependency(\.defaultDatabase)` | Done |
| FR-DB-04 | Query returns InstantDB `Value` | Query result | Done |
| FR-DB-05 | Transact accepts create/update/delete steps | Transaction | Done |
| FR-DB-06 | Subscribe returns watch channel for reactive updates | ValueObservation | Done |

### 2.6 Query Builder (FR-QUERY)

| ID | Requirement | Swift Equivalent | Status |
|----|-------------|------------------|--------|
| FR-QUERY-01 | `QueryBuilder<T>` with fluent API | Structured queries DSL | Done |
| FR-QUERY-02 | `where_eq`, `where_gt`, `where_lt`, etc. | `.where()` | Done |
| FR-QUERY-03 | `order()` with field and direction | `.order(by:)` | Done |
| FR-QUERY-04 | `limit()` and `offset()` | `.limit()` | Done |
| FR-QUERY-05 | `where_in()` for set membership | `$in` filter | Done |
| FR-QUERY-06 | `where_is_null()` for null checks | `$isNull` filter | Done |
| FR-QUERY-07 | Builds to InstantDB InstaQL `Value` format | InstaQL JSON | Done |

### 2.7 Sync Engine (FR-SYNC)

| ID | Requirement | Swift Equivalent | Status |
|----|-------------|------------------|--------|
| FR-SYNC-01 | `SyncEngine` manages WebSocket lifecycle | `SyncEngine` | Partial |
| FR-SYNC-02 | `SyncStatus` tracks connection state | Observable properties | Done |
| FR-SYNC-03 | Start/stop/status change notifications | `start()/stop()` | Done |
| FR-SYNC-04 | Auto-reconnection with backoff | CKSyncEngine reconnect | TODO |
| FR-SYNC-05 | Conflict resolution | CloudKit merge | TODO (InstantDB handles server-side) |

## 3. Architecture

### 3.1 Crate Structure

```
sharing-instant (workspace)
├── sharing-instant        — Core library
├── sharing-instant-macros — #[derive(Table)] proc macro
├── sharing-instant-test   — Test utilities
└── trinity                — Doc/test/code sync tool
```

### 3.2 Module Graph

```
lib.rs
├── table.rs              — Table trait, QueryBuilder, Value conversion
├── database.rs           — Database trait, InMemoryDatabase, DefaultDatabase
├── shared.rs             — Shared<V, K> mutable wrapper
├── shared_reader.rs      — SharedReader<V> read-only wrapper
├── shared_key.rs         — SharedKey trait (mutable persistence)
├── shared_reader_key.rs  — SharedReaderKey trait (read-only persistence)
├── subscription.rs       — SharedSubscription RAII handle
├── fetch_all.rs          — FetchAll<T> reactive collection
├── fetch_one.rs          — FetchOne<T> reactive single value
├── fetch.rs              — Fetch<R> custom request
├── fetch_key_request.rs  — FetchKeyRequest trait
├── error.rs              — SharingInstantError enum
├── keys/
│   ├── in_memory_key.rs  — InMemoryKey
│   ├── instant_db_key.rs — InstantDbKey
│   └── file_storage_key.rs — FileStorageKey
└── sync/
    └── engine.rs         — SyncEngine, SyncConfig, SyncStatus
```

### 3.3 Type Mapping (Swift → Rust)

| Swift | Rust | Notes |
|-------|------|-------|
| `@Table struct` | `#[derive(Table)] struct` | Proc macro |
| `@FetchAll var items: [T]` | `FetchAll::<T>::new(db)` | Struct, not property wrapper |
| `@FetchOne var item: T?` | `FetchOne::<T>::new(db)` | Returns `Option<T>` |
| `@Fetch(Request()) var resp` | `Fetch::new(request, db)` | Generic over `FetchKeyRequest` |
| `@Shared(.key) var x` | `Shared::new(default, key)` | Generic over `SharedKey` |
| `SharedReader<T>` | `SharedReader<T>` | 1:1 mapping |
| `SharedKey` protocol | `SharedKey` trait | Uses associated types |
| `SharedSubscription` | `SharedSubscription` | RAII via `Drop` |
| `DatabaseWriter` | `Database` trait | Combined read/write |
| `SyncEngine` | `SyncEngine` | InstantDB reactor wrapper |

## 4. Verification Matrix

| Requirement | Test File | Test Count |
|-------------|-----------|------------|
| FR-TABLE | table_tests.rs | 23 |
| FR-FETCH | fetch_all_tests.rs, fetch_one_tests.rs, fetch_tests.rs | 16 |
| FR-SHARED | shared_tests.rs | 9 |
| FR-KEY | in_memory_key_tests.rs | 6 |
| FR-DB | database_tests.rs | 10 |
| FR-SYNC | sync_engine_tests.rs | 5 |
| Errors | error_tests.rs | 6 |
| Subscription | subscription_tests.rs | 4 |
| Doc tests | (inline) | 31 |
| **Total** | | **110+** |

## 5. Version History

| Version | Date | Notes |
|---------|------|-------|
| v0.1.0 | 2026-03-22 | Initial port from Swift SQLiteData 1.6 + Sharing 2.7 |
