//! Tests for ConnectionState enum.

use sharing_instant::ConnectionState;

#[test]
fn default_is_disconnected() {
    let state = ConnectionState::default();
    assert!(matches!(state, ConnectionState::Disconnected));
    assert!(!state.is_connected());
    assert!(!state.is_authenticated());
}

#[test]
fn connecting_is_not_connected() {
    let state = ConnectionState::Connecting;
    assert!(!state.is_connected());
    assert!(!state.is_authenticated());
}

#[test]
fn connected_is_connected_not_authenticated() {
    let state = ConnectionState::Connected;
    assert!(state.is_connected());
    assert!(!state.is_authenticated());
    assert!(state.session_id().is_none());
}

#[test]
fn authenticated_is_connected_and_has_session_id() {
    let state = ConnectionState::Authenticated {
        session_id: "sess-123".to_string(),
    };
    assert!(state.is_connected());
    assert!(state.is_authenticated());
    assert_eq!(state.session_id(), Some("sess-123"));
}

#[test]
fn error_state() {
    let state = ConnectionState::Error("timeout".to_string());
    assert!(!state.is_connected());
    assert!(state.is_error());
}

#[test]
fn display_formatting() {
    assert_eq!(ConnectionState::Disconnected.to_string(), "disconnected");
    assert_eq!(ConnectionState::Connecting.to_string(), "connecting");
    assert_eq!(ConnectionState::Connected.to_string(), "connected");
    assert_eq!(
        ConnectionState::Authenticated {
            session_id: "abc".to_string()
        }
        .to_string(),
        "authenticated (session: abc)"
    );
    assert_eq!(
        ConnectionState::Error("oops".to_string()).to_string(),
        "error: oops"
    );
}

#[test]
fn equality() {
    assert_eq!(ConnectionState::Disconnected, ConnectionState::Disconnected);
    assert_ne!(ConnectionState::Disconnected, ConnectionState::Connecting);
    assert_eq!(
        ConnectionState::Authenticated {
            session_id: "x".to_string()
        },
        ConnectionState::Authenticated {
            session_id: "x".to_string()
        }
    );
    assert_ne!(
        ConnectionState::Authenticated {
            session_id: "x".to_string()
        },
        ConnectionState::Authenticated {
            session_id: "y".to_string()
        }
    );
}
