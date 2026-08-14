//! Renzora Community — the social layer of the editor, backed by renzora.com.
//!
//! Panels: Friends (with a Teams tab), Chat, Feed, Forum, Learn. Profiles are a
//! shared overlay and notifications are the top-bar bell dropdown — neither is a
//! dockable panel. Live events arrive over the site's WebSocket; everything else
//! is blocking HTTP on worker threads (the engine-wide convention — no async
//! runtime).

// ── Web: the community layer is absent, not broken ───────────────────────────
// Every panel here is a view onto renzora.com via `renzora_auth`, and that crate
// is native-only because `renzora_net` is — the engine's HTTP convention is
// blocking calls on worker threads (see the module doc above), and a browser has
// neither blocking sockets nor threads to spare. The WebSocket half is equally
// stuck: `ws` is a native client, not a browser `WebSocket`.
//
// `SocialPlugin` itself still has to exist on wasm: `renzora_editor`'s plugin
// list is generated from the `add!` below and CI fails if a regeneration diffs,
// so the type must resolve on every target the editor builds for. Hollowing the
// plugin keeps that generated file byte-identical.
//
// This comes back with `renzora_net` over `fetch` plus the browser `WebSocket`
// API — one dependency, and the whole community layer returns with it.
#[cfg(not(target_arch = "wasm32"))]
mod account_settings;
#[cfg(not(target_arch = "wasm32"))]
mod avatars;
#[cfg(not(target_arch = "wasm32"))]
mod confetti;
#[cfg(not(target_arch = "wasm32"))]
mod lightbox;
#[cfg(not(target_arch = "wasm32"))]
mod notify_dropdown;
#[cfg(not(target_arch = "wasm32"))]
mod panels;
#[cfg(not(target_arch = "wasm32"))]
mod reaction_picker;
#[cfg(not(target_arch = "wasm32"))]
mod routing;
#[cfg(not(target_arch = "wasm32"))]
mod settings;
#[cfg(not(target_arch = "wasm32"))]
mod toasts;
#[cfg(not(target_arch = "wasm32"))]
mod util;
#[cfg(not(target_arch = "wasm32"))]
mod ws;

use bevy::prelude::*;
use renzora::core::{
    RenzoraShellExt, ShellStatusAlign, ShellStatusItem, ShellStatusSegment, SocialBridge,
    SocialPanelRequest, SocialWsState,
};
use renzora::SplashState;

/// Deep-link context handed to panels after a [`SocialPanelRequest`] focuses
/// them (e.g. which conversation or profile to show). Panels `take()` what
/// they understand.
#[derive(Resource, Default)]
pub(crate) struct PendingSocialRequest(pub Option<SocialPanelRequest>);

#[derive(Default)]
pub struct SocialPlugin;

