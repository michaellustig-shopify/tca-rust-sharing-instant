use crate::auth::auth_state::{AuthState, AuthUser};
use crate::error::{Result, SharingInstantError};
use crate::mutation_callbacks::MutationCallbacks;
use crate::shared_reader::SharedReader;
use instant_client::auth;
use tokio::sync::watch;

/// Coordinates authentication flows with InstantDB.
///
/// Wraps `instant_client::Auth` (HTTP-based) and maintains reactive
/// `AuthState` that the rest of the app can observe.
///
/// # Example
///
/// ```
/// use sharing_instant::auth::AuthCoordinator;
///
/// let auth = AuthCoordinator::new("my-app-id");
/// let state = auth.state();
/// // Initially unauthenticated
/// ```
pub struct AuthCoordinator {
    inner: auth::Auth,
    state_sender: watch::Sender<AuthState>,
    state_receiver: watch::Receiver<AuthState>,
    /// Stored refresh token for sign_out (needs to invalidate server-side).
    refresh_token: parking_lot::RwLock<Option<String>>,
}

impl AuthCoordinator {
    /// Create a new auth coordinator for the given app ID.
    pub fn new(app_id: &str) -> Self {
        let inner = auth::Auth::new(app_id);
        let (state_sender, state_receiver) = watch::channel(AuthState::Unauthenticated);

        Self {
            inner,
            state_sender,
            state_receiver,
            refresh_token: parking_lot::RwLock::new(None),
        }
    }

    /// Create with a custom API URI (for self-hosted InstantDB).
    pub fn with_api_uri(app_id: &str, api_uri: &str) -> Self {
        let inner = auth::Auth::with_api_uri(app_id, api_uri);
        let (state_sender, state_receiver) = watch::channel(AuthState::Unauthenticated);

        Self {
            inner,
            state_sender,
            state_receiver,
            refresh_token: parking_lot::RwLock::new(None),
        }
    }

    /// Get a read-only view of the auth state.
    pub fn state(&self) -> SharedReader<AuthState> {
        let current = self.state_receiver.borrow().clone();
        SharedReader::from_watch(current, self.state_receiver.clone())
    }

    /// Sign in as a guest (anonymous) user.
    ///
    /// Hits InstantDB's `/runtime/auth/sign_in_as_guest` endpoint.
    pub async fn sign_in_as_guest(&self) -> Result<AuthUser> {
        let _ = self.state_sender.send(AuthState::Loading);

        let user = match self.inner.sign_in_as_guest().await {
            Ok(u) => u,
            Err(e) => {
                let _ = self.state_sender.send(AuthState::Unauthenticated);
                return Err(SharingInstantError::AuthError(e.to_string()));
            }
        };

        let auth_user = AuthUser {
            id: user.id,
            email: user.email,
            refresh_token: Some(user.refresh_token.clone()),
        };

        *self.refresh_token.write() = Some(user.refresh_token);
        let _ = self.state_sender.send(AuthState::Guest {
            user: auth_user.clone(),
        });
        Ok(auth_user)
    }

    /// Send a magic code to the given email.
    pub async fn send_magic_code(&self, email: &str) -> Result<()> {
        self.inner
            .send_magic_code(email)
            .await
            .map_err(|e| SharingInstantError::AuthError(e.to_string()))
    }

    /// Verify a magic code and sign in.
    pub async fn verify_magic_code(&self, email: &str, code: &str) -> Result<AuthUser> {
        let _ = self.state_sender.send(AuthState::Loading);

        let result = match self.inner.sign_in_with_magic_code(email, code).await {
            Ok(r) => r,
            Err(e) => {
                let _ = self.state_sender.send(AuthState::Unauthenticated);
                return Err(SharingInstantError::AuthError(e.to_string()));
            }
        };

        let auth_user = AuthUser {
            id: result.user.id,
            email: result.user.email,
            refresh_token: Some(result.user.refresh_token.clone()),
        };

        *self.refresh_token.write() = Some(result.user.refresh_token);
        let _ = self.state_sender.send(AuthState::Authenticated {
            user: auth_user.clone(),
        });
        Ok(auth_user)
    }

    /// Sign in with an existing refresh token.
    pub async fn sign_in_with_token(&self, refresh_token: &str) -> Result<AuthUser> {
        let _ = self.state_sender.send(AuthState::Loading);

        let user = match self.inner.sign_in_with_token(refresh_token).await {
            Ok(u) => u,
            Err(e) => {
                let _ = self.state_sender.send(AuthState::Unauthenticated);
                return Err(SharingInstantError::AuthError(e.to_string()));
            }
        };

        let auth_user = AuthUser {
            id: user.id,
            email: user.email,
            refresh_token: Some(user.refresh_token.clone()),
        };

        *self.refresh_token.write() = Some(user.refresh_token);
        let _ = self.state_sender.send(AuthState::Authenticated {
            user: auth_user.clone(),
        });
        Ok(auth_user)
    }

    /// Sign out the current user (invalidates server-side token).
    pub async fn sign_out(&self) -> Result<()> {
        let token = self.refresh_token.read().clone();
        if let Some(ref t) = token {
            // Best-effort server-side sign out. Don't fail if it errors —
            // we still want to clear local state.
            let _ = self.inner.sign_out(t).await;
        }
        *self.refresh_token.write() = None;
        let _ = self.state_sender.send(AuthState::Unauthenticated);
        Ok(())
    }

