use std::time::Instant;

/// State machine for async operations (mutations, auth calls, publishes).
///
/// Replaces ad-hoc `is_loading: Arc<RwLock<bool>>` fields with a typed,
/// TanStack Query-style state enum. Used by topics, mutations, and auth.
///
/// # Example
///
/// ```
/// use sharing_instant::OperationState;
///
/// let state: OperationState<String> = OperationState::Idle;
/// assert!(!state.is_loading());
/// assert!(state.is_idle());
///
/// let state = OperationState::<String>::in_flight();
/// assert!(state.is_loading());
/// ```
#[derive(Debug, Clone)]
pub enum OperationState<T> {
    /// No operation has been started.
    Idle,
    /// An operation is currently running.
    InFlight { started_at: Instant },
    /// The operation completed successfully.
    Success { value: T, finished_at: Instant },
    /// The operation failed.
    Failure { error: String, finished_at: Instant },
}

impl<T> Default for OperationState<T> {
    fn default() -> Self {
        Self::Idle
    }
}

impl<T> OperationState<T> {
    /// Create an InFlight state with the current timestamp.
    pub fn in_flight() -> Self {
        Self::InFlight {
            started_at: Instant::now(),
        }
    }

    /// Create a Success state with the current timestamp.
    pub fn success(value: T) -> Self {
        Self::Success {
            value,
            finished_at: Instant::now(),
        }
    }

    /// Create a Failure state with the current timestamp.
    pub fn failure(error: impl Into<String>) -> Self {
        Self::Failure {
            error: error.into(),
            finished_at: Instant::now(),
        }
    }

    /// Whether an operation is currently in flight.
    pub fn is_loading(&self) -> bool {
        matches!(self, Self::InFlight { .. })
    }

    /// Whether the state is idle (no operation started).
    pub fn is_idle(&self) -> bool {
        matches!(self, Self::Idle)
    }

    /// Whether the last operation succeeded.
    pub fn is_success(&self) -> bool {
        matches!(self, Self::Success { .. })
    }

    /// Whether the last operation failed.
    pub fn is_failure(&self) -> bool {
        matches!(self, Self::Failure { .. })
    }

    /// Extract the success value, if present.
    pub fn value(&self) -> Option<&T> {
        match self {
            Self::Success { value, .. } => Some(value),
            _ => None,
        }
    }

    /// Extract the error message, if present.
    pub fn error(&self) -> Option<&str> {
        match self {
            Self::Failure { error, .. } => Some(error),
            _ => None,
        }
    }

    /// Map the success value to a different type.
    pub fn map<U>(self, f: impl FnOnce(T) -> U) -> OperationState<U> {
        match self {
            Self::Idle => OperationState::Idle,
            Self::InFlight { started_at } => OperationState::InFlight { started_at },
            Self::Success { value, finished_at } => OperationState::Success {
                value: f(value),
                finished_at,
            },
            Self::Failure { error, finished_at } => OperationState::Failure { error, finished_at },
        }
    }
}
