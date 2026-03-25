//! ┌─────────────────────────────────────────────────────┐
//! │  SYNC                                                │
//! │  Real-time synchronization via InstantDB              │
//! ├─────────────────────────────────────────────────────┤
//! │                                                      │
//! │  Unlike Swift's SyncEngine (which bridges SQLite     │
//! │  to CloudKit), our sync is FREE — InstantDB's        │
//! │  WebSocket reactor handles it natively.               │
//! │                                                      │
//! │  ┌────────┐  subscribe   ┌──────────┐  WebSocket    │
//! │  │ Client │ ───────────► │ Reactor  │ ──────────►   │
//! │  └────────┘              └──────────┘     Server     │
//! │       ▲                       │                      │
//! │       └───── watch update ────┘                      │
//! │                                                      │
//! ├─────────────────────────────────────────────────────┤
//! │  WHY: InstantDB was chosen specifically because it   │
//! │  provides real-time sync out of the box. Swift's     │
//! │  SQLiteData needs hundreds of lines of SyncEngine    │
//! │  code to bridge SQLite ↔ CloudKit. We get that       │
//! │  for free.                                           │
//! │                                                      │
//! │  TESTED BY: tests/sync_tests.rs                      │
//! │                                                      │
//! │  CHANGELOG:                                          │
//! │  • v0.1.0 — Initial sync module                      │
//! │                                                      │
//! │  HISTORY: git log --oneline --follow -- crates/sharing-instant/src/sync/ │
//! └─────────────────────────────────────────────────────┘

pub mod engine;
