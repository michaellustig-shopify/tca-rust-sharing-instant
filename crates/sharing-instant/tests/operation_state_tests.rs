//! Tests for OperationState<T>.

use sharing_instant::OperationState;

#[test]
fn default_is_idle() {
    let state: OperationState<String> = OperationState::default();
    assert!(state.is_idle());
    assert!(!state.is_loading());
    assert!(!state.is_success());
    assert!(!state.is_failure());
    assert!(state.value().is_none());
    assert!(state.error().is_none());
}

#[test]
fn in_flight() {
    let state = OperationState::<i32>::in_flight();
    assert!(state.is_loading());
    assert!(!state.is_idle());
    assert!(state.value().is_none());
}

#[test]
fn success() {
    let state = OperationState::success(42);
    assert!(state.is_success());
    assert!(!state.is_loading());
    assert_eq!(state.value(), Some(&42));
    assert!(state.error().is_none());
}

#[test]
fn failure() {
    let state = OperationState::<String>::failure("connection timeout");
    assert!(state.is_failure());
    assert!(!state.is_success());
    assert!(state.value().is_none());
    assert_eq!(state.error(), Some("connection timeout"));
}

#[test]
fn map_success() {
    let state = OperationState::success(42);
    let mapped = state.map(|v| v.to_string());
    assert!(mapped.is_success());
    assert_eq!(mapped.value(), Some(&"42".to_string()));
}

#[test]
fn map_idle_stays_idle() {
    let state: OperationState<i32> = OperationState::Idle;
    let mapped = state.map(|v| v.to_string());
    assert!(mapped.is_idle());
}

#[test]
fn map_failure_preserves_error() {
    let state = OperationState::<i32>::failure("boom");
    let mapped = state.map(|v| v.to_string());
    assert!(mapped.is_failure());
    assert_eq!(mapped.error(), Some("boom"));
}

#[test]
fn map_in_flight_stays_in_flight() {
    let state = OperationState::<i32>::in_flight();
    let mapped = state.map(|v| v.to_string());
    assert!(mapped.is_loading());
}
