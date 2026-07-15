//! Friends panel — friends list with presence, incoming requests, and
//! add-friend search. Tabs: Friends / Requests / Add.

use std::collections::HashMap;

use bevy::prelude::*;
use crossbeam_channel::{unbounded, Receiver, Sender};
use renzora::core::{RenzoraShellExt, SocialBridge, SocialPanelRequest};
use renzora::SplashState;
use renzora_auth::social::{FriendEntry, FriendRequestEntry, PopularUser, PresenceEntry, UserSearchResult};
use renzora_auth::AuthSession;
use renzora_ember::dock::panel_active;
use renzora_ember::font::{ui_font, EmberFonts};
use renzora_ember::panel::RegisterPanelContent;
use renzora_ember::reactive::{bind_bg, bind_display, bind_text, keyed_list_tokened, KeyedSnapshot};
use renzora_ember::theme::*;
use renzora_ember::widgets::{
    accent_banner, accent_button, accent_card, accent_ghost, accent_icon_button, empty_state,
    text_input, EmberTextInput, HoverTooltip,
};

use crate::avatars::avatar_image;
use crate::toasts::{ToastQueue, Tone};
use crate::util::{self, hash64, session_clone, HUE_CHAT, HUE_FRIENDS};
use crate::PendingSocialRequest;

pub(crate) const PANEL_ID: &str = "social_friends";

const RED: (u8, u8, u8) = (224, 80, 80);
const SEARCH_DEBOUNCE_SECS: f64 = 0.4;

#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub(crate) enum FriendsTab {
    #[default]
    Friends,
    Requests,
    Add,
    /// Your teams, pending invites, and (rate-limited) create — folded in from
    /// the retired Teams panel. UI comes from [`crate::panels::teams`].
    Teams,
}

pub(crate) enum FriendsResult {
    Friends(Result<Vec<FriendEntry>, String>),
    Requests(Result<Vec<FriendRequestEntry>, String>),
    Presence(Result<Vec<PresenceEntry>, String>),
    Search(Result<Vec<UserSearchResult>, String>),
    Popular(Result<Vec<PopularUser>, String>),
    /// A friend action finished; Ok(message) → toast + refetch lists.
    Action(Result<String, String>),
    OpenDm(Result<String, String>),
}

#[derive(Resource)]
pub(crate) struct FriendsPanel {
    pub tab: FriendsTab,
    pub friends: Vec<FriendEntry>,
    pub requests: Vec<FriendRequestEntry>,
    /// user_id → online (fed by the presence endpoint + live WS events).
    pub online: HashMap<String, bool>,
    pub results: Vec<UserSearchResult>,
    pub popular: Vec<PopularUser>,
    pub popular_requested: bool,
    pub loading: bool,
    pub error: Option<String>,
    /// Bumped on any data change — keyed lists rebuild off this token.
    pub version: u64,
    pub loaded_once: bool,
    pub last_query: String,
    /// (query, deadline) awaiting the debounce window.
    pub pending_query: Option<(String, f64)>,
    pub tx: Sender<FriendsResult>,
    rx: Receiver<FriendsResult>,
}

impl Default for FriendsPanel {
    fn default() -> Self {
        let (tx, rx) = unbounded();
        Self {
            tab: FriendsTab::default(),
            friends: Vec::new(),
            requests: Vec::new(),
            online: HashMap::new(),
            results: Vec::new(),
            popular: Vec::new(),
            popular_requested: false,
            loading: false,
            error: None,
            version: 0,
            loaded_once: false,
            last_query: String::new(),
            pending_query: None,
            tx,
            rx,
        }
    }
}

impl FriendsPanel {
    pub fn bump(&mut self) {
        self.version = self.version.wrapping_add(1);
    }

    pub fn online_count(&self) -> u32 {
        self.online.values().filter(|v| **v).count() as u32
    }
}

