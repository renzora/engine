//! Teams — teams list, pending invites, create-a-team, and team detail
//! (members, invite by username/email + the "Open Team Chat" bridge into the
//! Messages panel).
//!
//! Teams has **no panel of its own**: a dedicated Teams tab felt like wasted
//! real estate, so this module is data + [`build_section`], embedded as the
//! Friends panel's "Teams" tab. Team creation is rate-limited (see
//! [`CREATE_COOLDOWN`]) so the button can't be spammed.

use std::time::Instant;

use bevy::prelude::*;
use crossbeam_channel::{unbounded, Receiver, Sender};
use renzora::core::{SocialBridge, SocialPanelRequest};
use renzora::SplashState;
use renzora_auth::teams::{Team, TeamDetail, TeamInvite};
use renzora_auth::AuthSession;
use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::reactive::{bind_display, bind_text, keyed_list_tokened, KeyedSnapshot};
use renzora_ember::theme::*;
use renzora_ember::widgets::{
    accent_button, accent_chip, accent_ghost, accent_icon_button, icon_badge, text_input,
    EmberForm, EmberTextInput,
};

use crate::toasts::{ToastQueue, Tone};
use crate::util::{self, hash64, session_clone, HUE_CHAT, HUE_TEAMS};

const RED: (u8, u8, u8) = (224, 80, 80);

/// Minimum spacing between team creations, client-side, so the Create button
/// can't be hammered into spamming the API.
const CREATE_COOLDOWN: std::time::Duration = std::time::Duration::from_secs(20);

pub(crate) enum TeamsResult {
    Teams(Result<Vec<Team>, String>),
    Invites(Result<Vec<TeamInvite>, String>),
    Detail(Result<TeamDetail, String>),
    /// Action finished; Ok(message) → toast + refresh.
    Action(Result<String, String>),
    /// Team conversation ready → jump to chat.
    Conversation(Result<String, String>),
}

#[derive(Resource)]
pub(crate) struct TeamsPanel {
    /// `None` = list view; `Some(team_id)` = detail view.
    pub open_team: Option<String>,
    pub teams: Vec<Team>,
    pub invites: Vec<TeamInvite>,
    pub detail: Option<TeamDetail>,
    pub loading: bool,
    pub error: Option<String>,
    pub version: u64,
    pub loaded_once: bool,
    /// When the last team was created — gates [`CREATE_COOLDOWN`].
    pub last_create: Option<Instant>,
    pub tx: Sender<TeamsResult>,
    rx: Receiver<TeamsResult>,
}

impl Default for TeamsPanel {
    fn default() -> Self {
        let (tx, rx) = unbounded();
        Self {
            open_team: None,
            teams: Vec::new(),
            invites: Vec::new(),
            detail: None,
            loading: false,
            error: None,
            version: 0,
            loaded_once: false,
            last_create: None,
            tx,
            rx,
        }
    }
}

impl TeamsPanel {
    pub fn bump(&mut self) {
        self.version = self.version.wrapping_add(1);
    }
}

pub(crate) fn register(app: &mut App) {
    app.init_resource::<TeamsPanel>();
    // Data + click systems only — the UI lives inside the Friends panel.
    app.add_systems(Update, (poll_results, clicks).run_if(in_state(SplashState::Editor)));
}

/// Baseline-load teams + invites once (called when the Friends "Teams" tab is
/// first opened, and re-armed by a `Teams` deep-link request).
pub(crate) fn ensure_loaded(panel: &mut TeamsPanel, session: &AuthSession) {
    if !panel.loaded_once {
        refresh(panel, session);
    }
}

/// Re-arm a full reload (a deep-link into Teams: invites likely changed).
pub(crate) fn reload(panel: &mut TeamsPanel, session: &AuthSession) {
    panel.open_team = None;
    panel.loaded_once = false;
    refresh(panel, session);
    panel.bump();
}

// ── Fetching ─────────────────────────────────────────────────────────────────

