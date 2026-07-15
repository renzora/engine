//! Messages panel — conversation list (DMs, groups, team chats) on the left,
//! the active conversation on the right with history pagination, optimistic
//! send, and live updates from the WebSocket.

use bevy::prelude::*;
use crossbeam_channel::{unbounded, Receiver, Sender};
use renzora::core::{RenzoraShellExt, SocialBridge, SocialPanelRequest};
use renzora::SplashState;
use renzora_auth::messages::{ConversationPreview, MessageRow, MessageSearchHit};
use renzora_auth::AuthSession;
use renzora_ember::dock::panel_active;
use renzora_ember::font::{ui_font, EmberFonts};
use renzora_ember::panel::RegisterPanelContent;
use renzora_ember::reactive::{bind_display, bind_text, keyed_list_tokened, KeyedSnapshot};
use renzora_ember::theme::*;
use renzora_ember::theme::{popup_bg, row_even, row_odd};
use renzora_ember::widgets::{
    accent_button, accent_ghost, accent_icon_button, empty_state, icon_badge, scroll_view_pinned,
    text_input, tint, EmberForm, EmberTextInput, HoverTint, HoverTooltip,
};

use crate::avatars::avatar_image;
use crate::toasts::{ToastQueue, Tone};
use crate::util::{self, hash64, session_clone, HUE_CHAT};
use crate::PendingSocialRequest;

pub(crate) const PANEL_ID: &str = "social_chat";

const PAGE_SIZE: u32 = 50;

pub(crate) enum ChatResult {
    Conversations(Result<Vec<ConversationPreview>, String>),
    /// (conversation_id, prepend) — history pages prepend, initial loads replace.
    Messages(String, bool, Result<Vec<MessageRow>, String>),
    /// Optimistic-send reconciliation: (temp_id, result).
    Sent(String, Result<renzora_auth::messages::SentMessage, String>),
    Participants(String, Result<Vec<renzora_auth::messages::Participant>, String>),
    SearchHits(Result<Vec<MessageSearchHit>, String>),
    /// Group created → open it.
    GroupCreated(Result<String, String>),
    ActionDone,
}

#[derive(Resource)]
pub(crate) struct ChatPanel {
    pub conversations: Vec<ConversationPreview>,
    pub active: Option<String>,
    /// Messages of the active conversation, oldest → newest.
    pub messages: Vec<MessageRow>,
    /// True when the top of history has been reached.
    pub at_history_end: bool,
    pub history_loading: bool,
    pub my_id: Option<String>,
    /// user_id -> avatar_url for the active conversation (fills in avatars on
    /// live WebSocket messages, which don't carry one).
    pub participants: std::collections::HashMap<String, Option<String>>,
    pub loading: bool,
    pub error: Option<String>,
    pub version: u64,
    pub loaded_once: bool,
    // Message search.
    pub search_hits: Vec<MessageSearchHit>,
    pub last_query: String,
    pub pending_query: Option<(String, f64)>,
    // "New group" overlay state.
    pub group_open: bool,
    pub group_selected: std::collections::HashSet<String>,
    temp_counter: u64,
    pub tx: Sender<ChatResult>,
    rx: Receiver<ChatResult>,
}

impl Default for ChatPanel {
    fn default() -> Self {
        let (tx, rx) = unbounded();
        Self {
            conversations: Vec::new(),
            active: None,
            messages: Vec::new(),
            at_history_end: false,
            history_loading: false,
            my_id: None,
            participants: std::collections::HashMap::new(),
            loading: false,
            error: None,
            version: 0,
            loaded_once: false,
            search_hits: Vec::new(),
            last_query: String::new(),
            pending_query: None,
            group_open: false,
            group_selected: std::collections::HashSet::new(),
            temp_counter: 0,
            tx,
            rx,
        }
    }
}

impl ChatPanel {
    pub fn bump(&mut self) {
        self.version = self.version.wrapping_add(1);
    }

    /// Total unread across conversations (for the status-bar badge).
    pub fn total_unread(&self) -> u32 {
        self.conversations.iter().map(|c| c.unread_count.max(0) as u32).sum()
    }

    /// Live incoming message from the WebSocket.
    #[allow(clippy::too_many_arguments)]
    pub fn apply_incoming(
        &mut self,
        conversation_id: &str,
        message_id: String,
        sender_id: String,
        sender_username: String,
        body: String,
        reply_to_id: Option<String>,
        created_at: String,
    ) {
        let own = self.my_id.as_deref() == Some(sender_id.as_str());
        let viewing = self.active.as_deref() == Some(conversation_id);

        // Update the conversation preview + ordering.
        if let Some(pos) = self.conversations.iter().position(|c| c.id == conversation_id) {
            let mut c = self.conversations.remove(pos);
            c.last_message_body = Some(body.clone());
            c.last_message_sender = Some(sender_username.clone());
            c.last_message_at = Some(created_at.clone());
            if !own && !viewing {
                c.unread_count += 1;
            }
            self.conversations.insert(0, c);
        }

        if viewing {
            // Reconcile the WS echo of our own optimistic send.
            if own {
                if let Some(m) = self
                    .messages
                    .iter_mut()
                    .find(|m| m.id.starts_with("temp-") && m.body == body)
                {
                    m.id = message_id.clone();
                    m.created_at = created_at.clone();
                    self.bump();
                    return;
                }
            }
            if !self.messages.iter().any(|m| m.id == message_id) {
                let avatar = self.participants.get(&sender_id).cloned().flatten();
                self.messages.push(MessageRow {
                    id: message_id,
                    conversation_id: conversation_id.to_string(),
                    sender_id,
                    sender_username,
                    sender_avatar_url: avatar,
                    body,
                    reply_to_id,
                    edited_at: None,
                    deleted: false,
                    created_at,
                });
            }
        }
        self.bump();
    }