pub(crate) fn register(app: &mut App) {
    app.init_resource::<FriendsPanel>();
    app.register_shell_panel(PANEL_ID, "Friends", "users", "Community");
    app.register_panel_content(PANEL_ID, true, build);
    app.add_systems(
        Update,
        (
            poll_results,
            auto_refresh.run_if(panel_active(PANEL_ID)),
            search_debounce.run_if(panel_active(PANEL_ID)),
            tab_clicks,
            action_clicks,
            consume_request,
        )
            .run_if(in_state(SplashState::Editor)),
    );
}

// ── Fetching ─────────────────────────────────────────────────────────────────

/// Refetch friends + requests + presence on background threads.
///
/// Marks `loaded_once` at SPAWN time (not on success): the one-shot baseline
/// snapshot must never auto-retry on failure — live updates come from the
/// WebSocket, and the refresh button / WS reconnect resync the rest.
pub(crate) fn refresh(panel: &mut FriendsPanel, session: &AuthSession) {
    if !session.is_signed_in() {
        return;
    }
    panel.loaded_once = true;
    panel.loading = true;
    panel.error = None;
    spawn_fetch(panel.tx.clone(), session_clone(session));
}

#[cfg(not(target_arch = "wasm32"))]
fn spawn_fetch(tx: Sender<FriendsResult>, session: AuthSession) {
    std::thread::spawn(move || {
        let _ = tx.send(FriendsResult::Friends(renzora_auth::social::get_friends(&session)));
        let _ = tx.send(FriendsResult::Requests(renzora_auth::social::get_friend_requests(&session)));
        let _ = tx.send(FriendsResult::Presence(renzora_auth::social::get_friends_presence(&session)));
    });
}
#[cfg(target_arch = "wasm32")]
fn spawn_fetch(_tx: Sender<FriendsResult>, _session: AuthSession) {}

#[cfg(not(target_arch = "wasm32"))]
fn spawn_search(tx: Sender<FriendsResult>, query: String) {
    std::thread::spawn(move || {
        let _ = tx.send(FriendsResult::Search(renzora_auth::social::search_users(&query)));
    });
}

#[cfg(not(target_arch = "wasm32"))]
fn spawn_popular(tx: Sender<FriendsResult>) {
    std::thread::spawn(move || {
        let _ = tx.send(FriendsResult::Popular(renzora_auth::social::get_popular_users()));
    });
}
#[cfg(target_arch = "wasm32")]
fn spawn_popular(_tx: Sender<FriendsResult>) {}
#[cfg(target_arch = "wasm32")]
fn spawn_search(_tx: Sender<FriendsResult>, _query: String) {}

enum FriendAction {
    /// Look up the username's id, then send a friend request.
    AddByName(String),
    Accept(String),
    Remove(String),
    OpenDm(String),
}

#[cfg(not(target_arch = "wasm32"))]
fn spawn_action(tx: Sender<FriendsResult>, session: AuthSession, action: FriendAction) {
    std::thread::spawn(move || {
        let result = match action {
            FriendAction::AddByName(username) => {
                renzora_auth::social::view_profile(&username, Some(&session))
                    .and_then(|p| renzora_auth::social::friend_add(&session, &p.id))
                    .map(|_| format!("Friend request sent to {username}"))
            }
            FriendAction::Accept(user_id) => renzora_auth::social::friend_accept(&session, &user_id)
                .map(|_| "Friend request accepted".to_string()),
            FriendAction::Remove(user_id) => renzora_auth::social::friend_remove(&session, &user_id)
                .map(|_| "Removed".to_string()),
            FriendAction::OpenDm(user_id) => {
                let r = renzora_auth::messages::open_dm(&session, &user_id).map(|r| r.conversation_id);
                let _ = tx.send(FriendsResult::OpenDm(r));
                return;
            }
        };
        let _ = tx.send(FriendsResult::Action(result));
    });
}
#[cfg(target_arch = "wasm32")]
fn spawn_action(_tx: Sender<FriendsResult>, _session: AuthSession, _action: FriendAction) {}

// ── Systems ──────────────────────────────────────────────────────────────────