#[cfg(not(target_arch = "wasm32"))]
fn spawn_thread(f: impl FnOnce() + Send + 'static) {
    std::thread::spawn(f);
}
#[cfg(target_arch = "wasm32")]
fn spawn_thread(_f: impl FnOnce() + Send + 'static) {}

/// One-shot baseline fetch; marks `loaded_once` at SPAWN time so a failure can
/// never auto-retry — team changes arrive as notifications over the WebSocket.
fn refresh(panel: &mut TeamsPanel, session: &AuthSession) {
    if !session.is_signed_in() {
        return;
    }
    panel.loaded_once = true;
    panel.loading = true;
    panel.error = None;
    let tx = panel.tx.clone();
    let session = session_clone(session);
    let open = panel.open_team.clone();
    spawn_thread(move || {
        let _ = tx.send(TeamsResult::Teams(renzora_auth::teams::list_teams(&session)));
        let _ = tx.send(TeamsResult::Invites(renzora_auth::teams::list_invites(&session)));
        if let Some(team_id) = open {
            let _ = tx.send(TeamsResult::Detail(renzora_auth::teams::get_team(&session, &team_id)));
        }
    });
}

fn open_detail(panel: &mut TeamsPanel, session: &AuthSession, team_id: String) {
    panel.open_team = Some(team_id.clone());
    panel.detail = None;
    panel.bump();
    let tx = panel.tx.clone();
    let session = session_clone(session);
    spawn_thread(move || {
        let _ = tx.send(TeamsResult::Detail(renzora_auth::teams::get_team(&session, &team_id)));
    });
}

// ── Systems ──────────────────────────────────────────────────────────────────

fn poll_results(
    mut panel: ResMut<TeamsPanel>,
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
            TeamsResult::Teams(Ok(list)) => {
                panel.teams = list;
                panel.loading = false;
                panel.loaded_once = true;
                panel.bump();
            }
            TeamsResult::Invites(Ok(list)) => {
                panel.invites = list.into_iter().filter(|i| i.status == "pending").collect();
                panel.bump();
            }
            TeamsResult::Detail(Ok(detail)) => {
                if panel.open_team.as_deref() == Some(detail.team.id.as_str()) {
                    panel.detail = Some(detail);
                    panel.bump();
                }
            }
            TeamsResult::Action(Ok(msg)) => {
                toasts.push(Tone::Success, msg, None);
                refresh(&mut panel, &session);
            }
            TeamsResult::Action(Err(e)) => {
                toasts.push(Tone::Error, e, None);
                refresh(&mut panel, &session);
            }
            TeamsResult::Conversation(Ok(conversation_id)) => {
                bridge.open_panel_request = Some(SocialPanelRequest::Chat {
                    conversation_id: Some(conversation_id),
                });
            }
            TeamsResult::Conversation(Err(e)) => {
                toasts.push(Tone::Error, format!("Couldn't open team chat: {e}"), None);
            }
            TeamsResult::Teams(Err(e)) | TeamsResult::Invites(Err(e)) | TeamsResult::Detail(Err(e)) => {
                panel.loading = false;
                panel.error = Some(e);
                panel.bump();
            }
        }
    }
}

// ── Clicks ───────────────────────────────────────────────────────────────────

#[derive(Component)]
struct TeamOpenBtn(String);
#[derive(Component)]
struct TeamsBackBtn;
#[derive(Component)]
struct TeamsRefreshBtn;
#[derive(Component)]
struct InviteAcceptBtn(String);
#[derive(Component)]
struct InviteDeclineBtn(String);
#[derive(Component)]
struct TeamChatBtn(String);
#[derive(Component)]
struct TeamInviteInput;
#[derive(Component)]
struct TeamInviteSendBtn(String);
#[derive(Component)]
struct TeamCreateInput;
#[derive(Component)]
struct TeamCreateBtn;
#[derive(Component)]
struct MemberRemoveBtn { team_id: String, user_id: String }

