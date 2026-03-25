//! Tests for PublishHandle and its OperationState tracking.

use sharing_instant::OperationState;
use tokio::sync::watch;

// PublishHandle is constructed from a watch channel internally.
// We test the same pattern here by constructing one from a manual channel.

#[test]
fn operation_state_starts_in_flight() {
    let (_tx, rx) = watch::channel(OperationState::<()>::in_flight());
    assert!(rx.borrow().is_loading());
    assert!(!rx.borrow().is_success());
    assert!(!rx.borrow().is_failure());
}

#[test]
fn operation_state_transitions_to_success() {
    let (tx, rx) = watch::channel(OperationState::<()>::in_flight());
    assert!(rx.borrow().is_loading());

    let _ = tx.send(OperationState::success(()));
    assert!(rx.borrow().is_success());
    assert!(!rx.borrow().is_loading());
    assert!(rx.borrow().error().is_none());
}

#[test]
fn operation_state_transitions_to_failure() {
    let (tx, rx) = watch::channel(OperationState::<()>::in_flight());
    assert!(rx.borrow().is_loading());

    let _ = tx.send(OperationState::failure("network timeout"));
    assert!(rx.borrow().is_failure());
    assert!(!rx.borrow().is_loading());
    assert_eq!(rx.borrow().error(), Some("network timeout"));
}

#[test]
fn operation_state_error_returns_none_for_success() {
    let (_tx, rx) = watch::channel(OperationState::success(()));
    assert!(rx.borrow().error().is_none());
}

#[test]
fn operation_state_value_returns_some_for_success() {
    let (_tx, rx) = watch::channel(OperationState::success(42));
    assert_eq!(rx.borrow().value(), Some(&42));
}

#[tokio::test]
async fn watch_receiver_notifies_on_state_change() {
    let (tx, mut rx) = watch::channel(OperationState::<()>::in_flight());

    let handle = tokio::spawn(async move {
        rx.changed().await.expect("should receive change");
        rx.borrow().is_success()
    });

    let _ = tx.send(OperationState::success(()));
    let is_success = handle.await.expect("task should complete");
    assert!(is_success);
}
