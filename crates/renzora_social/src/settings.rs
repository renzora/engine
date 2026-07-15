//! "Social & Privacy" settings section — account privacy controls (applied to
//! the renzora.com account via `PUT /api/user/privacy`) and blocked-user
//! management.

use bevy::prelude::*;
use crossbeam_channel::{unbounded, Receiver, Sender};
use renzora::SplashState;
use renzora_auth::social::{BlockedUser, MeResponse, PrivacySettings};
use renzora_auth::AuthSession;
use renzora_ember::font::{ui_font, EmberFonts};
use renzora_ember::reactive::{bind_2way, bind_display, bind_text, keyed_list_tokened, KeyedSnapshot};
use renzora_ember::settings_sections::RegisterSettingsSection;
use renzora_ember::theme::*;
use renzora_ember::widgets::{textarea, toggle_switch, EmberTextInput};

use renzora::core::SocialBridge;

use crate::toasts::{ToastQueue, Tone};
use crate::util::{self, hash64, session_clone};

const MESSAGE_PRIVACY: [&str; 3] = ["everyone", "friends", "nobody"];
const PROFILE_VISIBILITY: [&str; 2] = ["public", "friends_only"];

pub(crate) enum SettingsResult {
    Me(Result<MeResponse, String>),
    Blocked(Result<Vec<BlockedUser>, String>),
    Saved(Result<(), String>),
}

#[derive(Resource)]
pub(crate) struct SocialSettings {
    pub message_privacy: String,
    pub online_status_visible: bool,
    pub profile_visibility: String,
    pub blocked: Vec<BlockedUser>,
    pub loaded: bool,
    pub loading: bool,
    pub version: u64,
    pub tx: Sender<SettingsResult>,
    rx: Receiver<SettingsResult>,
}

impl Default for SocialSettings {
    fn default() -> Self {
        let (tx, rx) = unbounded();
        Self {
            message_privacy: "everyone".into(),
            online_status_visible: true,
            profile_visibility: "public".into(),
            blocked: Vec::new(),
            loaded: false,
            loading: false,
            version: 0,
            tx,
            rx,
        }
    }
}