fn poll_results(
    mut panel: ResMut<FriendsPanel>,
    session: Res<AuthSession>,
    mut bridge: ResMut<SocialBridge>,
    mut toasts: ResMut<ToastQueue>,
) {
    let mut got = Vec::new();
    while let Ok(r) = panel.rx.try_recv() {
        got.push(r);
    }
    for r in got {
        match r {
            FriendsResult::Friends(Ok(list)) => {
                panel.friends = list;
                panel.loading = false;
                panel.loaded_once = true;
                panel.bump();
            }
            FriendsResult::Requests(Ok(list)) => {
                panel.requests = list;
                panel.bump();
            }
            FriendsResult::Presence(Ok(list)) => {
                panel.online = list.into_iter().map(|p| (p.user_id, p.online)).collect();
                bridge.friends_online = panel.online_count();
                panel.bump();
            }
            FriendsResult::Search(Ok(list)) => {
                panel.results = list;
                panel.bump();
            }
            FriendsResult::Popular(Ok(list)) => {
                panel.popular = list;
                panel.bump();
            }
            FriendsResult::Popular(Err(_)) => {}
            FriendsResult::Action(Ok(msg)) => {
                toasts.push(Tone::Success, msg, None);
                refresh(&mut panel, &session);
            }
            FriendsResult::OpenDm(Ok(conversation_id)) => {
                bridge.open_panel_request = Some(SocialPanelRequest::Chat {
                    conversation_id: Some(conversation_id),
                });
            }
            FriendsResult::Friends(Err(e))
            | FriendsResult::Requests(Err(e))
            | FriendsResult::Presence(Err(e))
            | FriendsResult::Search(Err(e))
            | FriendsResult::OpenDm(Err(e)) => {
                panel.loading = false;
                panel.error = Some(e);
                panel.bump();
            }
            FriendsResult::Action(Err(e)) => {
                toasts.push(Tone::Error, e, None);
                refresh(&mut panel, &session);
            }
        }
    }
}

/// One-shot baseline fetch on first view. After that the WebSocket keeps the
/// panel live (friend_online/offline, notification-driven refreshes); a WS
/// reconnect clears `loaded_once` to resync. No polling.
fn auto_refresh(mut panel: ResMut<FriendsPanel>, session: Res<AuthSession>) {
    if session.is_signed_in() && !panel.loaded_once {
        refresh(&mut panel, &session);
    }
}

/// Debounced user search from the Add tab's input.
fn search_debounce(
    mut panel: ResMut<FriendsPanel>,
    time: Res<Time>,
    inputs: Query<&EmberTextInput, With<FriendsSearchInput>>,
) {
    let Ok(input) = inputs.single() else { return };
    let query = input.value.trim().to_string();
    let now = time.elapsed_secs_f64();

    if query != panel.last_query {
        panel.last_query = query.clone();
        if query.len() >= 2 {
            panel.pending_query = Some((query, now + SEARCH_DEBOUNCE_SECS));
        } else {
            panel.pending_query = None;
            if !panel.results.is_empty() {
                panel.results.clear();
                panel.bump();
            }
        }
        return;
    }
    if let Some((q, deadline)) = panel.pending_query.clone() {
        if now >= deadline {
            panel.pending_query = None;
            spawn_search(panel.tx.clone(), q);
        }
    }
}

/// Deep-link: `Friends` / `FriendRequests` / `Teams` requests select the right
/// tab (Teams was folded into this panel, so team notifications land here).
fn consume_request(
    mut pending: ResMut<PendingSocialRequest>,
    mut panel: ResMut<FriendsPanel>,
    mut teams: ResMut<crate::panels::teams::TeamsPanel>,
    session: Res<AuthSession>,
) {
    match &pending.0 {
        Some(SocialPanelRequest::Friends) => panel.tab = FriendsTab::Friends,
        Some(SocialPanelRequest::FriendRequests) => panel.tab = FriendsTab::Requests,
        Some(SocialPanelRequest::Teams) => {
            panel.tab = FriendsTab::Teams;
            // A team invite/join likely changed things — reload from scratch.
            crate::panels::teams::reload(&mut teams, &session);
        }
        _ => return,
    }
    pending.0 = None;
    panel.bump();
}