#[allow(clippy::too_many_arguments)]
fn clicks(
    mut panel: ResMut<TeamsPanel>,
    session: Res<AuthSession>,
    mut toasts: ResMut<ToastQueue>,
    opens: Query<(&Interaction, &TeamOpenBtn), Changed<Interaction>>,
    backs: Query<&Interaction, (With<TeamsBackBtn>, Changed<Interaction>)>,
    refreshes: Query<&Interaction, (With<TeamsRefreshBtn>, Changed<Interaction>)>,
    accepts: Query<(&Interaction, &InviteAcceptBtn), Changed<Interaction>>,
    declines: Query<(&Interaction, &InviteDeclineBtn), Changed<Interaction>>,
    chats: Query<(&Interaction, &TeamChatBtn), Changed<Interaction>>,
    invite_sends: Query<(&Interaction, &TeamInviteSendBtn), Changed<Interaction>>,
    creates: Query<&Interaction, (With<TeamCreateBtn>, Changed<Interaction>)>,
    removes: Query<(&Interaction, &MemberRemoveBtn), Changed<Interaction>>,
    mut invite_inputs: Query<&mut EmberTextInput, (With<TeamInviteInput>, Without<TeamCreateInput>)>,
    mut create_inputs: Query<&mut EmberTextInput, (With<TeamCreateInput>, Without<TeamInviteInput>)>,
) {
    let pressed = |i: &Interaction| *i == Interaction::Pressed;

    for (i, b) in &opens {
        if pressed(i) {
            open_detail(&mut panel, &session, b.0.clone());
        }
    }
    for i in &backs {
        if pressed(i) {
            panel.open_team = None;
            panel.detail = None;
            panel.bump();
        }
    }
    for i in &refreshes {
        if pressed(i) {
            refresh(&mut panel, &session);
        }
    }
    for (i, b) in &accepts {
        if pressed(i) {
            let tx = panel.tx.clone();
            let s = session_clone(&session);
            let id = b.0.clone();
            spawn_thread(move || {
                let r = renzora_auth::teams::accept_invite(&s, &id).map(|_| "Invite accepted".to_string());
                let _ = tx.send(TeamsResult::Action(r));
            });
        }
    }
    for (i, b) in &declines {
        if pressed(i) {
            let tx = panel.tx.clone();
            let s = session_clone(&session);
            let id = b.0.clone();
            spawn_thread(move || {
                let r = renzora_auth::teams::decline_invite(&s, &id).map(|_| "Invite declined".to_string());
                let _ = tx.send(TeamsResult::Action(r));
            });
        }
    }
    for (i, b) in &chats {
        if pressed(i) {
            let tx = panel.tx.clone();
            let s = session_clone(&session);
            let id = b.0.clone();
            spawn_thread(move || {
                let r = renzora_auth::teams::get_team_conversation(&s, &id).map(|r| r.conversation_id);
                let _ = tx.send(TeamsResult::Conversation(r));
            });
        }
    }
    for (i, b) in &invite_sends {
        if pressed(i) {
            if let Ok(mut input) = invite_inputs.single_mut() {
                let identifier = input.value.trim().to_string();
                if identifier.is_empty() {
                    continue;
                }
                input.value.clear();
                let tx = panel.tx.clone();
                let s = session_clone(&session);
                let team_id = b.0.clone();
                spawn_thread(move || {
                    let r = renzora_auth::teams::invite_member(&s, &team_id, &identifier, None)
                        .map(|_| format!("Invited {identifier}"));
                    let _ = tx.send(TeamsResult::Action(r));
                });
            }
        }
    }
    for i in &creates {
        if pressed(i) {
            // Rate-limit: refuse a second create inside the cooldown so the
            // button can't be spammed into a burst of teams.
            if let Some(last) = panel.last_create {
                if last.elapsed() < CREATE_COOLDOWN {
                    let secs = (CREATE_COOLDOWN - last.elapsed()).as_secs() + 1;
                    toasts.push(Tone::Error, format!("Hold on — you can create another team in {secs}s"), None);
                    continue;
                }
            }
            if let Ok(mut input) = create_inputs.single_mut() {
                let name = input.value.trim().to_string();
                if name.is_empty() {
                    continue;
                }
                input.value.clear();
                panel.last_create = Some(Instant::now());
                let tx = panel.tx.clone();
                let s = session_clone(&session);
                spawn_thread(move || {
                    let r = renzora_auth::teams::create_team(&s, &name, "")
                        .map(|t| format!("Created team {}", t.name));
                    let _ = tx.send(TeamsResult::Action(r));
                });
            }
        }
    }
    for (i, b) in &removes {
        if pressed(i) {
            let tx = panel.tx.clone();
            let s = session_clone(&session);
            let team_id = b.team_id.clone();
            let user_id = b.user_id.clone();
            spawn_thread(move || {
                let r = renzora_auth::teams::remove_member(&s, &team_id, &user_id)
                    .map(|_| "Member removed".to_string());
                let _ = tx.send(TeamsResult::Action(r));
            });
        }
    }
}

// ── Section (embedded in the Friends panel's Teams tab) ──────────────────────

