/// Authenticated user info from InstantDB.
#[derive(Debug, Clone, PartialEq)]
pub struct AuthUser {
    /// The user's unique ID.
    pub id: String,
    /// The user's email, if available.
    pub email: Option<String>,
    /// The refresh token for maintaining the session.
    pub refresh_token: Option<String>,
}

/// Authentication state machine.
///
/// Tracks the auth lifecycle: loading → unauthenticated → guest/authenticated.
///
/// # Example
///
/// ```
/// use sharing_instant::auth::AuthState;
///
/// let state = AuthState::default();
/// assert!(matches!(state, AuthState::Loading));
/// ```
#[derive(Debug, Clone, PartialEq)]
pub enum AuthState {
    /// Auth state is being determined (checking stored tokens, etc.).
    Loading,
    /// No user is signed in.
    Unauthenticated,
    /// Signed in as a guest (anonymous) user.
    Guest { user: AuthUser },
    /// Signed in as an authenticated user.
    Authenticated { user: AuthUser },
}

impl Default for AuthState {
    fn default() -> Self {
        Self::Loading
    }
}

impl AuthState {
    /// Whether any user is signed in (guest or authenticated).
    pub fn is_signed_in(&self) -> bool {
        matches!(self, Self::Guest { .. } | Self::Authenticated { .. })
    }

    /// Get the current user, if signed in.
    pub fn user(&self) -> Option<&AuthUser> {
        match self {
            Self::Guest { user } | Self::Authenticated { user } => Some(user),
            _ => None,
        }
    }
}