// ── Buttons ──────────────────────────────────────────────────────────────────

#[derive(Component)]
struct FriendsTabBtn(FriendsTab);
#[derive(Component)]
struct FriendsRefreshBtn;
#[derive(Component)]
struct FriendsSearchInput;
#[derive(Component)]
struct AcceptBtn(String);
#[derive(Component)]
struct DeclineBtn(String);
#[derive(Component)]
struct RemoveBtn(String);
#[derive(Component)]
struct MessageBtn(String);
#[derive(Component)]
struct ProfileBtn(String);
#[derive(Component)]
struct AddFriendBtn(String);

fn tab_clicks(
    mut panel: ResMut<FriendsPanel>,
    mut teams: ResMut<crate::panels::teams::TeamsPanel>,
    session: Res<AuthSession>,
    tabs: Query<(&Interaction, &FriendsTabBtn), Changed<Interaction>>,
    refresh_q: Query<&Interaction, (With<FriendsRefreshBtn>, Changed<Interaction>)>,
) {
    for (i, tab) in &tabs {
        if *i == Interaction::Pressed && panel.tab != tab.0 {
            panel.tab = tab.0;
            if tab.0 == FriendsTab::Add && !panel.popular_requested {
                panel.popular_requested = true;
                spawn_popular(panel.tx.clone());
            }
            // Lazy-load teams the first time the Teams tab is opened.
            if tab.0 == FriendsTab::Teams {
                crate::panels::teams::ensure_loaded(&mut teams, &session);
            }
            panel.bump();
        }
    }
    for i in &refresh_q {
        if *i == Interaction::Pressed {
            refresh(&mut panel, &session);
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn action_clicks(
    panel: Res<FriendsPanel>,
    session: Res<AuthSession>,
    mut bridge: ResMut<SocialBridge>,
    accepts: Query<(&Interaction, &AcceptBtn), Changed<Interaction>>,
    declines: Query<(&Interaction, &DeclineBtn), Changed<Interaction>>,
    removes: Query<(&Interaction, &RemoveBtn), Changed<Interaction>>,
    messages: Query<(&Interaction, &MessageBtn), Changed<Interaction>>,
    profiles: Query<(&Interaction, &ProfileBtn), Changed<Interaction>>,
    adds: Query<(&Interaction, &AddFriendBtn), Changed<Interaction>>,
) {
    let pressed = |i: &Interaction| *i == Interaction::Pressed;
    for (i, b) in &accepts {
        if pressed(i) {
            spawn_action(panel.tx.clone(), session_clone(&session), FriendAction::Accept(b.0.clone()));
        }
    }
    for (i, b) in &declines {
        if pressed(i) {
            spawn_action(panel.tx.clone(), session_clone(&session), FriendAction::Remove(b.0.clone()));
        }
    }
    for (i, b) in &removes {
        if pressed(i) {
            spawn_action(panel.tx.clone(), session_clone(&session), FriendAction::Remove(b.0.clone()));
        }
    }
    for (i, b) in &messages {
        if pressed(i) {
            spawn_action(panel.tx.clone(), session_clone(&session), FriendAction::OpenDm(b.0.clone()));
        }
    }
    for (i, b) in &profiles {
        if pressed(i) {
            bridge.open_panel_request = Some(SocialPanelRequest::Profile { username: Some(b.0.clone()) });
        }
    }
    for (i, b) in &adds {
        if pressed(i) {
            spawn_action(panel.tx.clone(), session_clone(&session), FriendAction::AddByName(b.0.clone()));
        }
    }
}

// ── Build ────────────────────────────────────────────────────────────────────

fn build(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let root = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(8.0),
            padding: UiRect::all(Val::Px(10.0)),
            ..default()
        })
        .id();

    // Signed-out empty state.
    let signed_out = empty_state(
        commands,
        fonts,
        HUE_FRIENDS,
        "users",
        "Sign in to find your people",
        Some("Friends, requests, and who's online live here"),
    );
    bind_display(commands, signed_out, |w| !util::signed_in(w));

    let body = commands
        .spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(8.0), ..default() })
        .id();
    bind_display(commands, body, util::signed_in);

    // Identity banner with a live "N online" subtitle and refresh action.
    let banner = accent_banner(commands, fonts, HUE_FRIENDS, "users", "Friends", "Your people — online and off");
    bind_text(commands, banner.subtitle, |w| {
        let Some(p) = w.get_resource::<FriendsPanel>() else {
            return "Your people — online and off".to_string();
        };
        let online = p.online_count();
        match (p.friends.len(), online) {
            (0, _) => "Your people — online and off".to_string(),
            (n, 0) => format!("{n} friends — nobody online right now"),
            (n, o) => format!("{n} friends — {o} online now"),
        }
    });
    let refresh_btn = accent_icon_button(commands, fonts, HUE_FRIENDS, "arrows-clockwise");
    commands.entity(refresh_btn).insert(FriendsRefreshBtn);
    commands.entity(banner.actions).add_child(refresh_btn);

    // Tab row — hand-built so the Requests label can carry a live count.
    let tabs = commands
        .spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Row, column_gap: Val::Px(4.0), align_items: AlignItems::Center, ..default() })
        .id();
    let mut tab_kids = Vec::new();
    for (tab, label) in [
        (FriendsTab::Friends, "Friends"),
        (FriendsTab::Requests, "Requests"),
        (FriendsTab::Add, "Add friends"),
        (FriendsTab::Teams, "Teams"),
    ] {
        let btn = commands
            .spawn((
                Node {
                    padding: UiRect::axes(Val::Px(11.0), Val::Px(4.0)),
                    align_items: AlignItems::Center,
                    border_radius: BorderRadius::all(Val::Px(5.0)),
                    ..default()
                },
                BackgroundColor(rgb(hover_bg())),
                Interaction::default(),
                FriendsTabBtn(tab),
            ))
            .id();
        // Reactive: active tab is filled with the area hue.
        bind_bg(commands, btn, move |w| {
            let active = w.get_resource::<FriendsPanel>().map(|p| p.tab == tab).unwrap_or(false);
            if active {
                renzora_ember::widgets::tint(HUE_FRIENDS, 200)
            } else {
                rgb(hover_bg())
            }
        });
        let text = commands
            .spawn((Text::new(label.to_string()), ui_font(&fonts.ui, 10.5), TextColor(rgb(text_primary()))))
            .id();
        if tab == FriendsTab::Requests {
            bind_text(commands, text, |w| {
                let n = w.get_resource::<FriendsPanel>().map(|p| p.requests.len()).unwrap_or(0);
                if n > 0 { format!("Requests ({n})") } else { "Requests".to_string() }
            });
        }
        commands.entity(btn).add_child(text);
        tab_kids.push(btn);
    }
    commands.entity(tabs).add_children(&tab_kids);

    // Error line.
    let error = commands
        .spawn((Text::new(""), ui_font(&fonts.ui, 10.5), TextColor(rgb(RED))))
        .id();
    bind_text(commands, error, |w| {
        w.get_resource::<FriendsPanel>().and_then(|p| p.error.clone()).unwrap_or_default()
    });
    bind_display(commands, error, |w| {
        w.get_resource::<FriendsPanel>().map(|p| p.error.is_some()).unwrap_or(false)
    });

    // Friends list.
    let friends_wrap = commands
        .spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(3.0), ..default() })
        .id();
    bind_display(commands, friends_wrap, |w| {
        w.get_resource::<FriendsPanel>().map(|p| p.tab == FriendsTab::Friends).unwrap_or(false)
    });
    keyed_list_tokened(
        commands,
        friends_wrap,
        |w| w.get_resource::<FriendsPanel>().map(|p| p.version).unwrap_or(0),
        friends_snapshot,
    );

    // Requests list.
    let requests_wrap = commands
        .spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(3.0), ..default() })
        .id();
    bind_display(commands, requests_wrap, |w| {
        w.get_resource::<FriendsPanel>().map(|p| p.tab == FriendsTab::Requests).unwrap_or(false)
    });
    keyed_list_tokened(
        commands,
        requests_wrap,
        |w| w.get_resource::<FriendsPanel>().map(|p| p.version).unwrap_or(0),
        requests_snapshot,
    );

    // Add tab: search input + results.
    let add_wrap = commands
        .spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(6.0), ..default() })
        .id();
    bind_display(commands, add_wrap, |w| {
        w.get_resource::<FriendsPanel>().map(|p| p.tab == FriendsTab::Add).unwrap_or(false)
    });
    let search = text_input(commands, &fonts.ui, "Search users (min 2 chars)...", "");
    commands.entity(search).insert(FriendsSearchInput);
    let results_wrap = commands
        .spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(3.0), ..default() })
        .id();
    keyed_list_tokened(
        commands,
        results_wrap,
        |w| w.get_resource::<FriendsPanel>().map(|p| p.version).unwrap_or(0),
        search_snapshot,
    );
    commands.entity(add_wrap).add_children(&[search, results_wrap]);

    // Teams tab: the whole teams section (list + invites + create + detail),
    // which builds its own keyed lists tokened on `TeamsPanel.version`.
    let teams_wrap = commands
        .spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, ..default() })
        .id();
    bind_display(commands, teams_wrap, |w| {
        w.get_resource::<FriendsPanel>().map(|p| p.tab == FriendsTab::Teams).unwrap_or(false)
    });
    let teams_section = crate::panels::teams::build_section(commands, fonts);
    commands.entity(teams_wrap).add_child(teams_section);

    commands.entity(body).add_children(&[banner.root, tabs, error, friends_wrap, requests_wrap, add_wrap, teams_wrap]);
    commands.entity(root).add_children(&[signed_out, body]);
    root
}