impl SocialSettings {
    fn bump(&mut self) {
        self.version = self.version.wrapping_add(1);
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn spawn_thread(f: impl FnOnce() + Send + 'static) {
    std::thread::spawn(f);
}
#[cfg(target_arch = "wasm32")]
fn spawn_thread(_f: impl FnOnce() + Send + 'static) {}

pub(crate) fn register(app: &mut App) {
    app.init_resource::<SocialSettings>();
    app.register_settings_section("social", "Social & Privacy", "shield-check", build);
    app.add_systems(
        Update,
        (poll_results, auto_load, clicks).run_if(in_state(SplashState::Editor)),
    );
}

fn save(settings: &SocialSettings, session: &AuthSession) {
    let body = PrivacySettings {
        message_privacy: Some(settings.message_privacy.clone()),
        online_status_visible: Some(settings.online_status_visible),
        profile_visibility: Some(settings.profile_visibility.clone()),
    };
    let tx = settings.tx.clone();
    let session = session_clone(session);
    spawn_thread(move || {
        let r = renzora_auth::social::update_privacy(&session, &body).map(|_| ());
        let _ = tx.send(SettingsResult::Saved(r));
    });
}

fn poll_results(mut settings: ResMut<SocialSettings>, mut toasts: ResMut<ToastQueue>) {
    let mut got = Vec::new();
    while let Ok(r) = settings.rx.try_recv() {
        got.push(r);
    }
    for r in got {
        match r {
            SettingsResult::Me(Ok(me)) => {
                if !me.message_privacy.is_empty() {
                    settings.message_privacy = me.message_privacy;
                }
                if !me.profile_visibility.is_empty() {
                    settings.profile_visibility = me.profile_visibility;
                }
                settings.online_status_visible = me.online_status_visible;
                settings.loaded = true;
                settings.loading = false;
                settings.bump();
            }
            SettingsResult::Me(Err(_)) => {
                settings.loading = false;
            }
            SettingsResult::Blocked(Ok(list)) => {
                settings.blocked = list;
                settings.bump();
            }
            SettingsResult::Blocked(Err(_)) => {}
            SettingsResult::Saved(Ok(())) => {
                toasts.push(Tone::Success, "Privacy settings saved", None);
            }
            SettingsResult::Saved(Err(e)) => {
                toasts.push(Tone::Error, format!("Couldn't save: {e}"), None);
            }
        }
    }
}

fn auto_load(mut settings: ResMut<SocialSettings>, session: Res<AuthSession>) {
    if session.is_signed_in() && !settings.loaded && !settings.loading {
        // Marked loaded at SPAWN time so a failure never auto-retries.
        settings.loaded = true;
        settings.loading = true;
        let tx = settings.tx.clone();
        let s = session_clone(&session);
        spawn_thread(move || {
            let _ = tx.send(SettingsResult::Me(renzora_auth::social::get_me(&s)));
            let _ = tx.send(SettingsResult::Blocked(renzora_auth::social::get_blocked(&s)));
        });
    }
    // Re-load after account switches.
    if !session.is_signed_in() && settings.loaded {
        *settings = SocialSettings::default();
    }
}

// ── Clicks ───────────────────────────────────────────────────────────────────

#[derive(Component)]
struct MessagePrivacyBtn;
#[derive(Component)]
struct ProfileVisibilityBtn;
#[derive(Component)]
struct UnblockBtn(String);
#[derive(Component)]
struct SignatureInput;
#[derive(Component)]
struct SignatureSaveBtn;

#[allow(clippy::too_many_arguments)]
fn clicks(
    mut settings: ResMut<SocialSettings>,
    session: Res<AuthSession>,
    msg: Query<&Interaction, (With<MessagePrivacyBtn>, Changed<Interaction>)>,
    vis: Query<&Interaction, (With<ProfileVisibilityBtn>, Changed<Interaction>)>,
    unblocks: Query<(&Interaction, &UnblockBtn), Changed<Interaction>>,
    sig_saves: Query<&Interaction, (With<SignatureSaveBtn>, Changed<Interaction>)>,
    sig_inputs: Query<&EmberTextInput, With<SignatureInput>>,
) {
    let pressed = |i: &Interaction| *i == Interaction::Pressed;
    let mut changed = false;

    for i in &msg {
        if pressed(i) {
            let idx = MESSAGE_PRIVACY.iter().position(|v| *v == settings.message_privacy).unwrap_or(0);
            settings.message_privacy = MESSAGE_PRIVACY[(idx + 1) % MESSAGE_PRIVACY.len()].to_string();
            changed = true;
        }
    }
    for i in &vis {
        if pressed(i) {
            let idx = PROFILE_VISIBILITY.iter().position(|v| *v == settings.profile_visibility).unwrap_or(0);
            settings.profile_visibility = PROFILE_VISIBILITY[(idx + 1) % PROFILE_VISIBILITY.len()].to_string();
            changed = true;
        }
    }
    if changed {
        settings.bump();
        save(&settings, &session);
    }
    for i in &sig_saves {
        if pressed(i) {
            let Ok(input) = sig_inputs.single() else { continue };
            let signature: String = input.value.chars().take(300).collect();
            let tx = settings.tx.clone();
            let s = session_clone(&session);
            spawn_thread(move || {
                let r = renzora_auth::social::update_signature(&s, &signature).map(|_| ());
                let _ = tx.send(SettingsResult::Saved(r));
            });
        }
    }
    for (i, b) in &unblocks {
        if pressed(i) {
            settings.blocked.retain(|u| u.user_id != b.0);
            settings.bump();
            let tx = settings.tx.clone();
            let s = session_clone(&session);
            let id = b.0.clone();
            spawn_thread(move || {
                let r = renzora_auth::social::unblock(&s, &id).map(|_| ());
                let _ = tx.send(SettingsResult::Saved(r));
            });
        }
    }
}

// ── Build ────────────────────────────────────────────────────────────────────

fn setting_row(
    commands: &mut Commands,
    fonts: &EmberFonts,
    label: &str,
    description: &str,
) -> (Entity, Entity) {
    let row = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(10.0),
            ..default()
        })
        .id();
    let col = commands
        .spawn(Node { flex_direction: FlexDirection::Column, flex_grow: 1.0, ..default() })
        .id();
    let l = commands
        .spawn((Text::new(label.to_string()), ui_font(&fonts.ui, 11.5), TextColor(rgb(text_primary()))))
        .id();
    let d = commands
        .spawn((Text::new(description.to_string()), ui_font(&fonts.ui, 9.5), TextColor(rgb(text_muted()))))
        .id();
    commands.entity(col).add_children(&[l, d]);
    commands.entity(row).add_child(col);
    (row, col)
}

fn build(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let root = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(10.0),
            ..default()
        })
        .id();

    let note = commands
        .spawn((
            Text::new("These settings apply to your renzora.com account."),
            ui_font(&fonts.ui, 9.5),
            TextColor(rgb(placeholder())),
        ))
        .id();

    // Editor preference: top-bar notification bell (works signed in or out).
    // A switch bound both ways to the editor pref — no account API, just the
    // local `SocialBridge` flag, persisted to disk on change.
    let (row_bell, _) = setting_row(
        commands,
        fonts,
        "Notification bell",
        "Show notifications in the editor's top bar",
    );
    let sw_bell = toggle_switch(commands, true);
    bind_2way::<bool, _, _>(
        commands,
        sw_bell,
        |w| {
            w.get_resource::<SocialBridge>()
                .map(|b| b.notify_button_enabled)
                .unwrap_or(true)
        },
        |w, on| {
            if let Some(mut b) = w.get_resource_mut::<SocialBridge>() {
                b.notify_button_enabled = *on;
            }
            save_bell_pref(*on);
        },
    );
    commands.entity(row_bell).add_child(sw_bell);

    let signed_out = commands
        .spawn((Text::new("Sign in to manage privacy settings."), ui_font(&fonts.ui, 11.0), TextColor(rgb(text_muted()))))
        .id();
    bind_display(commands, signed_out, |w| !util::signed_in(w));

    let body = commands
        .spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(10.0), ..default() })
        .id();
    bind_display(commands, body, util::signed_in);

    // Who can message you.
    let (row1, _) = setting_row(commands, fonts, "Who can message you", "Controls who can start conversations with you");
    let b1 = util::pill_button(commands, fonts, "Change", hover_bg(), text_primary());
    commands.entity(b1).insert(MessagePrivacyBtn);
    let v1 = value_label(commands, fonts, b1);
    bind_text(commands, v1, |w| {
        w.get_resource::<SocialSettings>().map(|s| s.message_privacy.clone()).unwrap_or_default()
    });
    commands.entity(row1).add_child(b1);

    // Online status — a switch bound both ways to the account setting; flipping
    // it persists to renzora.com (same body as `save`) and bumps the list token.
    let (row2, _) = setting_row(commands, fonts, "Show online status", "Friends can see when you're in the editor");
    let sw_online = toggle_switch(commands, true);
    bind_2way::<bool, _, _>(
        commands,
        sw_online,
        |w| {
            w.get_resource::<SocialSettings>()
                .map(|s| s.online_status_visible)
                .unwrap_or(true)
        },
        |w, on| {
            if let Some(mut s) = w.get_resource_mut::<SocialSettings>() {
                s.online_status_visible = *on;
                s.bump();
            }
            // Persist to the account (reuse `save`'s body via its two resources).
            if let (Some(settings), Some(session)) =
                (w.get_resource::<SocialSettings>(), w.get_resource::<AuthSession>())
            {
                save(settings, session);
            }
        },
    );
    commands.entity(row2).add_child(sw_online);

    // Profile visibility.
    let (row3, _) = setting_row(commands, fonts, "Profile visibility", "Who can view your full profile");
    let b3 = util::pill_button(commands, fonts, "Change", hover_bg(), text_primary());
    commands.entity(b3).insert(ProfileVisibilityBtn);
    let v3 = value_label(commands, fonts, b3);
    bind_text(commands, v3, |w| {
        w.get_resource::<SocialSettings>().map(|s| s.profile_visibility.clone()).unwrap_or_default()
    });
    commands.entity(row3).add_child(b3);

    // Forum signature.
    let sig_title = commands
        .spawn((Text::new("Forum signature"), ui_font(&fonts.ui, 11.5), TextColor(rgb(text_primary())), Node { margin: UiRect::top(Val::Px(6.0)), ..default() }))
        .id();
    let sig_sub = commands
        .spawn((Text::new("Shown under your forum posts (max 300 characters)"), ui_font(&fonts.ui, 9.5), TextColor(rgb(text_muted()))))
        .id();
    let sig_input = textarea(commands, &fonts.ui, "e.g. Making a cozy farming game in Renzora", "");
    commands.entity(sig_input).insert((SignatureInput, Node { width: Val::Percent(100.0), min_height: Val::Px(48.0), ..default() }));
    let sig_save = util::pill_button(commands, fonts, "Save signature", hover_bg(), text_primary());
    commands.entity(sig_save).insert((SignatureSaveBtn, Node { align_self: AlignSelf::FlexStart, padding: UiRect::axes(Val::Px(10.0), Val::Px(4.0)), border_radius: BorderRadius::all(Val::Px(5.0)), ..default() }));

    // Blocked users.
    let blocked_title = commands
        .spawn((Text::new("Blocked users"), ui_font(&fonts.ui, 11.5), TextColor(rgb(text_primary())), Node { margin: UiRect::top(Val::Px(6.0)), ..default() }))
        .id();
    let blocked_list = commands
        .spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(3.0), ..default() })
        .id();
    keyed_list_tokened(
        commands,
        blocked_list,
        |w| w.get_resource::<SocialSettings>().map(|s| s.version).unwrap_or(0),
        blocked_snapshot,
    );

    commands.entity(body).add_children(&[row1, row2, row3, sig_title, sig_sub, sig_input, sig_save, blocked_title, blocked_list]);
    commands.entity(root).add_children(&[note, row_bell, signed_out, body]);
    root
}

