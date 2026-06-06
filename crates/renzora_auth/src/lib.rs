//! Authentication UI and API client for Renzora Editor.
//!
//! Provides sign-in, register, and forgot-password modals that communicate
//! with the renzora.com API. Tokens are persisted to disk for auto-login.

pub mod api;
pub mod marketplace;
mod native;
pub mod session;

use bevy::prelude::*;

pub use session::AuthSession;

use std::sync::{mpsc, Mutex};

/// Bevy plugin that registers auth resources, renders the auth window, and
/// syncs state into the [`renzora::core::AuthBridge`] so the editor can display
/// sign-in info without depending on this crate.
#[derive(Default)]
pub struct AuthPlugin;

impl Plugin for AuthPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<AuthState>()
            .insert_resource(try_restore_session())
            .init_resource::<renzora::core::AuthBridge>()
            .add_systems(Update, auth_system);
        // Native (bevy_ui) sign-in modal.
        native::register(app);
    }
}

/// Single exclusive system that handles auth requests, renders the auth window,
/// and syncs the bridge resource.
fn auth_system(world: &mut World) {
    // Handle toggle/sign-out requests from the editor (via marker resources).
    if world
        .remove_resource::<renzora::core::AuthToggleWindowRequest>()
        .is_some()
    {
        if let Some(mut auth) = world.get_resource_mut::<AuthState>() {
            auth.window_open = !auth.window_open;
        }
    }
    if world
        .remove_resource::<renzora::core::AuthSignOutRequest>()
        .is_some()
    {
        if let Some(mut session) = world.get_resource_mut::<AuthSession>() {
            session.clear();
            #[cfg(not(target_arch = "wasm32"))]
            session::delete_session();
        }
        if let Some(mut auth) = world.get_resource_mut::<AuthState>() {
            auth.status = None;
            auth.error = None;
            auth.view = AuthView::SignIn;
        }
    }

    // Sync the lightweight bridge resource for the editor title bar.
    let window_open = world
        .get_resource::<AuthState>()
        .map(|a| a.window_open)
        .unwrap_or(false);
    let signed_in_username = world
        .get_resource::<AuthSession>()
        .and_then(|s| s.user.as_ref().map(|u| u.username.clone()));
    if let Some(mut bridge) = world.get_resource_mut::<renzora::core::AuthBridge>() {
        bridge.window_open = window_open;
        bridge.signed_in_username = signed_in_username;
    }
}

/// Current view within the auth window.
#[derive(Clone, Copy, PartialEq, Default)]
pub enum AuthView {
    #[default]
    SignIn,
    Register,
    ForgotPassword,
}

/// Result from a background auth API call.
enum AuthResult {
    Success(api::AuthResponse),
    ForgotSuccess(String),
    Error(String),
}

/// Persistent authentication UI state.
#[derive(Resource)]
#[derive(Default)]
pub struct AuthState {
    pub window_open: bool,
    pub view: AuthView,
    pub email: String,
    pub password: String,
    pub confirm_password: String,
    pub username: String,
    /// Status message shown in the UI.
    pub status: Option<String>,
    /// Error message shown in the UI.
    pub error: Option<String>,
    /// Whether an API call is in flight.
    pub loading: bool,
    /// Set to `true` when sign-in succeeds so the editor can react (e.g. switch layout).
    pub just_signed_in: bool,
    /// Channel receiver for background API results.
    receiver: Option<Mutex<mpsc::Receiver<AuthResult>>>,
}

/// Poll for results from background auth API calls.
fn poll_auth_result(state: &mut AuthState, session: &mut AuthSession) {
    let result = state
        .receiver
        .as_ref()
        .and_then(|rx| rx.lock().ok())
        .and_then(|rx| rx.try_recv().ok());

    if let Some(result) = result {
        state.loading = false;
        match result {
            AuthResult::Success(response) => {
                session.set_from_response(&response);
                #[cfg(not(target_arch = "wasm32"))]
                session::save_session(session);
                state.error = None;
                state.status = None;
                // Clear form fields and close window — editor will handle the transition
                state.password.clear();
                state.confirm_password.clear();
                state.window_open = false;
                state.just_signed_in = true;
            }
            AuthResult::ForgotSuccess(msg) => {
                state.error = None;
                state.status = Some(msg);
            }
            AuthResult::Error(err) => {
                state.status = None;
                state.error = Some(err);
            }
        }
    }
}

/// Spawn a background thread for an auth API call.
#[cfg(not(target_arch = "wasm32"))]
fn spawn_auth_request(state: &mut AuthState, f: impl FnOnce() -> AuthResult + Send + 'static) {
    state.loading = true;
    state.error = None;
    state.status = None;

    let (tx, rx) = mpsc::channel();
    state.receiver = Some(Mutex::new(rx));

    std::thread::spawn(move || {
        let result = f();
        let _ = tx.send(result);
    });
}

#[cfg(target_arch = "wasm32")]
fn spawn_auth_request(_state: &mut AuthState, _f: impl FnOnce() -> AuthResult + Send + 'static) {
    // No-op on WASM
}


/// Try to restore a previously saved session on startup.
/// Call this once during editor initialization.
#[cfg(not(target_arch = "wasm32"))]
pub fn try_restore_session() -> AuthSession {
    if let Some(mut saved) = session::load_session() {
        // Try to refresh the token in the background to verify it's still valid
        if let Some(refresh) = &saved.refresh_token {
            match api::refresh_token(refresh) {
                Ok(response) => {
                    saved.set_from_response(&response);
                    session::save_session(&saved);
                }
                Err(_) => {
                    // Token expired or invalid — clear session
                    saved.clear();
                    session::delete_session();
                }
            }
        }
        saved
    } else {
        AuthSession::default()
    }
}

#[cfg(target_arch = "wasm32")]
pub fn try_restore_session() -> AuthSession {
    AuthSession::default()
}

renzora::add!(AuthPlugin, Editor);