// ── Snapshots / rows ─────────────────────────────────────────────────────────

fn friends_snapshot(w: &World) -> KeyedSnapshot {
    let Some(panel) = w.get_resource::<FriendsPanel>() else {
        return util::empty_snapshot();
    };
    let mut friends = panel.friends.clone();
    let online = panel.online.clone();
    // Online friends first, then alphabetical.
    friends.sort_by(|a, b| {
        let ao = online.get(&a.user_id).copied().unwrap_or(false);
        let bo = online.get(&b.user_id).copied().unwrap_or(false);
        bo.cmp(&ao).then_with(|| a.username.to_lowercase().cmp(&b.username.to_lowercase()))
    });
    if friends.is_empty() {
        if panel.loading {
            return note_snapshot("Loading...");
        }
        return KeyedSnapshot {
            items: vec![(u64::MAX, 1)],
            build: Box::new(|commands, fonts, _| {
                empty_state(
                    commands,
                    fonts,
                    HUE_FRIENDS,
                    "user-plus",
                    "No friends yet",
                    Some("Head to the Add friends tab and search for someone"),
                )
            }),
        };
    }
    let items = friends
        .iter()
        .map(|f| {
            let on = online.get(&f.user_id).copied().unwrap_or(false);
            (hash64(&f.user_id), hash64(&(&f.username, &f.avatar_url, on)))
        })
        .collect();
    KeyedSnapshot {
        items,
        build: Box::new(move |commands, fonts, i| {
            let f = &friends[i];
            friend_row(commands, fonts, f, online.get(&f.user_id).copied().unwrap_or(false), i)
        }),
    }
}

