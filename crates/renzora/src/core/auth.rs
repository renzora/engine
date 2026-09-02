//! Sign-in state, mirrored out of the marketplace plugin for the title bar.

use bevy::prelude::*;

/// Lightweight auth info resource that the auth plugin keeps in sync.
/// The editor reads this to display sign-in state in the title bar without
/// depending on the `marketplace` plugin that owns the session.
#[derive(Resource, Default, Clone)]
pub struct AuthBridge {
    /// Whether the auth sign-in window is currently open.
    pub window_open: bool,
    /// The signed-in username, if any.
    pub signed_in_username: Option<String>,
}

/// Marker resource inserted for one frame when sign-in succeeds.
/// The editor can consume this to react (e.g. switch to the Hub layout).
#[derive(Resource)]
pub struct AuthJustSignedIn;

/// Event-like resource: requests the auth window to toggle open/closed.
#[derive(Resource)]
pub struct AuthToggleWindowRequest;

/// Event-like resource: requests sign-out.
#[derive(Resource)]
pub struct AuthSignOutRequest;

// The social bridge lived here — `SocialWsState`, `SocialPanelRequest` and the
// `SocialBridge` resource carrying unread counts, the live-WebSocket state and
// the notification-dropdown request. All of it went with the social features:
// there is no feed, no messages, no friends and no notification bell to bridge
// to. `AuthBridge` above stays, because signing in is still how you publish and
// purchase — it is now the only account state the shell reads.
