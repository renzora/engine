//! Renzora Community — the social layer of the editor, backed by renzora.com.
//!
//! Panels: Friends (with a Teams tab), Chat, Feed, Forum, Learn. Profiles are a
//! shared overlay and notifications are the top-bar bell dropdown — neither is a
//! dockable panel. Live events arrive over the site's WebSocket; everything else
//! is blocking HTTP on worker threads (the engine-wide convention — no async
//! runtime).

mod account_settings;
mod avatars;
mod confetti;
mod lightbox;
mod notify_dropdown;
mod panels;
mod reaction_picker;
mod routing;
mod settings;
mod toasts;
mod util;
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