fn requests_snapshot(w: &World) -> KeyedSnapshot {
    let Some(panel) = w.get_resource::<FriendsPanel>() else {
        return util::empty_snapshot();
    };
    let requests = panel.requests.clone();
    if requests.is_empty() {
        return KeyedSnapshot {
            items: vec![(u64::MAX, 2)],
            build: Box::new(|commands, fonts, _| {
                empty_state(commands, fonts, HUE_FRIENDS, "envelope-open", "No requests waiting", None)
            }),
        };
    }
    let items = requests
        .iter()
        .map(|r| (hash64(&r.from_user_id), hash64(&(&r.username, &r.avatar_url))))
        .collect();
    KeyedSnapshot {
        items,
        build: Box::new(move |commands, fonts, i| request_row(commands, fonts, &requests[i])),
    }
}

fn search_snapshot(w: &World) -> KeyedSnapshot {
    let Some(panel) = w.get_resource::<FriendsPanel>() else {
        return util::empty_snapshot();
    };
    let results = panel.results.clone();
    // No query yet → suggest popular community members instead of a blank list.
    if results.is_empty() && panel.last_query.len() < 2 {
        let popular = panel.popular.clone();
        if popular.is_empty() {
            return note_snapshot("Type a username, or check back for popular creators");
        }
        let friend_names: std::collections::HashSet<String> =
            panel.friends.iter().map(|f| f.username.to_lowercase()).collect();
        let mut items: Vec<(u64, u64)> = vec![(u64::MAX - 1, 0)];
        items.extend(popular.iter().map(|u| (hash64(&u.username), hash64(&u.follower_count))));
        return KeyedSnapshot {
            items,
            build: Box::new(move |commands, fonts, i| {
                if i == 0 {
                    return commands
                        .spawn((Text::new("POPULAR IN THE COMMUNITY"), ui_font(&fonts.ui, 8.5), TextColor(rgb(placeholder())), Node { margin: UiRect::top(Val::Px(4.0)), ..default() }))
                        .id();
                }
                let u = &popular[i - 1];
                popular_row(commands, fonts, u, friend_names.contains(&u.username.to_lowercase()))
            }),
        };
    }
    if results.is_empty() {
        return note_snapshot("No users found");
    }
    let friend_ids: std::collections::HashSet<String> =
        panel.friends.iter().map(|f| f.username.to_lowercase()).collect();
    let items = results
        .iter()
        .map(|u| (hash64(&u.username), hash64(&(&u.avatar_url, &u.role))))
        .collect();
    KeyedSnapshot {
        items,
        build: Box::new(move |commands, fonts, i| {
            let u = &results[i];
            search_row(commands, fonts, u, friend_ids.contains(&u.username.to_lowercase()))
        }),
    }
}