/// The Teams UI, minus any signed-out state or banner — the Friends panel that
/// hosts it supplies both. Returns a column: header (Back on detail + refresh),
/// error line, list view (invites + teams + create), and detail view.
pub(crate) fn build_section(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let body = commands
        .spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(6.0), ..default() })
        .id();

    // Header: back (detail only) + title + refresh.
    let header = commands
        .spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(6.0), ..default() })
        .id();
    let back = accent_ghost(commands, fonts, HUE_TEAMS, "Back");
    commands.entity(back).insert(TeamsBackBtn);
    bind_display(commands, back, |w| {
        w.get_resource::<TeamsPanel>().map(|p| p.open_team.is_some()).unwrap_or(false)
    });
    let title = commands
        .spawn((Text::new("Teams"), ui_font(&fonts.ui, 12.5), TextColor(rgb(text_primary())), Node { flex_grow: 1.0, ..default() }))
        .id();
    bind_text(commands, title, |w| {
        let Some(p) = w.get_resource::<TeamsPanel>() else { return "Teams".into() };
        match (&p.open_team, &p.detail) {
            (Some(_), Some(d)) => d.team.name.clone(),
            (Some(_), None) => "Loading...".into(),
            (None, _) => "Teams".into(),
        }
    });
    let refresh_btn = accent_icon_button(commands, fonts, HUE_TEAMS, "arrows-clockwise");
    commands.entity(refresh_btn).insert(TeamsRefreshBtn);
    commands.entity(header).add_children(&[back, title, refresh_btn]);

    // Error line.
    let error = commands
        .spawn((Text::new(""), ui_font(&fonts.ui, 10.5), TextColor(rgb(RED))))
        .id();
    bind_text(commands, error, |w| {
        w.get_resource::<TeamsPanel>().and_then(|p| p.error.clone()).unwrap_or_default()
    });
    bind_display(commands, error, |w| {
        w.get_resource::<TeamsPanel>().map(|p| p.error.is_some()).unwrap_or(false)
    });

    // ── List view ──
    let list_view = commands
        .spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(6.0), ..default() })
        .id();
    bind_display(commands, list_view, |w| {
        w.get_resource::<TeamsPanel>().map(|p| p.open_team.is_none()).unwrap_or(true)
    });

    // Invites section.
    let invites_list = commands
        .spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(3.0), ..default() })
        .id();
    keyed_list_tokened(
        commands,
        invites_list,
        |w| w.get_resource::<TeamsPanel>().map(|p| p.version).unwrap_or(0),
        invites_snapshot,
    );

    // Teams list.
    let teams_list = commands
        .spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(3.0), ..default() })
        .id();
    keyed_list_tokened(
        commands,
        teams_list,
        |w| w.get_resource::<TeamsPanel>().map(|p| p.version).unwrap_or(0),
        teams_snapshot,
    );

    // Create team row.
    let create_row = commands
        .spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Row, column_gap: Val::Px(6.0), align_items: AlignItems::Center, margin: UiRect::top(Val::Px(4.0)), ..default() })
        .id();
    let create_input = text_input(commands, &fonts.ui, "New team name...", "");
    commands.entity(create_input).insert((TeamCreateInput, Node { flex_grow: 1.0, ..default() }));
    let create_btn = accent_button(commands, fonts, HUE_TEAMS, "Create team");
    commands.entity(create_btn).insert(TeamCreateBtn);
    // Enter in the name field presses "Create team".
    commands.entity(create_row).insert(EmberForm { submit: create_btn });
    commands.entity(create_row).add_children(&[create_input, create_btn]);

    commands.entity(list_view).add_children(&[invites_list, teams_list, create_row]);

    // ── Detail view ──
    let detail_view = commands
        .spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(6.0), ..default() })
        .id();
    bind_display(commands, detail_view, |w| {
        w.get_resource::<TeamsPanel>().map(|p| p.open_team.is_some()).unwrap_or(false)
    });
    let detail_body = commands
        .spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(3.0), ..default() })
        .id();
    keyed_list_tokened(
        commands,
        detail_body,
        |w| w.get_resource::<TeamsPanel>().map(|p| p.version).unwrap_or(0),
        detail_snapshot,
    );
    commands.entity(detail_view).add_child(detail_body);

    commands.entity(body).add_children(&[header, error, list_view, detail_view]);
    body
}

// ── Snapshots / rows ─────────────────────────────────────────────────────────