impl Plugin for SocialPlugin {
    #[cfg(target_arch = "wasm32")]
    fn build(&self, _app: &mut App) {
        info!("[editor] SocialPlugin: community features unavailable on the web (no HTTP client)");
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn build(&self, app: &mut App) {
        info!("[editor] SocialPlugin");
        app.init_resource::<SocialBridge>();
        // Relative image/link paths in docs & forum markdown resolve against
        // the same server the client talks to.
        app.insert_resource(renzora_ember::widgets::MarkdownBaseUrl(
            renzora_auth::client::api_base().to_string(),
        ));
        // Restore the top-bar bell preference before the shell reads it.
        app.world_mut().resource_mut::<SocialBridge>().notify_button_enabled =
            settings::load_bell_pref();
        app
            .init_resource::<PendingSocialRequest>()
            .init_resource::<avatars::AvatarCache>()
            .init_resource::<toasts::ToastQueue>()
            .init_resource::<toasts::ToastUi>()
            .init_resource::<notify_dropdown::NotifyDropdownUi>()
            .init_resource::<reaction_picker::ReactionPicker>()
            .init_resource::<lightbox::Lightbox>()
            .init_resource::<confetti::Confetti>()
            .init_resource::<ws::WsConnection>();

        app.add_systems(
            Update,
            (
                avatars::poll_avatars,
                avatars::request_avatars,
                toasts::drain_toasts,
                toasts::toast_clicks,
                notify_dropdown::toggle,
                notify_dropdown::clicks,
                // Chained: the grid rebuild's queued commands must apply
                // before `picks` can despawn the picker (same-frame race).
                (
                    reaction_picker::open_clicks,
                    reaction_picker::search_filter,
                    reaction_picker::picks,
                )
                    .chain(),
                // Open after close so clicking a NEW image while a lightbox is
                // up swaps images instead of the backdrop-close eating the press.
                (lightbox::close_clicks, lightbox::open_clicks).chain(),
                ws::manage_ws_connection,
                ws::poll_ws_events,
                (confetti::spawn, confetti::animate),
                handle_panel_requests,
                sign_out_cleanup,
            )
                .run_if(in_state(SplashState::Editor)),
        );

        app.register_shell_status_item(ShellStatusItem {
            id: "social_status",
            align: ShellStatusAlign::Right,
            order: 50,
            render: social_status,
        });

        panels::register(app);
        settings::register(app);
        account_settings::register(app);
    }
}

/// When the session ends, clear all account-scoped panel state and counters.
/// (The WebSocket worker is stopped by `manage_ws_connection`.)
///
/// Native-only along with everything it touches — nothing registers it on wasm.
#[cfg(not(target_arch = "wasm32"))]
#[allow(clippy::too_many_arguments)]
fn sign_out_cleanup(
    session: Res<renzora_auth::AuthSession>,
    mut was_signed_in: Local<bool>,
    mut bridge: ResMut<SocialBridge>,
    mut friends: ResMut<panels::friends::FriendsPanel>,
    mut chat: ResMut<panels::chat::ChatPanel>,
    mut notifications: ResMut<panels::notifications::NotificationsPanel>,
    mut profile: ResMut<panels::profile::ProfilePanel>,
    mut feed: ResMut<panels::feed::FeedPanel>,
    mut onboarding: ResMut<panels::onboarding::OnboardingPanel>,
) {
    let signed_in = session.is_signed_in();
    if *was_signed_in && !signed_in {
        *friends = Default::default();
        *chat = Default::default();
        *notifications = Default::default();
        *profile = Default::default();
        *feed = Default::default();
        // Onboarding status is account-scoped — a new sign-in must re-fetch it.
        *onboarding = Default::default();
        bridge.unread_notifications = 0;
        bridge.unread_messages = 0;
        bridge.friends_online = 0;
    }
    *was_signed_in = signed_in;
}

renzora::add!(SocialPlugin, Editor);

/// Status bar: WS connection dot + unread counters + friends online.
#[cfg(not(target_arch = "wasm32"))]
fn social_status(world: &World) -> Vec<ShellStatusSegment> {
    let Some(bridge) = world.get_resource::<SocialBridge>() else {
        return Vec::new();
    };
    let mut segs = Vec::new();
    let (color, label): ([u8; 3], &str) = match bridge.ws_state {
        SocialWsState::Connected => ([82, 196, 120], "Online"),
        SocialWsState::Connecting => ([230, 180, 80], "Connecting"),
        SocialWsState::Disconnected => ([120, 120, 134], "Offline"),
    };
    segs.push(ShellStatusSegment::new("globe", label, color));
    if bridge.friends_online > 0 {
        segs.push(ShellStatusSegment::new(
            "users",
            format!("{}", bridge.friends_online),
            [120, 120, 134],
        ));
    }
    if bridge.unread_notifications > 0 {
        segs.push(ShellStatusSegment::new(
            "bell",
            format!("{}", bridge.unread_notifications),
            [230, 180, 80],
        ));
    }
    if bridge.unread_messages > 0 {
        segs.push(ShellStatusSegment::new(
            "chats",
            format!("{}", bridge.unread_messages),
            [82, 196, 120],
        ));
    }
    segs
}

/// Consume [`SocialBridge::open_panel_request`]: focus the target panel in the
/// dock and stash the request so the panel can apply its context.
///
/// Two requests have no panel and are handled before the dock: `Notifications`
/// pops the bell dropdown, and `Profile` is stashed for `profile::open_overlay`
/// to raise the shared profile modal (so a username click anywhere opens the
/// same overlay, not a stray tab).
#[cfg(not(target_arch = "wasm32"))]
fn handle_panel_requests(
    mut bridge: ResMut<SocialBridge>,
    mut pending: ResMut<PendingSocialRequest>,
    dock: Option<ResMut<renzora_ember::dock::Dock>>,
    windows: Option<Res<renzora_ember::dock::DockWindows>>,
) {
    let Some(req) = bridge.open_panel_request.take() else {
        return;
    };
    // Notifications no longer have a panel — the request pops the centered bell
    // dropdown (which ignores the x and self-centers; y anchors it under the bar).
    if matches!(req, SocialPanelRequest::Notifications) {
        bridge.notify_dropdown_request = Some((0.0, 46.0));
        return;
    }
    // Profiles are a shared overlay now, not a panel — stash the request and let
    // `profile::open_overlay` pop the modal from wherever you clicked a username.
    if matches!(req, SocialPanelRequest::Profile { .. }) {
        pending.0 = Some(req);
        return;
    }
    let id = match &req {
        SocialPanelRequest::Friends | SocialPanelRequest::FriendRequests => panels::friends::PANEL_ID,
        SocialPanelRequest::Chat { .. } => panels::chat::PANEL_ID,
        SocialPanelRequest::Feed { .. } => panels::feed::PANEL_ID,
        // The forum was replaced by feed channels — old forum deep-links (from
        // notifications, the palette, or a profile's activity) open the feed.
        SocialPanelRequest::Forum { .. } => panels::feed::PANEL_ID,
        // Teams folded into the Friends panel (Teams tab).
        SocialPanelRequest::Teams => panels::friends::PANEL_ID,
        SocialPanelRequest::Learn => panels::learn::PANEL_ID,
        // Handled above (no panel).
        SocialPanelRequest::Notifications | SocialPanelRequest::Profile { .. } => unreachable!(),
    };
    if let Some(mut dock) = dock {
        // Don't steal a panel that's currently torn off into a floating window.
        let in_float = windows
            .as_ref()
            .is_some_and(|ws| ws.0.iter().any(|w| w.tree.contains_panel(id)));
        if !in_float {
            dock.tree.focus_or_add_panel(id);
        }
    }
    pending.0 = Some(req);
}