    pub fn apply_edit(&mut self, conversation_id: &str, message_id: &str, body: String) {
        if self.active.as_deref() == Some(conversation_id) {
            if let Some(m) = self.messages.iter_mut().find(|m| m.id == message_id) {
                m.body = body;
                m.edited_at = Some("edited".to_string());
                self.bump();
            }
        }
    }

    pub fn apply_delete(&mut self, conversation_id: &str, message_id: &str) {
        if self.active.as_deref() == Some(conversation_id) {
            if let Some(m) = self.messages.iter_mut().find(|m| m.id == message_id) {
                m.deleted = true;
                m.body = String::new();
                self.bump();
            }
        }
    }

    /// Open a conversation: reset the thread state and fetch its history.
    pub fn open_conversation(&mut self, session: &AuthSession, conversation_id: String) {
        self.active = Some(conversation_id.clone());
        self.messages.clear();
        self.at_history_end = false;
        self.history_loading = false;
        // Zero the local unread for it.
        if let Some(c) = self.conversations.iter_mut().find(|c| c.id == conversation_id) {
            c.unread_count = 0;
        }
        self.bump();
        let tx = self.tx.clone();
        let session = session_clone(session);
        spawn_thread(move || {
            let result = renzora_auth::messages::list_messages(&session, &conversation_id, None, PAGE_SIZE);
            let _ = tx.send(ChatResult::Messages(conversation_id.clone(), false, result));
            let participants = renzora_auth::messages::list_participants(&session, &conversation_id);
            let _ = tx.send(ChatResult::Participants(conversation_id.clone(), participants));
            let _ = renzora_auth::messages::mark_read(&session, &conversation_id);
            let _ = tx.send(ChatResult::ActionDone);
        });
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn spawn_thread(f: impl FnOnce() + Send + 'static) {
    std::thread::spawn(f);
}
#[cfg(target_arch = "wasm32")]
fn spawn_thread(_f: impl FnOnce() + Send + 'static) {}

pub(crate) fn register(app: &mut App) {
    app.init_resource::<ChatPanel>();
    app.init_resource::<MsgContextMenu>();
    app.register_shell_panel(PANEL_ID, "Messages", "chats", "Community");
    app.register_panel_content(PANEL_ID, false, build);
    app.add_systems(
        Update,
        (
            poll_results,
            auto_refresh.run_if(panel_active(PANEL_ID)),
            clicks,
            group_overlay_clicks,
            search_debounce.run_if(panel_active(PANEL_ID)),
            right_click_menu,
            context_menu_clicks,
            consume_request,
        )
            .run_if(in_state(SplashState::Editor)),
    );
}

/// Debounced message search; empty query clears results.
fn search_debounce(
    mut panel: ResMut<ChatPanel>,
    session: Res<AuthSession>,
    time: Res<Time>,
    inputs: Query<&EmberTextInput, With<ChatSearchInput>>,
) {
    let Ok(input) = inputs.single() else { return };
    let query = input.value.trim().to_string();
    let now = time.elapsed_secs_f64();
    if query != panel.last_query {
        panel.last_query = query.clone();
        if query.len() >= 2 {
            panel.pending_query = Some((query, now + 0.4));
        } else {
            panel.pending_query = None;
            if !panel.search_hits.is_empty() {
                panel.search_hits.clear();
                panel.bump();
            }
        }
        return;
    }
    if let Some((q, deadline)) = panel.pending_query.clone() {
        if now >= deadline {
            panel.pending_query = None;
            let tx = panel.tx.clone();
            let s = session_clone(&session);
            spawn_thread(move || {
                let _ = tx.send(ChatResult::SearchHits(renzora_auth::messages::search_messages(&s, &q)));
            });
        }
    }
}

/// Right-click on a message → context menu (view profile / delete own).
fn right_click_menu(
    mut commands: Commands,
    fonts: Option<Res<EmberFonts>>,
    mouse: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    mut menu: ResMut<MsgContextMenu>,
    rows: Query<(&Interaction, &MsgRowCtx)>,
) {
    if !mouse.just_pressed(MouseButton::Right) {
        return;
    }
    let Some(fonts) = fonts else { return };
    // Close any existing menu first. `try_despawn`: the stored root can be
    // stale if a workspace/layout rebuild tore the backdrop down already.
    if let Some(root) = menu.0.take() {
        commands.entity(root).try_despawn();
    }
    let Some((_, ctx)) = rows
        .iter()
        .find(|(i, _)| matches!(i, Interaction::Hovered | Interaction::Pressed))
    else {
        return;
    };
    let cursor = windows
        .iter()
        .next()
        .and_then(|w| w.cursor_position())
        .unwrap_or(Vec2::new(200.0, 200.0));

    let backdrop = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                ..default()
            },
            BackgroundColor(Color::NONE),
            Interaction::default(),
            renzora_ember::cursor_icon::NoAutoCursor,
            CtxBackdrop,
            GlobalZIndex(955),
            Name::new("msg_context_menu"),
        ))
        .id();
    let menu_panel = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(cursor.x.max(4.0)),
                top: Val::Px(cursor.y.max(4.0)),
                width: Val::Px(170.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(4.0)),
                row_gap: Val::Px(2.0),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                ..default()
            },
            BackgroundColor(rgb(popup_bg())),
            BorderColor::all(tint(HUE_CHAT, 60)),
            bevy::ui::FocusPolicy::Block,
        ))
        .id();
    let view = accent_ghost(&mut commands, &fonts, HUE_CHAT, "View profile");
    commands.entity(view).insert((CtxViewProfileBtn(ctx.sender_username.clone()), Node { width: Val::Percent(100.0), padding: UiRect::axes(Val::Px(8.0), Val::Px(5.0)), border: UiRect::all(Val::Px(0.0)), border_radius: BorderRadius::all(Val::Px(4.0)), ..default() }));
    commands.entity(menu_panel).add_child(view);
    if ctx.own && !ctx.message_id.starts_with("temp-") {
        let del = accent_ghost(&mut commands, &fonts, (224, 80, 80), "Delete message");
        commands.entity(del).insert((CtxDeleteBtn(ctx.message_id.clone()), Node { width: Val::Percent(100.0), padding: UiRect::axes(Val::Px(8.0), Val::Px(5.0)), border: UiRect::all(Val::Px(0.0)), border_radius: BorderRadius::all(Val::Px(4.0)), ..default() }));
        commands.entity(menu_panel).add_child(del);
    }
    commands.entity(backdrop).add_child(menu_panel);
    menu.0 = Some(backdrop);
}

