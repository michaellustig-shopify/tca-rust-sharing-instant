//! Tests for PresenceState<P> and PresenceData trait.

use sharing_instant::PresenceState;

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
struct Cursor {
    x: f64,
    y: f64,
}

#[test]
fn default_presence_state() {
    let state = PresenceState::<Cursor>::default();
    assert!(state.user.is_none());
    assert!(state.peers.is_empty());
    assert!(state.is_loading);
    assert!(state.error.is_none());
    assert_eq!(state.peer_count(), 0);
    assert!(!state.has_peers());
}

#[test]
fn presence_with_peers() {
    let mut state = PresenceState::<Cursor>::default();
    state
        .peers
        .insert("peer-1".to_string(), Cursor { x: 10.0, y: 20.0 });
    state
        .peers
        .insert("peer-2".to_string(), Cursor { x: 30.0, y: 40.0 });

    assert_eq!(state.peer_count(), 2);
    assert!(state.has_peers());
    assert_eq!(state.peer_ids().len(), 2);
}

#[test]
fn presence_with_user() {
    let mut state = PresenceState::<Cursor>::default();
    state.user = Some(Cursor { x: 0.0, y: 0.0 });

    assert!(state.user.is_some());
    assert_eq!(state.user.as_ref().expect("user").x, 0.0);
}

#[test]
fn serde_json_value_is_presence_data() {
    // serde_json::Value implements all bounds required for PresenceData.
    let state = PresenceState::<serde_json::Value>::default();
    assert!(state.peers.is_empty());
}

// OperationState for presence is tracked via watch channels.
// Room::set_presence drives the OperationState channel. Since we can't
// easily construct a Room without a reactor, we test the channel pattern directly.
#[test]
fn presence_operation_state_channel() {
    use sharing_instant::OperationState;
    use tokio::sync::watch;

    let (tx, rx) = watch::channel(OperationState::<()>::Idle);
    assert!(rx.borrow().is_idle());

    let _ = tx.send(OperationState::in_flight());
    assert!(rx.borrow().is_loading());

    let _ = tx.send(OperationState::success(()));
    assert!(rx.borrow().is_success());
}
