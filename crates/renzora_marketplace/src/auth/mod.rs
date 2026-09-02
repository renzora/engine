//! The renzora.com account: session, sign-in UI, and the API client.
//!
//! Sign-in, register and forgot-password modals talking to the renzora.com API,
//! with tokens persisted to disk for auto-login.
//!
//! This was the `renzora_auth` crate, and it mirrored every feature the site
//! had. The `docs`, `feed`, `forum`, `messages`, `social` and `teams` modules
//! went with the social removal — what is left is exactly what an account needs
//! to buy and sell assets.

pub mod account;
pub mod api;
pub mod billing;
pub mod client;
pub mod marketplace;
pub mod publish;
mod modal;
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
            .init_resource::<SessionRefresh>()
            .add_systems(Update, (start_session_refresh, apply_session_refresh))
            .add_systems(Update, auth_system);
        // The sign-in modal.
        modal::register(app);
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


/// Restore a previously saved session from disk.
///
/// **Reads the file and nothing else.** Verifying the token with the server is
/// [`start_session_refresh`]'s job, on a background thread, because this runs
/// inside `Plugin::build` — before the app has a frame loop.
///
/// It used to refresh here, which worked only as long as the HTTP client was a
/// direct dependency and a blocking call could be made from anywhere. Now that
/// requests are handed to a plugin by a per-frame pump, a blocking call made
/// during `build` waits for a frame that cannot begin until `build` returns; the
/// refresh timed out, the `Err` was read as "token expired", and the session
/// file was deleted. The editor then asked for a sign-in on every launch.
#[cfg(not(target_arch = "wasm32"))]
pub fn try_restore_session() -> AuthSession {
    session::load_session().unwrap_or_default()
}

/// The in-flight startup token refresh.
#[derive(Resource, Default)]
pub struct SessionRefresh {
    /// `None` until the refresh is spawned, then holds its result channel.
    #[cfg(not(target_arch = "wasm32"))]
    rx: Option<Mutex<mpsc::Receiver<Result<api::AuthResponse, api::RefreshFailure>>>>,
    /// Set once a refresh has been started or abandoned, so neither happens
    /// twice.
    settled: bool,
    /// Frames spent waiting for a network backend to appear.
    waited: u32,
}

/// How many frames to wait for the HTTP plugin to register before giving up on
/// the startup refresh.
///
/// The plugin loader runs during the first frames, so the backend usually
/// arrives within a handful. Giving up merely skips the refresh — the saved
/// session is still used, and the next API call will discover it if the token
/// really has expired. A few seconds at 60 Hz.
#[cfg(not(target_arch = "wasm32"))]
const REFRESH_WAIT_FRAMES: u32 = 300;

/// Once a network backend exists, verify the restored token on a background
/// thread.
#[cfg(not(target_arch = "wasm32"))]
fn start_session_refresh(
    session: Res<AuthSession>,
    mut refresh: ResMut<SessionRefresh>,
) {
    if refresh.settled {
        return;
    }
    let Some(token) = session.refresh_token.clone() else {
        refresh.settled = true;
        return;
    };
    // Nothing to hand a request to yet. The HTTP plugin loads during the first
    // frames, so this is the ordinary case for a frame or two.
    if !renzora::net::is_available() {
        refresh.waited += 1;
        if refresh.waited > REFRESH_WAIT_FRAMES {
            warn!("[auth] no network backend — keeping the saved session unverified");
            refresh.settled = true;
        }
        return;
    }

    let (tx, rx) = mpsc::channel();
    std::thread::Builder::new()
        .name("renzora-auth-refresh".to_string())
        .spawn(move || {
            let _ = tx.send(api::refresh_token_checked(&token));
        })
        .ok();
    refresh.rx = Some(Mutex::new(rx));
    refresh.settled = true;
}

/// Apply the refresh once it lands.
#[cfg(not(target_arch = "wasm32"))]
fn apply_session_refresh(
    mut session: ResMut<AuthSession>,
    mut refresh: ResMut<SessionRefresh>,
) {
    let Some(rx) = &refresh.rx else { return };
    let Ok(rx) = rx.lock() else { return };
    let result = match rx.try_recv() {
        Ok(result) => result,
        Err(mpsc::TryRecvError::Empty) => return,
        // The worker died without answering. Leave the session alone — the same
        // reasoning as `Unavailable` below.
        Err(mpsc::TryRecvError::Disconnected) => {
            drop(rx);
            refresh.rx = None;
            return;
        }
    };
    drop(rx);
    refresh.rx = None;

    match result {
        Ok(response) => {
            session.set_from_response(&response);
            session::save_session(&session);
        }
        // The server refused the token. This is the ONLY case that signs the
        // user out.
        Err(api::RefreshFailure::Rejected(e)) => {
            info!("[auth] session expired ({e}) — signing out");
            session.clear();
            session::delete_session();
        }
        // Offline, or the API is having a bad day. Keep the session: the token
        // may well still be good, and the next call that needs it will find out.
        Err(api::RefreshFailure::Unavailable(e)) => {
            warn!("[auth] could not verify the saved session ({e}) — keeping it");
        }
    }
}

/// Both refresh systems are native-only; wasm has no session to restore.
#[cfg(target_arch = "wasm32")]
fn start_session_refresh() {}

#[cfg(target_arch = "wasm32")]
fn apply_session_refresh() {}

#[cfg(target_arch = "wasm32")]
pub fn try_restore_session() -> AuthSession {
    AuthSession::default()
}

// No `add!` here. This was `renzora_auth`'s own declaration, and leaving it
// would register `AuthPlugin` twice — once from the generated list and once from
// `MarketplacePlugin::build`, which adds it explicitly so the session exists
// before any panel that needs it. Bevy rejects the same plugin added twice.