#[allow(clippy::too_many_arguments)]
fn context_menu_clicks(
    mut commands: Commands,
    mut menu: ResMut<MsgContextMenu>,
    mut panel: ResMut<ChatPanel>,
    mut bridge: ResMut<SocialBridge>,
    session: Res<AuthSession>,
    views: Query<(&Interaction, &CtxViewProfileBtn), Changed<Interaction>>,
    deletes: Query<(&Interaction, &CtxDeleteBtn), Changed<Interaction>>,
    backdrops: Query<&Interaction, (With<CtxBackdrop>, Changed<Interaction>)>,
) {
    let mut close = false;
    for (i, b) in &views {
        if *i == Interaction::Pressed {
            bridge.open_panel_request = Some(SocialPanelRequest::Profile { username: Some(b.0.clone()) });
            close = true;
        }
    }
    for (i, b) in &deletes {
        if *i == Interaction::Pressed {
            if let Some(conv) = panel.active.clone() {
                // Optimistic local delete.
                let id = b.0.clone();
                panel.apply_delete(&conv, &id);
                let s = session_clone(&session);
                spawn_thread(move || {
                    let _ = renzora_auth::messages::delete_message(&s, &conv, &id);
                });
            }
            close = true;
        }
    }
    for i in &backdrops {
        if *i == Interaction::Pressed {
            close = true;
        }
    }
    if close {
        // `try_despawn` for the same stale-root reason as `right_click_menu`.
        if let Some(root) = menu.0.take() {
            commands.entity(root).try_despawn();
        }
    }
}

/// The "new group" overlay: name + friend picks → create.
#[allow(clippy::too_many_arguments)]
fn group_overlay_clicks(
    mut panel: ResMut<ChatPanel>,
    session: Res<AuthSession>,
    opens: Query<&Interaction, (With<NewGroupBtn>, Changed<Interaction>)>,
    picks: Query<(&Interaction, &GroupPickRow), Changed<Interaction>>,
    confirms: Query<&Interaction, (With<GroupCreateConfirmBtn>, Changed<Interaction>)>,
    cancels: Query<&Interaction, (With<GroupCancelBtn>, Changed<Interaction>)>,
    mut names: Query<&mut EmberTextInput, With<GroupNameInput>>,
) {
    let pressed = |i: &Interaction| *i == Interaction::Pressed;
    for i in &opens {
        if pressed(i) {
            panel.group_open = !panel.group_open;
            panel.group_selected.clear();
            panel.bump();
        }
    }
    for (i, row) in &picks {
        if pressed(i) {
            if !panel.group_selected.insert(row.0.clone()) {
                panel.group_selected.remove(&row.0);
            }
            panel.bump();
        }
    }
    for i in &cancels {
        if pressed(i) {
            panel.group_open = false;
            panel.group_selected.clear();
            panel.bump();
        }
    }
    for i in &confirms {
        if pressed(i) {
            let Ok(mut name) = names.single_mut() else { continue };
            let group_name = name.value.trim().to_string();
            if group_name.is_empty() || panel.group_selected.is_empty() {
                continue;
            }
            name.value.clear();
            let members: Vec<String> = panel.group_selected.iter().cloned().collect();
            let tx = panel.tx.clone();
            let s = session_clone(&session);
            spawn_thread(move || {
                let r = renzora_auth::messages::create_group(&s, &group_name, &members)
                    .map(|r| r.conversation_id);
                let _ = tx.send(ChatResult::GroupCreated(r));
            });
        }
    }
}

// ── Fetching ─────────────────────────────────────────────────────────────────

/// One-shot baseline fetch; marks `loaded_once` at SPAWN time so a failure can
/// never auto-retry — live updates arrive over the WebSocket.
fn refresh_conversations(panel: &mut ChatPanel, session: &AuthSession) {
    if !session.is_signed_in() {
        return;
    }
    panel.loaded_once = true;
    panel.loading = true;
    panel.error = None;
    panel.my_id = session.user.as_ref().map(|u| u.id.clone());
    let tx = panel.tx.clone();
    let session = session_clone(session);
    spawn_thread(move || {
        let _ = tx.send(ChatResult::Conversations(renzora_auth::messages::list_conversations(&session)));
    });
}

fn load_older(panel: &mut ChatPanel, session: &AuthSession) {
    let (Some(conv), Some(oldest)) = (panel.active.clone(), panel.messages.first().map(|m| m.id.clone()))
    else {
        return;
    };
    if panel.history_loading || panel.at_history_end || oldest.starts_with("temp-") {
        return;
    }
    panel.history_loading = true;
    let tx = panel.tx.clone();
    let session = session_clone(session);
    spawn_thread(move || {
        let result = renzora_auth::messages::list_messages(&session, &conv, Some(&oldest), PAGE_SIZE);
        let _ = tx.send(ChatResult::Messages(conv, true, result));
    });
}

// ── Systems ──────────────────────────────────────────────────────────────────