/// A bindable value label appended inside the button (the pill's own label
/// stays static, e.g. "Change").
fn value_label(commands: &mut Commands, fonts: &EmberFonts, btn: Entity) -> Entity {
    let t = commands
        .spawn((
            Text::new(""),
            ui_font(&fonts.ui, 10.0),
            TextColor(rgb(accent())),
            Node { margin: UiRect::left(Val::Px(6.0)), ..default() },
        ))
        .id();
    commands.entity(btn).add_child(t);
    t
}

fn blocked_snapshot(w: &World) -> KeyedSnapshot {
    let Some(settings) = w.get_resource::<SocialSettings>() else {
        return util::empty_snapshot();
    };
    if settings.blocked.is_empty() {
        return KeyedSnapshot {
            items: vec![(u64::MAX, 1)],
            build: Box::new(|commands, fonts, _| {
                commands
                    .spawn((Text::new("No blocked users"), ui_font(&fonts.ui, 10.0), TextColor(rgb(placeholder()))))
                    .id()
            }),
        };
    }
    let blocked = settings.blocked.clone();
    let keys = blocked.iter().map(|u| (hash64(&u.user_id), hash64(&u.username))).collect();
    KeyedSnapshot {
        items: keys,
        build: Box::new(move |commands, fonts, i| {
            let u = &blocked[i];
            let row = commands
                .spawn((
                    Node {
                        width: Val::Percent(100.0),
                        flex_direction: FlexDirection::Row,
                        align_items: AlignItems::Center,
                        column_gap: Val::Px(8.0),
                        padding: UiRect::all(Val::Px(5.0)),
                        border_radius: BorderRadius::all(Val::Px(5.0)),
                        ..default()
                    },
                    BackgroundColor(rgb(section_bg())),
                ))
                .id();
            let av = crate::avatars::avatar_image(commands, fonts, u.avatar_url.as_deref(), 20.0);
            let name = commands
                .spawn((Text::new(u.username.clone()), ui_font(&fonts.ui, 11.0), TextColor(rgb(text_primary())), Node { flex_grow: 1.0, ..default() }))
                .id();
            let btn = util::pill_button(commands, fonts, "Unblock", hover_bg(), text_primary());
            commands.entity(btn).insert(UnblockBtn(u.user_id.clone()));
            commands.entity(row).add_children(&[av, name, btn]);
            row
        }),
    }
}


// ── Local editor prefs (not account settings) ────────────────────────────────

#[cfg(not(target_arch = "wasm32"))]
fn prefs_path() -> Option<std::path::PathBuf> {
    dirs::config_dir().map(|p| p.join("renzora").join("social.json"))
}

/// Whether the top-bar notification bell is enabled (default: yes).
pub(crate) fn load_bell_pref() -> bool {
    #[cfg(not(target_arch = "wasm32"))]
    {
        if let Some(path) = prefs_path() {
            if let Ok(data) = std::fs::read_to_string(path) {
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(&data) {
                    return v
                        .get("notify_button_enabled")
                        .and_then(|b| b.as_bool())
                        .unwrap_or(true);
                }
            }
        }
    }
    true
}

pub(crate) fn save_bell_pref(enabled: bool) {
    #[cfg(not(target_arch = "wasm32"))]
    {
        if let Some(path) = prefs_path() {
            if let Some(dir) = path.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            let json = serde_json::json!({ "notify_button_enabled": enabled });
            let _ = std::fs::write(path, serde_json::to_string_pretty(&json).unwrap_or_default());
        }
    }
    #[cfg(target_arch = "wasm32")]
    let _ = enabled;
}