fn invites_snapshot(w: &World) -> KeyedSnapshot {
    let Some(panel) = w.get_resource::<TeamsPanel>() else {
        return util::empty_snapshot();
    };
    if panel.invites.is_empty() {
        return util::empty_snapshot();
    }
    let invites = panel.invites.clone();
    let teams: std::collections::HashMap<String, String> =
        panel.teams.iter().map(|t| (t.id.clone(), t.name.clone())).collect();
    let keys = invites.iter().map(|i| (hash64(&i.id), hash64(&i.status))).collect();
    KeyedSnapshot {
        items: keys,
        build: Box::new(move |commands, fonts, idx| {
            let inv = &invites[idx];
            let row = row_base(commands);
            let ic = icon_badge(commands, fonts, HUE_TEAMS, "envelope", 26.0);
            let team_name = teams.get(&inv.team_id).cloned().unwrap_or_else(|| "a team".to_string());
            let label = commands
                .spawn((
                    Text::new(format!("Invited to join {team_name} as {}", inv.role)),
                    ui_font(&fonts.ui, 11.5),
                    TextColor(rgb(text_primary())),
                    Node { flex_grow: 1.0, ..default() },
                ))
                .id();
            let accept = accent_button(commands, fonts, HUE_TEAMS, "Accept");
            commands.entity(accept).insert(InviteAcceptBtn(inv.id.clone()));
            let decline = accent_ghost(commands, fonts, RED, "Decline");
            commands.entity(decline).insert(InviteDeclineBtn(inv.id.clone()));
            commands.entity(row).add_children(&[ic, label, accept, decline]);
            row
        }),
    }
}

fn teams_snapshot(w: &World) -> KeyedSnapshot {
    let Some(panel) = w.get_resource::<TeamsPanel>() else {
        return util::empty_snapshot();
    };
    if panel.teams.is_empty() {
        let msg = if panel.loading { "Loading..." } else { "No teams yet — create one below" };
        return KeyedSnapshot {
            items: vec![(u64::MAX, hash64(msg))],
            build: Box::new(move |commands, fonts, _| {
                commands
                    .spawn((Text::new(msg), ui_font(&fonts.ui, 11.0), TextColor(rgb(placeholder())), Node { margin: UiRect::top(Val::Px(6.0)), ..default() }))
                    .id()
            }),
        };
    }
    let teams = panel.teams.clone();
    let keys = teams.iter().map(|t| (hash64(&t.id), hash64(&(&t.name, &t.description)))).collect();
    KeyedSnapshot {
        items: keys,
        build: Box::new(move |commands, fonts, i| {
            let t = &teams[i];
            let row = row_base(commands);
            let ic = icon_badge(commands, fonts, HUE_TEAMS, "users-three", 28.0);
            let col = commands
                .spawn(Node { flex_direction: FlexDirection::Column, flex_grow: 1.0, ..default() })
                .id();
            let name = commands
                .spawn((Text::new(t.name.clone()), ui_font(&fonts.ui, 12.0), TextColor(rgb(text_primary()))))
                .id();
            commands.entity(col).add_child(name);
            if !t.description.is_empty() {
                let d = commands
                    .spawn((Text::new(t.description.clone()), ui_font(&fonts.ui, 10.0), TextColor(rgb(text_muted()))))
                    .id();
                commands.entity(col).add_child(d);
            }
            let chat = accent_ghost(commands, fonts, HUE_CHAT, "Team chat");
            commands.entity(chat).insert(TeamChatBtn(t.id.clone()));
            let open = accent_button(commands, fonts, HUE_TEAMS, "Open");
            commands.entity(open).insert(TeamOpenBtn(t.id.clone()));
            commands.entity(row).add_children(&[ic, col, chat, open]);
            row
        }),
    }
}

fn detail_snapshot(w: &World) -> KeyedSnapshot {
    let Some(panel) = w.get_resource::<TeamsPanel>() else {
        return util::empty_snapshot();
    };
    let Some(detail) = panel.detail.clone() else {
        return KeyedSnapshot {
            items: vec![(u64::MAX, 1)],
            build: Box::new(|commands, fonts, _| {
                commands
                    .spawn((Text::new("Loading..."), ui_font(&fonts.ui, 11.0), TextColor(rgb(placeholder()))))
                    .id()
            }),
        };
    };
    let my_id = w
        .get_resource::<AuthSession>()
        .and_then(|s| s.user.as_ref().map(|u| u.id.clone()))
        .unwrap_or_default();
    let key = hash64(&(
        &detail.team.id,
        detail.members.len(),
        detail.invites.len(),
    ));
    KeyedSnapshot {
        items: vec![(hash64(&detail.team.id), key)],
        build: Box::new(move |commands, fonts, _| detail_block(commands, fonts, &detail, &my_id)),
    }
}

