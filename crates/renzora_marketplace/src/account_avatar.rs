//! The signed-in account's profile picture, published to [`AuthBridge`].
//!
//! The picture is not in the sign-in response. `POST /api/auth/login` returns a
//! `UserProfile` (id, username, email, role, credits) and nothing about how the
//! account looks, so the URL has to be asked for separately: `GET /api/user/me`
//! carries `avatar_url`, which is where the website's own nav gets it too.
//!
//! Fetched once per signed-in session, on a worker thread, and then left alone.
//! An avatar changes when the user changes it, which they do on the website —
//! polling for that would be a request a minute to notice something that
//! happens twice a year, and the next launch picks it up regardless.
//!
//! What lands on the bridge is the **loaded handle**, not the URL: see
//! [`AuthBridge::avatar`] for why the crates that draw it cannot be the ones
//! that fetch it.

use bevy::prelude::*;
use crossbeam_channel::{unbounded, Receiver};

use crate::auth::account;
use crate::auth::session::AuthSession;
use crate::avatars::{absolute_url, AvatarCache};

/// Where the fetch has got to for the current session.
#[derive(Resource, Default)]
pub(crate) struct AccountAvatar {
    /// The user id the fetch was made for. Signing in as somebody else changes
    /// it, which is what re-runs the fetch; without it a second account would
    /// wear the first one's picture for the rest of the session.
    fetched_for: Option<String>,
    /// The URL `get_me` reported, absolute. `None` once fetched means the
    /// account has no picture — a settled answer, not a pending one.
    url: Option<String>,
    rx: Option<Receiver<Option<String>>>,
}

pub(crate) fn register(app: &mut App) {
    app.init_resource::<AccountAvatar>();
    // Deliberately not gated on `SplashState::Editor` like the panels are. A
    // restored session is signed in from the moment the dashboard appears, and
    // the dashboard's own account row draws this picture — so the fetch, the
    // download and the handle all have to happen there, not on the way into a
    // project. (`avatars::poll_avatars` is registered for both states for the
    // same reason; see the plugin's `build`.)
    app.add_systems(Update, (start_fetch, poll_fetch, publish_handle).chain());
}

/// Ask the server for this session's `avatar_url`, once.
fn start_fetch(session: Res<AuthSession>, mut state: ResMut<AccountAvatar>) {
    let signed_in = session.user.as_ref().map(|u| u.id.clone());
    let Some(id) = signed_in else {
        // Signed out: forget the picture so the next account does not inherit
        // it, and so the bridge clears (`publish_handle` follows `url`).
        if state.fetched_for.is_some() {
            *state = AccountAvatar::default();
        }
        return;
    };
    if state.fetched_for.as_ref() == Some(&id) {
        return;
    }
    state.fetched_for = Some(id);
    state.url = None;

    let (tx, rx) = unbounded();
    state.rx = Some(rx);
    let session = crate::util::session_clone(&session);
    std::thread::spawn(move || {
        // A failed call is silence, not a toast: nobody asked for this, and an
        // account with no picture and an account the server would not tell us
        // about look the same on screen.
        let url = account::get_me(&session).ok().and_then(|me| me.avatar_url);
        let _ = tx.send(url);
    });
}

/// Take the fetched URL and start the download through the shared cache.
fn poll_fetch(mut state: ResMut<AccountAvatar>, mut cache: ResMut<AvatarCache>) {
    let Some(rx) = state.rx.as_ref() else { return };
    let Ok(url) = rx.try_recv() else { return };
    state.rx = None;
    let Some(url) = url.filter(|u| !u.trim().is_empty()) else { return };
    let url = absolute_url(&url);
    cache.request(&url);
    state.url = Some(url);
}

/// Mirror the loaded handle onto the bridge, so the shell and the splash can
/// draw it without knowing where it came from.
fn publish_handle(
    state: Res<AccountAvatar>,
    cache: Res<AvatarCache>,
    mut bridge: ResMut<renzora::core::AuthBridge>,
) {
    let want = state.url.as_ref().and_then(|u| cache.get(u));
    // Guarded: `AuthBridge` is read by reactive bindings, and writing an
    // unchanged handle every frame would mark it changed every frame.
    if bridge.avatar != want {
        bridge.avatar = want;
    }
}