fn note_snapshot(text: &'static str) -> KeyedSnapshot {
    KeyedSnapshot {
        items: vec![(u64::MAX, hash64(text))],
        build: Box::new(move |commands, fonts, _| {
            commands
                .spawn((Text::new(text), ui_font(&fonts.ui, 11.0), TextColor(rgb(placeholder())), Node { margin: UiRect::top(Val::Px(8.0)), ..default() }))
                .id()
        }),
    }
}

/// Old-Facebook-sidebar style: compact striped rows — avatar with presence
/// dot, name, and icon-only actions with tooltips.
fn friend_row(commands: &mut Commands, fonts: &EmberFonts, f: &FriendEntry, online: bool, index: usize) -> Entity {
    let row = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(7.0),
                padding: UiRect::axes(Val::Px(6.0), Val::Px(3.0)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(rgb(if index.is_multiple_of(2) { row_even() } else { row_odd() })),
        ))
        .id();
    let av = crate::avatars::avatar_with_presence(commands, fonts, f.avatar_url.as_deref(), 22.0, online);
    // The name itself opens the profile — no separate profile icon button.
    let name = commands
        .spawn((
            Text::new(f.username.clone()),
            ui_font(&fonts.ui, 11.5),
            TextColor(if online { rgb(text_primary()) } else { rgb(text_muted()) }),
            Node { flex_grow: 1.0, ..default() },
            Interaction::default(),
            ProfileBtn(f.username.clone()),
            renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            HoverTooltip::new("View profile"),
        ))
        .id();
    let msg = accent_icon_button(commands, fonts, HUE_CHAT, "chat-circle");
    commands.entity(msg).insert((MessageBtn(f.user_id.clone()), HoverTooltip::new("Message")));
    let rm = accent_icon_button(commands, fonts, RED, "user-minus");
    commands.entity(rm).insert((RemoveBtn(f.user_id.clone()), HoverTooltip::new("Remove friend")));
    commands.entity(row).add_children(&[av, name, msg, rm]);
    row
}