fn poll_results(
    mut panel: ResMut<ChatPanel>,
    mut bridge: ResMut<SocialBridge>,
    mut toasts: ResMut<ToastQueue>,
    session: Res<AuthSession>,
) {
    let mut got = Vec::new();
    while let Ok(r) = panel.rx.try_recv() {
        got.push(r);
    }
    for r in got {
        match r {
            ChatResult::Conversations(Ok(list)) => {
                panel.conversations = list;
                panel.loading = false;
                panel.loaded_once = true;
                bridge.unread_messages = panel.total_unread();
                panel.bump();
            }
            ChatResult::Conversations(Err(e)) => {
                panel.loading = false;
                panel.error = Some(e);
                panel.bump();
            }
            ChatResult::Messages(conv, prepend, Ok(mut page)) => {
                if panel.active.as_deref() != Some(conv.as_str()) {
                    continue;
                }
                // Server returns newest-first; display oldest-first.
                page.reverse();
                if page.len() < PAGE_SIZE as usize {
                    panel.at_history_end = true;
                }
                if prepend {
                    panel.history_loading = false;
                    let existing: Vec<MessageRow> = std::mem::take(&mut panel.messages);
                    panel.messages = page;
                    panel.messages.extend(existing);
                } else {
                    panel.messages = page;
                }
                bridge.unread_messages = panel.total_unread();
                panel.bump();
            }
            ChatResult::Messages(_, prepend, Err(e)) => {
                if prepend {
                    panel.history_loading = false;
                }
                panel.error = Some(e);
                panel.bump();
            }
            ChatResult::Sent(temp_id, Ok(sent)) => {
                if let Some(m) = panel.messages.iter_mut().find(|m| m.id == temp_id) {
                    m.id = sent.id;
                    m.created_at = sent.created_at;
                    panel.bump();
                }
            }
            ChatResult::Sent(temp_id, Err(e)) => {
                panel.messages.retain(|m| m.id != temp_id);
                toasts.push(Tone::Error, format!("Message failed: {e}"), None);
                panel.bump();
            }
            ChatResult::Participants(conv, Ok(list)) => {
                if panel.active.as_deref() == Some(conv.as_str()) {
                    panel.participants =
                        list.into_iter().map(|p| (p.user_id, p.avatar_url)).collect();
                    // Backfill avatars on already-loaded rows that lack one.
                    let map = panel.participants.clone();
                    for m in panel.messages.iter_mut() {
                        if m.sender_avatar_url.is_none() {
                            m.sender_avatar_url = map.get(&m.sender_id).cloned().flatten();
                        }
                    }
                    panel.bump();
                }
            }
            ChatResult::Participants(_, Err(_)) => {}
            ChatResult::SearchHits(Ok(hits)) => {
                panel.search_hits = hits;
                panel.bump();
            }
            ChatResult::SearchHits(Err(_)) => {}
            ChatResult::GroupCreated(Ok(conversation_id)) => {
                panel.group_open = false;
                panel.group_selected.clear();
                refresh_conversations(&mut panel, &session);
                panel.open_conversation(&session, conversation_id);
            }
            ChatResult::GroupCreated(Err(e)) => {
                toasts.push(Tone::Error, format!("Couldn't create group: {e}"), None);
            }
            ChatResult::ActionDone => {}
        }
    }
}

/// One-shot baseline fetch on first view; the WebSocket keeps it live and a
/// WS reconnect clears `loaded_once` to resync. No polling.
fn auto_refresh(mut panel: ResMut<ChatPanel>, session: Res<AuthSession>) {
    if session.is_signed_in() && !panel.loaded_once {
        refresh_conversations(&mut panel, &session);
    }
}

/// Deep-link: open a specific conversation.
fn consume_request(
    mut pending: ResMut<PendingSocialRequest>,
    mut panel: ResMut<ChatPanel>,
    session: Res<AuthSession>,
) {
    let Some(SocialPanelRequest::Chat { conversation_id }) = &pending.0 else {
        return;
    };
    let conv = conversation_id.clone();
    pending.0 = None;
    if !panel.loaded_once {
        refresh_conversations(&mut panel, &session);
    }
    if let Some(conv) = conv {
        panel.open_conversation(&session, conv);
    }
}

// ── Clicks / input ───────────────────────────────────────────────────────────

#[derive(Component)]
struct ConvRowBtn(String);
#[derive(Component)]
struct ChatRefreshBtn;
#[derive(Component)]
struct LoadOlderBtn;
#[derive(Component)]
struct ChatInput;
#[derive(Component)]
struct SendBtn;
#[derive(Component)]
struct ChatSearchInput;
#[derive(Component)]
struct SearchHitRow(String);
#[derive(Component)]
struct NewGroupBtn;
#[derive(Component)]
struct GroupPickRow(String);
#[derive(Component)]
struct GroupCreateConfirmBtn;
#[derive(Component)]
struct GroupCancelBtn;
#[derive(Component)]
struct GroupNameInput;
#[derive(Component)]
pub(crate) struct MsgRowCtx {
    pub message_id: String,
    pub sender_username: String,
    pub own: bool,
}
#[derive(Component)]
struct CtxViewProfileBtn(String);
#[derive(Component)]
struct CtxDeleteBtn(String);
#[derive(Component)]
struct CtxBackdrop;

/// Root of the open right-click menu, if any.
#[derive(Resource, Default)]
pub(crate) struct MsgContextMenu(Option<Entity>);

