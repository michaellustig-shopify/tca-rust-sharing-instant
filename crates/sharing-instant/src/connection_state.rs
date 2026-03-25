use std::fmt;

/// Connection lifecycle state for the SyncEngine.
///
/// Replaces the boolean `is_connected` field in SyncStatus with a richer
/// state machine. Transitions: Disconnected → Connecting → Connected →
/// Authenticated (when session_id arrives). `stop()` → Disconnected.
///
/// # Example
///
/// ```
/// use sharing_instant::ConnectionState;
///
/// let state = ConnectionState::default();
/// assert!(matches!(state, ConnectionState::Disconnected));
/// assert!(!state.is_connected());
/// ```
#[derive(Debug, Clone, PartialEq)]
pub enum ConnectionState {
    /// No active connection.
    Disconnected,
    /// WebSocket handshake in progress.
    Connecting,
    /// WebSocket connected, awaiting session ID.
    Connected,
    /// Fully connected and authenticated with a session ID.
    Authenticated { session_id: String },
    /// Connection failed with an error.
    Error(String),
}

impl Default for ConnectionState {
    fn default() -> Self {
        Self::Disconnected
    }
}

impl ConnectionState {
    /// Whether the connection is in a usable state (Connected or Authenticated).
    pub fn is_connected(&self) -> bool {
        matches!(
            self,
            ConnectionState::Connected | ConnectionState::Authenticated { .. }
        )
    }

    /// Whether the connection has been fully authenticated with a session ID.
    pub fn is_authenticated(&self) -> bool {
        matches!(self, ConnectionState::Authenticated { .. })
    }

    /// Extract the session ID if authenticated.
    pub fn session_id(&self) -> Option<&str> {
        match self {
            ConnectionState::Authenticated { session_id } => Some(session_id),
            _ => None,
        }
    }

    /// Whether the connection is in an error state.
    pub fn is_error(&self) -> bool {
        matches!(self, ConnectionState::Error(_))
    }
}

impl fmt::Display for ConnectionState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConnectionState::Disconnected => write!(f, "disconnected"),
            ConnectionState::Connecting => write!(f, "connecting"),
            ConnectionState::Connected => write!(f, "connected"),
            ConnectionState::Authenticated { session_id } => {
                write!(f, "authenticated (session: {session_id})")
            }
            ConnectionState::Error(msg) => write!(f, "error: {msg}"),
        }
    }
}