fn request_row(commands: &mut Commands, fonts: &EmberFonts, r: &FriendRequestEntry) -> Entity {
    let row = accent_card(commands, false);
    let av = avatar_image(commands, fonts, r.avatar_url.as_deref(), 28.0);
    let col = commands
        .spawn(Node { flex_direction: FlexDirection::Column, row_gap: Val::Px(1.0), flex_grow: 1.0, ..default() })
        .id();
    let name = commands
        .spawn((Text::new(r.username.clone()), ui_font(&fonts.ui, 12.0), TextColor(rgb(text_primary()))))
        .id();
    let when = r.sent_at.as_deref().map(util::relative_time).unwrap_or_default();
    let sub = commands
        .spawn((
            Text::new(if when.is_empty() { "wants to be your friend".to_string() } else { format!("wants to be your friend · {when}") }),
            ui_font(&fonts.ui, 9.0),
            TextColor(rgb(text_muted())),
        ))
        .id();
    commands.entity(col).add_children(&[name, sub]);
    let accept = accent_button(commands, fonts, HUE_FRIENDS, "Accept");
    commands.entity(accept).insert(AcceptBtn(r.from_user_id.clone()));
    let decline = accent_ghost(commands, fonts, RED, "Decline");
    commands.entity(decline).insert(DeclineBtn(r.from_user_id.clone()));
    commands.entity(row).add_children(&[av, col, accept, decline]);
    row
}

fn search_row(commands: &mut Commands, fonts: &EmberFonts, u: &UserSearchResult, already: bool) -> Entity {
    let row = accent_card(commands, false);
    let av = avatar_image(commands, fonts, u.avatar_url.as_deref(), 28.0);
    let name = commands
        .spawn((
            Text::new(u.username.clone()),
            ui_font(&fonts.ui, 12.0),
            TextColor(rgb(text_primary())),
            Node { flex_grow: 1.0, ..default() },
            Interaction::default(),
            ProfileBtn(u.username.clone()),
            renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            HoverTooltip::new("View profile"),
        ))
        .id();
    let mut kids = vec![av, name];
    if already {
        kids.push(renzora_ember::widgets::accent_chip(commands, fonts, HUE_FRIENDS, Some("check"), "Friends"));
    } else {
        let add = accent_button(commands, fonts, HUE_FRIENDS, "Add friend");
        commands.entity(add).insert(AddFriendBtn(u.username.clone()));
        kids.push(add);
    }
    commands.entity(row).add_children(&kids);
    row
}


fn popular_row(commands: &mut Commands, fonts: &EmberFonts, u: &PopularUser, already: bool) -> Entity {
    let row = accent_card(commands, false);
    let av = avatar_image(commands, fonts, u.avatar_url.as_deref(), 26.0);
    let col = commands
        .spawn(Node { flex_direction: FlexDirection::Column, flex_grow: 1.0, ..default() })
        .id();
    let name = commands
        .spawn((
            Text::new(u.username.clone()),
            ui_font(&fonts.ui, 11.5),
            TextColor(rgb(text_primary())),
            Interaction::default(),
            ProfileBtn(u.username.clone()),
            renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            HoverTooltip::new("View profile"),
        ))
        .id();
    let subs = commands
        .spawn((Text::new(format!("{} followers", u.follower_count)), ui_font(&fonts.ui, 9.0), TextColor(rgb(text_muted()))))
        .id();
    commands.entity(col).add_children(&[name, subs]);
    let mut kids = vec![av, col];
    if !already {
        let add = accent_button(commands, fonts, HUE_FRIENDS, "Add");
        commands.entity(add).insert(AddFriendBtn(u.username.clone()));
        kids.push(add);
    }
    commands.entity(row).add_children(&kids);
    row
}