#[allow(clippy::too_many_arguments)]
fn clicks(
    mut panel: ResMut<ChatPanel>,
    session: Res<AuthSession>,
    convs: Query<(&Interaction, &ConvRowBtn), Changed<Interaction>>,
    hits: Query<(&Interaction, &SearchHitRow), Changed<Interaction>>,
    refresh_q: Query<&Interaction, (With<ChatRefreshBtn>, Changed<Interaction>)>,
    older: Query<&Interaction, (With<LoadOlderBtn>, Changed<Interaction>)>,
    send: Query<&Interaction, (With<SendBtn>, Changed<Interaction>)>,
    mut inputs: Query<&mut EmberTextInput, With<ChatInput>>,
) {
    for (i, row) in &convs {
        if *i == Interaction::Pressed && panel.active.as_deref() != Some(row.0.as_str()) {
            panel.open_conversation(&session, row.0.clone());
        }
    }
    for (i, hit) in &hits {
        if *i == Interaction::Pressed {
            panel.open_conversation(&session, hit.0.clone());
        }
    }
    for i in &refresh_q {
        if *i == Interaction::Pressed {
            refresh_conversations(&mut panel, &session);
        }
    }
    for i in &older {
        if *i == Interaction::Pressed {
            load_older(&mut panel, &session);
        }
    }
    for i in &send {
        if *i == Interaction::Pressed {
            if let Ok(mut input) = inputs.single_mut() {
                submit_message(&mut panel, &session, &mut input);
            }
        }
    }
}

fn submit_message(panel: &mut ChatPanel, session: &AuthSession, input: &mut EmberTextInput) {
    let body = input.value.trim().to_string();
    let Some(conv) = panel.active.clone() else {
        input.value.clear();
        return;
    };
    if body.is_empty() {
        input.value.clear();
        return;
    }
    input.value.clear();

    // Optimistic append.
    panel.temp_counter += 1;
    let temp_id = format!("temp-{}", panel.temp_counter);
    let own_avatar = panel
        .my_id
        .as_ref()
        .and_then(|id| panel.participants.get(id).cloned())
        .flatten();
    panel.messages.push(MessageRow {
        id: temp_id.clone(),
        conversation_id: conv.clone(),
        sender_id: panel.my_id.clone().unwrap_or_default(),
        sender_username: session.user.as_ref().map(|u| u.username.clone()).unwrap_or_default(),
        sender_avatar_url: own_avatar,
        body: body.clone(),
        reply_to_id: None,
        edited_at: None,
        deleted: false,
        created_at: String::new(),
    });
    panel.bump();

    let tx = panel.tx.clone();
    let session = session_clone(session);
    spawn_thread(move || {
        let result = renzora_auth::messages::send_message(&session, &conv, &body, None);
        let _ = tx.send(ChatResult::Sent(temp_id, result));
    });
}

// ── Build ────────────────────────────────────────────────────────────────────

