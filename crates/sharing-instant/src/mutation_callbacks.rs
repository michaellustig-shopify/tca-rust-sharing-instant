use crate::error::SharingInstantError;

/// TanStack Mutation-style callbacks for async operations.
///
/// Builder pattern for attaching lifecycle hooks to mutations, publishes,
/// and other async operations. Callbacks are consumed on execution (FnOnce).
///
/// # Example
///
/// ```
/// use sharing_instant::MutationCallbacks;
///
/// let callbacks = MutationCallbacks::<String>::new()
///     .on_mutate(|| println!("Starting mutation..."))
///     .on_success(|val| println!("Success: {val}"))
///     .on_error(|err| eprintln!("Error: {err}"))
///     .on_settled(|| println!("Done."));
/// ```
pub struct MutationCallbacks<T: Send + 'static> {
    /// Called immediately before the operation starts.
    pub on_mutate: Option<Box<dyn FnOnce() + Send + Sync>>,
    /// Called when the operation succeeds, with the result value.
    pub on_success: Option<Box<dyn FnOnce(T) + Send + Sync>>,
    /// Called when the operation fails, with the error.
    pub on_error: Option<Box<dyn FnOnce(SharingInstantError) + Send + Sync>>,
    /// Called after the operation completes (success or failure).
    pub on_settled: Option<Box<dyn FnOnce() + Send + Sync>>,
}

impl<T: Send + 'static> MutationCallbacks<T> {
    /// Create empty callbacks (no-ops).
    pub fn new() -> Self {
        Self {
            on_mutate: None,
            on_success: None,
            on_error: None,
            on_settled: None,
        }
    }

    /// Shorthand: callbacks with only an on_error handler.
    pub fn error_only(f: impl FnOnce(SharingInstantError) + Send + Sync + 'static) -> Self {
        Self::new().on_error(f)
    }

    /// Shorthand: callbacks with only an on_success handler.
    pub fn success_only(f: impl FnOnce(T) + Send + Sync + 'static) -> Self {
        Self::new().on_success(f)
    }

    /// Shorthand: callbacks with only an on_settled handler.
    pub fn settled_only(f: impl FnOnce() + Send + Sync + 'static) -> Self {
        Self::new().on_settled(f)
    }

    /// Set the on_mutate callback (called before the operation).
    pub fn on_mutate(mut self, f: impl FnOnce() + Send + Sync + 'static) -> Self {
        self.on_mutate = Some(Box::new(f));
        self
    }

    /// Set the on_success callback (called with the result).
    pub fn on_success(mut self, f: impl FnOnce(T) + Send + Sync + 'static) -> Self {
        self.on_success = Some(Box::new(f));
        self
    }

    /// Set the on_error callback (called with the error).
    pub fn on_error(mut self, f: impl FnOnce(SharingInstantError) + Send + Sync + 'static) -> Self {
        self.on_error = Some(Box::new(f));
        self
    }

    /// Set the on_settled callback (called after success or failure).
    pub fn on_settled(mut self, f: impl FnOnce() + Send + Sync + 'static) -> Self {
        self.on_settled = Some(Box::new(f));
        self
    }

    /// Execute the success path: on_mutate → operation → on_success → on_settled.
    pub fn fire_success(self, value: T) {
        if let Some(f) = self.on_mutate {
            f();
        }
        if let Some(f) = self.on_success {
            f(value);
        }
        if let Some(f) = self.on_settled {
            f();
        }
    }

    /// Execute the error path: on_mutate → operation → on_error → on_settled.
    pub fn fire_error(self, error: SharingInstantError) {
        if let Some(f) = self.on_mutate {
            f();
        }
        if let Some(f) = self.on_error {
            f(error);
        }
        if let Some(f) = self.on_settled {
            f();
        }
    }

    /// Returns true if no callbacks are set.
    pub fn is_empty(&self) -> bool {
        self.on_mutate.is_none()
            && self.on_success.is_none()
            && self.on_error.is_none()
            && self.on_settled.is_none()
    }
}

impl<T: Send + 'static> Default for MutationCallbacks<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Send + 'static> std::fmt::Debug for MutationCallbacks<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MutationCallbacks")
            .field("on_mutate", &self.on_mutate.is_some())
            .field("on_success", &self.on_success.is_some())
            .field("on_error", &self.on_error.is_some())
            .field("on_settled", &self.on_settled.is_some())
            .finish()
    }
}