fn detail_block(commands: &mut Commands, fonts: &EmberFonts, detail: &TeamDetail, my_id: &str) -> Entity {
    let wrap = commands
        .spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(6.0), ..default() })
        .id();

    // Actions: open chat.
    let actions = commands
        .spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Row, column_gap: Val::Px(6.0), ..default() })
        .id();
    let chat = accent_button(commands, fonts, HUE_CHAT, "Open team chat");
    commands.entity(chat).insert(TeamChatBtn(detail.team.id.clone()));
    commands.entity(actions).add_child(chat);

    // Members.
    let members_title = commands
        .spawn((Text::new(format!("Members ({})", detail.members.len())), ui_font(&fonts.ui, 11.5), TextColor(rgb(text_muted()))))
        .id();
    let members_col = commands
        .spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(2.0), ..default() })
        .id();
    let is_owner = detail.team.owner_id == my_id;
    for m in &detail.members {
        let row = row_base(commands);
        let ic = icon_text(commands, &fonts.phosphor, "user", text_muted(), 13.0);
        let name = commands
            .spawn((Text::new(m.username.clone()), ui_font(&fonts.ui, 11.5), TextColor(rgb(text_primary())), Node { flex_grow: 1.0, ..default() }))
            .id();
        let role = accent_chip(
            commands,
            fonts,
            if m.role == "owner" { HUE_TEAMS } else { (150, 150, 162) },
            if m.role == "owner" { Some("crown-simple") } else { None },
            &m.role,
        );
        let mut kids = vec![ic, name, role];
        // Owners can remove others; anyone can remove themselves (leave).
        if m.user_id != detail.team.owner_id && (is_owner || m.user_id == my_id) {
            let label = if m.user_id == my_id { "Leave" } else { "Remove" };
            let rm = accent_ghost(commands, fonts, RED, label);
            commands.entity(rm).insert(MemberRemoveBtn {
                team_id: detail.team.id.clone(),
                user_id: m.user_id.clone(),
            });
            kids.push(rm);
        }
        commands.entity(row).add_children(&kids);
        commands.entity(members_col).add_child(row);
    }

    // Invite row.
    let invite_row = commands
        .spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Row, column_gap: Val::Px(6.0), align_items: AlignItems::Center, ..default() })
        .id();
    let invite_input = text_input(commands, &fonts.ui, "Invite by username or email...", "");
    commands.entity(invite_input).insert((TeamInviteInput, Node { flex_grow: 1.0, ..default() }));
    let invite_btn = accent_button(commands, fonts, HUE_TEAMS, "Invite");
    commands.entity(invite_btn).insert(TeamInviteSendBtn(detail.team.id.clone()));
    // Enter in the invite field presses Invite.
    commands.entity(invite_row).insert(EmberForm { submit: invite_btn });
    commands.entity(invite_row).add_children(&[invite_input, invite_btn]);

    // Pending invites.
    let mut kids = vec![actions, members_title, members_col, invite_row];
    let pending: Vec<_> = detail.invites.iter().filter(|i| i.status == "pending").collect();
    if !pending.is_empty() {
        let t = commands
            .spawn((Text::new(format!("Pending invites ({})", pending.len())), ui_font(&fonts.ui, 11.0), TextColor(rgb(text_muted()))))
            .id();
        kids.push(t);
        for inv in pending {
            let who = inv
                .invited_email
                .clone()
                .or(inv.invited_user_id.clone())
                .unwrap_or_default();
            let r = commands
                .spawn((Text::new(format!("• {who} ({})", inv.role)), ui_font(&fonts.ui, 10.5), TextColor(rgb(placeholder()))))
                .id();
            kids.push(r);
        }
    }

    commands.entity(wrap).add_children(&kids);
    wrap
}

fn row_base(commands: &mut Commands) -> Entity {
    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(8.0),
                padding: UiRect::all(Val::Px(8.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(7.0)),
                ..default()
            },
            BackgroundColor(rgb(card_bg())),
            BorderColor::all(rgba([255, 255, 255, 10])),
        ))
        .id()
}