fn build(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let root = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            ..default()
        })
        .id();

    let signed_out = empty_state(
        commands,
        fonts,
        HUE_CHAT,
        "chats",
        "Sign in to start talking",
        Some("DMs, group chats, and team rooms live here"),
    );
    bind_display(commands, signed_out, |w| !util::signed_in(w));

    let body = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            ..default()
        })
        .id();
    bind_display(commands, body, util::signed_in);

    // ── Left: conversation list ──
    let left = commands
        .spawn((
            Node {
                width: Val::Px(240.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(4.0),
                padding: UiRect::all(Val::Px(6.0)),
                flex_shrink: 0.0,
                ..default()
            },
            BackgroundColor(rgb(section_bg())),
        ))
        .id();
    let left_head = commands
        .spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(6.0), padding: UiRect::bottom(Val::Px(2.0)), ..default() })
        .id();
    let lh_badge = icon_badge(commands, fonts, HUE_CHAT, "chats", 22.0);
    let lh_title = commands
        .spawn((Text::new("Messages"), ui_font(&fonts.ui, 12.0), TextColor(rgb(text_primary())), Node { flex_grow: 1.0, ..default() }))
        .id();
    let group_btn = accent_icon_button(commands, fonts, HUE_CHAT, "users-three");
    commands.entity(group_btn).insert((NewGroupBtn, HoverTooltip::new("New group chat")));
    let refresh_btn = accent_icon_button(commands, fonts, HUE_CHAT, "arrows-clockwise");
    commands.entity(refresh_btn).insert((ChatRefreshBtn, HoverTooltip::new("Refresh")));
    commands.entity(left_head).add_children(&[lh_badge, lh_title, group_btn, refresh_btn]);

    // Message search.
    let search = text_input(commands, &fonts.ui, "Search messages...", "");
    commands.entity(search).insert((ChatSearchInput, Node { width: Val::Percent(100.0), ..default() }));

    let conv_list = commands
        .spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(2.0), ..default() })
        .id();
    keyed_list_tokened(
        commands,
        conv_list,
        |w| w.get_resource::<ChatPanel>().map(|p| p.version).unwrap_or(0),
        conversations_snapshot,
    );
    let conv_scroll = renzora_ember::widgets::scroll_view(commands, conv_list);
    commands.entity(conv_scroll).insert(Node {
        width: Val::Percent(100.0),
        flex_grow: 1.0,
        min_height: Val::Px(0.0),
        overflow: Overflow::scroll_y(),
        ..default()
    });
    // "New group" overlay (inline panel under the header while open).
    let group_panel = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(5.0),
                padding: UiRect::all(Val::Px(7.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(7.0)),
                ..default()
            },
            BackgroundColor(rgb(popup_bg())),
            BorderColor::all(tint(HUE_CHAT, 60)),
        ))
        .id();
    bind_display(commands, group_panel, |w| {
        w.get_resource::<ChatPanel>().map(|p| p.group_open).unwrap_or(false)
    });
    let g_name = text_input(commands, &fonts.ui, "Group name...", "");
    commands.entity(g_name).insert((GroupNameInput, Node { width: Val::Percent(100.0), ..default() }));
    let g_list = commands
        .spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(2.0), ..default() })
        .id();
    keyed_list_tokened(
        commands,
        g_list,
        |w| {
            let p = w.get_resource::<ChatPanel>();
            let f = w.get_resource::<crate::panels::friends::FriendsPanel>();
            p.map(|p| p.version).unwrap_or(0) ^ f.map(|f| f.version).unwrap_or(0)
        },
        group_pick_snapshot,
    );
    let g_actions = commands
        .spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Row, column_gap: Val::Px(5.0), justify_content: JustifyContent::FlexEnd, ..default() })
        .id();
    let g_cancel = accent_ghost(commands, fonts, (170, 170, 182), "Cancel");
    commands.entity(g_cancel).insert(GroupCancelBtn);
    let g_create = accent_button(commands, fonts, HUE_CHAT, "Create group");
    commands.entity(g_create).insert(GroupCreateConfirmBtn);
    commands.entity(g_actions).add_children(&[g_cancel, g_create]);
    // Enter in the group-name field presses "Create group".
    commands.entity(group_panel).insert(EmberForm { submit: g_create });
    commands.entity(group_panel).add_children(&[g_name, g_list, g_actions]);

    commands.entity(left).add_children(&[left_head, search, group_panel, conv_scroll]);

    // ── Right: active conversation ──
    let right = commands
        .spawn(Node {
            flex_grow: 1.0,
            height: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            padding: UiRect::all(Val::Px(6.0)),
            row_gap: Val::Px(6.0),
            ..default()
        })
        .id();

    // Header: conversation name.
    let head = commands
        .spawn((Text::new(""), ui_font(&fonts.ui, 12.5), TextColor(rgb(text_primary()))))
        .id();
    bind_text(commands, head, |w| {
        let Some(p) = w.get_resource::<ChatPanel>() else { return String::new() };
        match (&p.active, p.conversations.iter().find(|c| Some(&c.id) == p.active.as_ref())) {
            (Some(_), Some(c)) => c.name.clone().unwrap_or_else(|| "Conversation".to_string()),
            (Some(_), None) => "Conversation".to_string(),
            (None, _) => "Select a conversation".to_string(),
        }
    });

    // "Load older" pill at the top of the thread.
    let older = renzora_ember::widgets::accent_ghost(commands, fonts, HUE_CHAT, "Load older messages");
    commands.entity(older).insert(LoadOlderBtn);
    bind_display(commands, older, |w| {
        w.get_resource::<ChatPanel>()
            .map(|p| p.active.is_some() && !p.at_history_end && !p.messages.is_empty())
            .unwrap_or(false)
    });

    // Message list (bottom-pinned scroll).
    let msg_list = commands
        .spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(4.0), ..default() })
        .id();
    keyed_list_tokened(
        commands,
        msg_list,
        |w| w.get_resource::<ChatPanel>().map(|p| p.version).unwrap_or(0),
        messages_snapshot,
    );
    let msg_scroll = scroll_view_pinned(commands, msg_list);
    commands.entity(msg_scroll).insert(Node {
        width: Val::Percent(100.0),
        flex_grow: 1.0,
        min_height: Val::Px(0.0),
        overflow: Overflow::scroll_y(),
        ..default()
    });

    // Composer.
    let composer = commands
        .spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Row, column_gap: Val::Px(6.0), align_items: AlignItems::Center, ..default() })
        .id();
    let input = text_input(commands, &fonts.ui, "Say something...", "");
    commands.entity(input).insert((ChatInput, Node { flex_grow: 1.0, ..default() }));
    let send = accent_button(commands, fonts, HUE_CHAT, "Send");
    commands.entity(send).insert(SendBtn);
    // Enter in the input presses Send (handled by `clicks` like a real click).
    commands.entity(composer).insert(EmberForm { submit: send });
    commands.entity(composer).add_children(&[input, send]);
    bind_display(commands, composer, |w| {
        w.get_resource::<ChatPanel>().map(|p| p.active.is_some()).unwrap_or(false)
    });

    commands.entity(right).add_children(&[head, older, msg_scroll, composer]);
    commands.entity(body).add_children(&[left, right]);
    commands.entity(root).add_children(&[signed_out, body]);
    root
}

// ── Snapshots / rows ─────────────────────────────────────────────────────────

