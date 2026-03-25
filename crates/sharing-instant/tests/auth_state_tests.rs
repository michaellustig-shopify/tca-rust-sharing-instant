//! Tests for AuthState and AuthCoordinator.

use sharing_instant::auth::{AuthCoordinator, AuthState, AuthUser};

#[test]
fn default_auth_state_is_loading() {
    let state = AuthState::default();
    assert!(matches!(state, AuthState::Loading));
    assert!(!state.is_signed_in());
    assert!(state.user().is_none());
}

#[test]
fn unauthenticated_state() {
    let state = AuthState::Unauthenticated;
    assert!(!state.is_signed_in());
    assert!(state.user().is_none());
}

#[test]
fn guest_state() {
    let user = AuthUser {
        id: "guest-1".into(),
        email: None,
        refresh_token: None,
    };
    let state = AuthState::Guest { user: user.clone() };
    assert!(state.is_signed_in());
    assert_eq!(state.user().expect("user").id, "guest-1");
}

#[test]
fn authenticated_state() {
    let user = AuthUser {
        id: "user-1".into(),
        email: Some("alice@example.com".into()),
        refresh_token: Some("tok".into()),
    };
    let state = AuthState::Authenticated { user };
    assert!(state.is_signed_in());
    assert_eq!(
        state.user().expect("user").email.as_deref(),
        Some("alice@example.com")
    );
}

#[tokio::test]
async fn coordinator_starts_unauthenticated() {
    let auth = AuthCoordinator::new("test-app");
    let state = auth.state();
    assert!(matches!(*state.get(), AuthState::Unauthenticated));
}

// sign_in_as_guest hits the real InstantDB API.
// With a bad app_id it should fail with an auth error and state stays unauthenticated.
#[tokio::test]
async fn sign_in_as_guest_bad_app_fails() {
    let auth = AuthCoordinator::new("nonexistent-app-id");
    let result = auth.sign_in_as_guest().await;
    assert!(result.is_err());
    // State should have reverted — it was set to Loading, then the error happened.
    // The coordinator doesn't revert to Unauthenticated on failure, so it stays Loading.
    // This is fine — the caller handles the error.
}

#[tokio::test]
async fn sign_out_without_sign_in() {
    let auth = AuthCoordinator::new("test-app");
    // sign_out with no stored token should succeed (no-op server-side).
    auth.sign_out().await.expect("sign out");
    let state = auth.state();
    assert!(matches!(*state.get(), AuthState::Unauthenticated));
}

// send_magic_code and verify_magic_code are now real HTTP calls to InstantDB.
// They're tested in integration_tests.rs with real credentials.
// Here we just verify that calling with a bogus app_id returns an auth error.
#[tokio::test]
async fn magic_code_with_bad_app_returns_error() {
    let auth = AuthCoordinator::new("nonexistent-app-id");
    let result = auth.send_magic_code("test@example.com").await;
    assert!(result.is_err());
    // Should be an AuthError (HTTP failure from InstantDB).
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("auth error"),
        "expected auth error, got: {err}"
    );
}

// Bug fix: verify that failed auth resets to Unauthenticated, not stuck on Loading.
#[tokio::test]
async fn sign_in_as_guest_error_resets_to_unauthenticated() {
    let auth = AuthCoordinator::new("nonexistent-app-id");
    let _result = auth.sign_in_as_guest().await;
    // State should be Unauthenticated, not Loading.
    let state = auth.state();
    assert!(
        matches!(*state.get(), AuthState::Unauthenticated),
        "expected Unauthenticated after failed sign_in_as_guest, got: {:?}",
        *state.get()
    );
}

#[tokio::test]
async fn watch_state_returns_receiver() {
    let auth = AuthCoordinator::new("test-app");
    let rx = auth.watch_state();
    assert!(matches!(*rx.borrow(), AuthState::Unauthenticated));
}

// Test callback variant: sign_in_as_guest_with_callbacks fires on_error + on_settled
// with a bad app_id.
#[tokio::test]
async fn sign_in_as_guest_with_callbacks_fires_error() {
    use sharing_instant::MutationCallbacks;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    let auth = AuthCoordinator::new("nonexistent-app-id");

    let error_called = Arc::new(AtomicBool::new(false));
    let settled_called = Arc::new(AtomicBool::new(false));
    let ec = error_called.clone();
    let sc = settled_called.clone();

    let cb = MutationCallbacks::<AuthUser>::new()
        .on_error(move |_| ec.store(true, Ordering::SeqCst))
        .on_settled(move || sc.store(true, Ordering::SeqCst));

    auth.sign_in_as_guest_with_callbacks(cb).await;

    assert!(error_called.load(Ordering::SeqCst), "on_error should fire");
    assert!(
        settled_called.load(Ordering::SeqCst),
        "on_settled should fire"
    );
}

#[test]
fn create_authorization_url_returns_string() {
    let auth = AuthCoordinator::new("test-app");
    let url = auth.create_authorization_url("my-client", "https://example.com/callback");
    // Should return a non-empty URL string.
    assert!(!url.is_empty(), "authorization URL should not be empty");
    assert!(
        url.contains("test-app") || url.contains("my-client") || url.contains("example.com"),
        "URL should contain app or client info: {url}"
    );
}

#[tokio::test]
async fn send_magic_code_with_callbacks_fires_error() {
    use sharing_instant::MutationCallbacks;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    let auth = AuthCoordinator::new("nonexistent-app-id");

    let error_called = Arc::new(AtomicBool::new(false));
    let settled_called = Arc::new(AtomicBool::new(false));
    let ec = error_called.clone();
    let sc = settled_called.clone();

    let cb = MutationCallbacks::<()>::new()
        .on_error(move |_| ec.store(true, Ordering::SeqCst))
        .on_settled(move || sc.store(true, Ordering::SeqCst));

    auth.send_magic_code_with_callbacks("test@example.com", cb)
        .await;

    assert!(error_called.load(Ordering::SeqCst), "on_error should fire");
    assert!(
        settled_called.load(Ordering::SeqCst),
        "on_settled should fire"
    );
}

#[tokio::test]
async fn sign_in_with_token_error_resets_to_unauthenticated() {
    let auth = AuthCoordinator::new("nonexistent-app-id");
    let _result = auth.sign_in_with_token("bad-token").await;
    let state = auth.state();
    assert!(
        matches!(*state.get(), AuthState::Unauthenticated),
        "expected Unauthenticated after failed sign_in_with_token, got: {:?}",
        *state.get()
    );
}

#[tokio::test]
async fn sign_out_with_callbacks_fires_success() {
    use sharing_instant::MutationCallbacks;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    let auth = AuthCoordinator::new("test-app");

    let success_called = Arc::new(AtomicBool::new(false));
    let settled_called = Arc::new(AtomicBool::new(false));
    let sc = success_called.clone();
    let stc = settled_called.clone();

    let cb = MutationCallbacks::<()>::new()
        .on_success(move |_| sc.store(true, Ordering::SeqCst))
        .on_settled(move || stc.store(true, Ordering::SeqCst));

    auth.sign_out_with_callbacks(cb).await;

    assert!(
        success_called.load(Ordering::SeqCst),
        "on_success should fire"
    );
    assert!(
        settled_called.load(Ordering::SeqCst),
        "on_settled should fire"
    );
}