    /// Create an OAuth authorization URL.
    pub fn create_authorization_url(&self, client_name: &str, redirect_url: &str) -> String {
        self.inner
            .create_authorization_url(client_name, redirect_url)
    }

    /// Exchange an OAuth code for a token and sign in.
    pub async fn exchange_oauth_code(
        &self,
        code: &str,
        code_verifier: Option<&str>,
    ) -> Result<AuthUser> {
        let _ = self.state_sender.send(AuthState::Loading);

        let result = match self.inner.exchange_oauth_code(code, code_verifier).await {
            Ok(r) => r,
            Err(e) => {
                let _ = self.state_sender.send(AuthState::Unauthenticated);
                return Err(SharingInstantError::AuthError(e.to_string()));
            }
        };

        let auth_user = AuthUser {
            id: result.user.id,
            email: result.user.email,
            refresh_token: Some(result.user.refresh_token.clone()),
        };

        *self.refresh_token.write() = Some(result.user.refresh_token);
        let _ = self.state_sender.send(AuthState::Authenticated {
            user: auth_user.clone(),
        });
        Ok(auth_user)
    }

    // --- Raw channel accessor ---

    /// Get a raw watch receiver for auth state changes.
    pub fn watch_state(&self) -> watch::Receiver<AuthState> {
        self.state_receiver.clone()
    }

    // --- Callback variants ---

    /// Sign in as guest with mutation callbacks.
    pub async fn sign_in_as_guest_with_callbacks(&self, callbacks: MutationCallbacks<AuthUser>) {
        if let Some(f) = callbacks.on_mutate {
            f();
        }
        match self.sign_in_as_guest().await {
            Ok(user) => {
                if let Some(f) = callbacks.on_success {
                    f(user);
                }
            }
            Err(e) => {
                if let Some(f) = callbacks.on_error {
                    f(e);
                }
            }
        }
        if let Some(f) = callbacks.on_settled {
            f();
        }
    }

    /// Send magic code with mutation callbacks.
    pub async fn send_magic_code_with_callbacks(
        &self,
        email: &str,
        callbacks: MutationCallbacks<()>,
    ) {
        if let Some(f) = callbacks.on_mutate {
            f();
        }
        match self.send_magic_code(email).await {
            Ok(()) => {
                if let Some(f) = callbacks.on_success {
                    f(());
                }
            }
            Err(e) => {
                if let Some(f) = callbacks.on_error {
                    f(e);
                }
            }
        }
        if let Some(f) = callbacks.on_settled {
            f();
        }
    }

    /// Verify magic code with mutation callbacks.
    pub async fn verify_magic_code_with_callbacks(
        &self,
        email: &str,
        code: &str,
        callbacks: MutationCallbacks<AuthUser>,
    ) {
        if let Some(f) = callbacks.on_mutate {
            f();
        }
        match self.verify_magic_code(email, code).await {
            Ok(user) => {
                if let Some(f) = callbacks.on_success {
                    f(user);
                }
            }
            Err(e) => {
                if let Some(f) = callbacks.on_error {
                    f(e);
                }
            }
        }
        if let Some(f) = callbacks.on_settled {
            f();
        }
    }

    /// Sign in with token with mutation callbacks.
    pub async fn sign_in_with_token_with_callbacks(
        &self,
        refresh_token: &str,
        callbacks: MutationCallbacks<AuthUser>,
    ) {
        if let Some(f) = callbacks.on_mutate {
            f();
        }
        match self.sign_in_with_token(refresh_token).await {
            Ok(user) => {
                if let Some(f) = callbacks.on_success {
                    f(user);
                }
            }
            Err(e) => {
                if let Some(f) = callbacks.on_error {
                    f(e);
                }
            }
        }
        if let Some(f) = callbacks.on_settled {
            f();
        }
    }

    /// Sign out with mutation callbacks.
    pub async fn sign_out_with_callbacks(&self, callbacks: MutationCallbacks<()>) {
        if let Some(f) = callbacks.on_mutate {
            f();
        }
        match self.sign_out().await {
            Ok(()) => {
                if let Some(f) = callbacks.on_success {
                    f(());
                }
            }
            Err(e) => {
                if let Some(f) = callbacks.on_error {
                    f(e);
                }
            }
        }
        if let Some(f) = callbacks.on_settled {
            f();
        }
    }

    /// Exchange OAuth code with mutation callbacks.
    pub async fn exchange_oauth_code_with_callbacks(
        &self,
        code: &str,
        code_verifier: Option<&str>,
        callbacks: MutationCallbacks<AuthUser>,
    ) {
        if let Some(f) = callbacks.on_mutate {
            f();
        }
        match self.exchange_oauth_code(code, code_verifier).await {
            Ok(user) => {
                if let Some(f) = callbacks.on_success {
                    f(user);
                }
            }
            Err(e) => {
                if let Some(f) = callbacks.on_error {
                    f(e);
                }
            }
        }
        if let Some(f) = callbacks.on_settled {
            f();
        }
    }
}