fn conversations_snapshot(w: &World) -> KeyedSnapshot {
    let Some(panel) = w.get_resource::<ChatPanel>() else {
        return util::empty_snapshot();
    };
    // Search mode: show matching messages instead of the conversation list.
    if panel.last_query.len() >= 2 {
        let hits = panel.search_hits.clone();
        if hits.is_empty() {
            return KeyedSnapshot {
                items: vec![(u64::MAX, 3)],
                build: Box::new(|commands, fonts, _| {
                    commands
                        .spawn((Text::new("No messages found"), ui_font(&fonts.ui, 10.0), TextColor(rgb(placeholder())), Node { margin: UiRect::all(Val::Px(6.0)), ..default() }))
                        .id()
                }),
            };
        }
        let keys = hits
            .iter()
            .enumerate()
            .map(|(i, h)| (hash64(&h.message_id), i as u64))
            .collect();
        return KeyedSnapshot {
            items: keys,
            build: Box::new(move |commands, fonts, i| {
                let h = &hits[i];
                let base = rgb(if i % 2 == 0 { row_even() } else { row_odd() });
                let row = commands
                    .spawn((
                        Node {
                            width: Val::Percent(100.0),
                            flex_direction: FlexDirection::Column,
                            row_gap: Val::Px(1.0),
                            padding: UiRect::all(Val::Px(6.0)),
                            border_radius: BorderRadius::all(Val::Px(5.0)),
                            ..default()
                        },
                        BackgroundColor(base),
                        Interaction::default(),
                        HoverTint::solid(base, rgb(hover_bg()), tint(HUE_CHAT, 50)),
                        SearchHitRow(h.conversation_id.clone()),
                    ))
                    .id();
                let head = commands
                    .spawn((
                        Text::new(format!(
                            "{} · {}",
                            h.conversation_name.clone().unwrap_or_else(|| h.sender_username.clone()),
                            util::relative_time(&h.created_at)
                        )),
                        ui_font(&fonts.ui, 9.0),
                        TextColor(rgb(text_muted())),
                    ))
                    .id();
                let body = commands
                    .spawn((
                        Text::new(h.body.chars().take(60).collect::<String>()),
                        ui_font(&fonts.ui, 10.5),
                        TextColor(rgb(text_primary())),
                        bevy::text::TextLayout::no_wrap(),
                    ))
                    .id();
                commands.entity(row).add_children(&[head, body]);
                row
            }),
        };
    }
    if panel.conversations.is_empty() {
        let msg = if panel.loading { "Loading..." } else { "No conversations yet" };
        return KeyedSnapshot {
            items: vec![(u64::MAX, hash64(msg))],
            build: Box::new(move |commands, fonts, _| {
                commands
                    .spawn((Text::new(msg), ui_font(&fonts.ui, 10.5), TextColor(rgb(placeholder())), Node { margin: UiRect::top(Val::Px(6.0)), ..default() }))
                    .id()
            }),
        };
    }
    let convs = panel.conversations.clone();
    let active = panel.active.clone();
    let keys = convs
        .iter()
        .map(|c| {
            (
                hash64(&c.id),
                hash64(&(
                    &c.name,
                    &c.last_message_body,
                    c.unread_count,
                    active.as_deref() == Some(c.id.as_str()),
                )),
            )
        })
        .collect();
    KeyedSnapshot {
        items: keys,
        build: Box::new(move |commands, fonts, i| {
            let c = &convs[i];
            conv_row(commands, fonts, c, active.as_deref() == Some(c.id.as_str()))
        }),
    }
}

fn conv_row(commands: &mut Commands, fonts: &EmberFonts, c: &ConversationPreview, active: bool) -> Entity {
    let base = if active { tint(HUE_CHAT, 46) } else { rgb(card_bg()) };
    let row = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(7.0),
                padding: UiRect::all(Val::Px(6.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                ..default()
            },
            BackgroundColor(base),
            BorderColor::all(if active { tint(HUE_CHAT, 90) } else { rgba([255, 255, 255, 8]) }),
            Interaction::default(),
            HoverTint::solid(base, if active { tint(HUE_CHAT, 60) } else { rgb(hover_bg()) }, tint(HUE_CHAT, 70)),
            ConvRowBtn(c.id.clone()),
        ))
        .id();
    let icon_or_avatar = if c.kind == "team" {
        icon_badge(commands, fonts, crate::util::HUE_TEAMS, "users-three", 22.0)
    } else if c.kind == "group" {
        icon_badge(commands, fonts, HUE_CHAT, "users", 22.0)
    } else {
        avatar_image(commands, fonts, c.avatar_url.as_deref(), 22.0)
    };
    let col = commands
        .spawn(Node { flex_direction: FlexDirection::Column, flex_grow: 1.0, overflow: Overflow::clip(), ..default() })
        .id();
    let name = commands
        .spawn((
            Text::new(c.name.clone().unwrap_or_else(|| "Conversation".to_string())),
            ui_font(&fonts.ui, 11.5),
            TextColor(rgb(text_primary())),
            bevy::text::TextLayout::no_wrap(),
        ))
        .id();
    commands.entity(col).add_child(name);
    if let Some(preview) = &c.last_message_body {
        let p = commands
            .spawn((
                Text::new(preview.chars().take(28).collect::<String>()),
                ui_font(&fonts.ui, 9.5),
                TextColor(rgb(text_muted())),
                bevy::text::TextLayout::no_wrap(),
            ))
            .id();
        commands.entity(col).add_child(p);
    }
    let mut kids = vec![icon_or_avatar, col];
    if c.unread_count > 0 && !active {
        // Unread count in a glowing chat-blue pill.
        let b = commands
            .spawn((
                Node {
                    padding: UiRect::axes(Val::Px(6.0), Val::Px(1.0)),
                    border_radius: BorderRadius::all(Val::Px(8.0)),
                    flex_shrink: 0.0,
                    ..default()
                },
                BackgroundColor(tint(HUE_CHAT, 220)),
            ))
            .id();
        let n = commands
            .spawn((Text::new(c.unread_count.to_string()), ui_font(&fonts.ui, 9.0), TextColor(rgb((255, 255, 255)))))
            .id();
        commands.entity(b).add_child(n);
        kids.push(b);
    }
    commands.entity(row).add_children(&kids);
    row
}

