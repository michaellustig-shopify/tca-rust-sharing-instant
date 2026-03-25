//! Tests for error types.

use sharing_instant::error::SharingInstantError;

#[test]
fn not_found_error_display() {
    let err = SharingInstantError::NotFound {
        entity: "Reminder".to_string(),
        query: "id = abc123".to_string(),
    };
    let msg = err.to_string();
    assert!(msg.contains("Reminder"));
    assert!(msg.contains("abc123"));
}

#[test]
fn connection_failed_display() {
    let err = SharingInstantError::ConnectionFailed("timeout".to_string());
    assert!(err.to_string().contains("timeout"));
}

#[test]
fn query_failed_display() {
    let err = SharingInstantError::QueryFailed("bad syntax".to_string());
    assert!(err.to_string().contains("bad syntax"));
}

#[test]
fn transaction_failed_display() {
    let err = SharingInstantError::TransactionFailed("conflict".to_string());
    assert!(err.to_string().contains("conflict"));
}

#[test]
fn serialization_error_display() {
    let err = SharingInstantError::SerializationError("missing field".to_string());
    assert!(err.to_string().contains("missing field"));
}

#[test]
fn error_is_debug() {
    let err = SharingInstantError::KeyError("oops".to_string());
    let debug = format!("{err:?}");
    assert!(debug.contains("KeyError"));
}