fn group_pick_snapshot(w: &World) -> KeyedSnapshot {
    let Some(chat) = w.get_resource::<ChatPanel>() else {
        return util::empty_snapshot();
    };
    if !chat.group_open {
        return util::empty_snapshot();
    }
    let Some(friends) = w.get_resource::<crate::panels::friends::FriendsPanel>() else {
        return util::empty_snapshot();
    };
    let list = friends.friends.clone();
    if list.is_empty() {
        return KeyedSnapshot {
            items: vec![(u64::MAX, 5)],
            build: Box::new(|commands, fonts, _| {
                commands
                    .spawn((Text::new("Add some friends first"), ui_font(&fonts.ui, 9.5), TextColor(rgb(placeholder()))))
                    .id()
            }),
        };
    }
    let selected = chat.group_selected.clone();
    let keys = list
        .iter()
        .map(|f| (hash64(&f.user_id), selected.contains(&f.user_id) as u64))
        .collect();
    KeyedSnapshot {
        items: keys,
        build: Box::new(move |commands, fonts, i| {
            let f = &list[i];
            let picked = selected.contains(&f.user_id);
            let base = if picked { tint(HUE_CHAT, 60) } else { Color::NONE };
            let row = commands
                .spawn((
                    Node {
                        width: Val::Percent(100.0),
                        flex_direction: FlexDirection::Row,
                        align_items: AlignItems::Center,
                        column_gap: Val::Px(6.0),
                        padding: UiRect::axes(Val::Px(6.0), Val::Px(3.0)),
                        border_radius: BorderRadius::all(Val::Px(4.0)),
                        ..default()
                    },
                    BackgroundColor(base),
                    Interaction::default(),
                    HoverTint::solid(base, tint(HUE_CHAT, 40), tint(HUE_CHAT, 80)),
                    GroupPickRow(f.user_id.clone()),
                ))
                .id();
            let av = avatar_image(commands, fonts, f.avatar_url.as_deref(), 18.0);
            let name = commands
                .spawn((Text::new(f.username.clone()), ui_font(&fonts.ui, 10.5), TextColor(rgb(text_primary())), Node { flex_grow: 1.0, ..default() }))
                .id();
            let mut kids = vec![av, name];
            if picked {
                kids.push(renzora_ember::font::icon_text(commands, &fonts.phosphor, "check", HUE_CHAT, 11.0));
            }
            commands.entity(row).add_children(&kids);
            row
        }),
    }
}

fn messages_snapshot(w: &World) -> KeyedSnapshot {
    let Some(panel) = w.get_resource::<ChatPanel>() else {
        return util::empty_snapshot();
    };
    if panel.active.is_none() {
        return KeyedSnapshot {
            items: vec![(u64::MAX, 0)],
            build: Box::new(|commands, fonts, _| {
                empty_state(
                    commands,
                    fonts,
                    HUE_CHAT,
                    "chat-circle-dots",
                    "Pick a conversation",
                    Some("Or message a friend from the Friends panel"),
                )
            }),
        };
    }
    let messages = panel.messages.clone();
    let my_id = panel.my_id.clone();
    let keys = messages
        .iter()
        .map(|m| (hash64(&m.id), hash64(&(&m.body, m.deleted, &m.edited_at))))
        .collect();
    KeyedSnapshot {
        items: keys,
        build: Box::new(move |commands, fonts, i| {
            let m = &messages[i];
            message_row(commands, fonts, m, my_id.as_deref() == Some(m.sender_id.as_str()))
        }),
    }
}

fn message_row(commands: &mut Commands, fonts: &EmberFonts, m: &MessageRow, own: bool) -> Entity {
    let row = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(8.0),
                align_items: AlignItems::FlexStart,
                justify_content: if own { JustifyContent::FlexEnd } else { JustifyContent::FlexStart },
                padding: UiRect::axes(Val::Px(4.0), Val::Px(2.0)),
                ..default()
            },
            Interaction::default(),
            renzora_ember::cursor_icon::NoAutoCursor,
            MsgRowCtx {
                message_id: m.id.clone(),
                sender_username: m.sender_username.clone(),
                own,
            },
        ))
        .id();
    let av = avatar_image(commands, fonts, m.sender_avatar_url.as_deref(), 24.0);
    let col = commands
        .spawn(Node {
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(1.0),
            align_items: if own { AlignItems::FlexEnd } else { AlignItems::FlexStart },
            max_width: Val::Percent(78.0),
            ..default()
        })
        .id();
    let head = commands
        .spawn(Node { flex_direction: FlexDirection::Row, column_gap: Val::Px(6.0), align_items: AlignItems::Baseline, ..default() })
        .id();
    let name = commands
        .spawn((
            Text::new(m.sender_username.clone()),
            ui_font(&fonts.ui, 11.0),
            TextColor(if own { tint(HUE_CHAT, 255) } else { rgb(text_primary()) }),
        ))
        .id();
    let mut head_kids = vec![name];
    let when = if m.id.starts_with("temp-") {
        "sending...".to_string()
    } else {
        util::relative_time(&m.created_at)
    };
    if !when.is_empty() {
        head_kids.push(
            commands
                .spawn((Text::new(when), ui_font(&fonts.ui, 9.0), TextColor(rgb(placeholder()))))
                .id(),
        );
    }
    if m.edited_at.is_some() && !m.deleted {
        head_kids.push(
            commands
                .spawn((Text::new("(edited)"), ui_font(&fonts.ui, 9.0), TextColor(rgb(placeholder()))))
                .id(),
        );
    }
    commands.entity(head).add_children(&head_kids);
    let body_text = if m.deleted { "message deleted".to_string() } else { m.body.clone() };
    // Own messages sit in a soft chat-blue bubble; everyone else's are plain.
    let bubble = commands
        .spawn((
            Node {
                padding: UiRect::axes(Val::Px(if own { 9.0 } else { 0.0 }), Val::Px(if own { 5.0 } else { 0.0 })),
                border_radius: BorderRadius::all(Val::Px(8.0)),
                align_self: if own { AlignSelf::FlexEnd } else { AlignSelf::FlexStart },
                ..default()
            },
            BackgroundColor(if own { tint(HUE_CHAT, 64) } else { Color::NONE }),
        ))
        .id();
    let body = commands
        .spawn((
            Text::new(body_text),
            ui_font(&fonts.ui, 11.5),
            TextColor(rgb(if m.deleted { placeholder() } else { value_text() })),
        ))
        .id();
    commands.entity(bubble).add_child(body);
    commands.entity(col).add_children(&[head, bubble]);
    if own {
        commands.entity(row).add_children(&[col, av]);
    } else {
        commands.entity(row).add_children(&[av, col]);
    }
    row
}
