//! "Account" settings section — manage the renzora.com account itself.
//!
//! Where "Social & Privacy" ([`crate::settings`]) covers privacy/visibility, this
//! section owns the account's own data: profile fields + email, password, social
//! links, communication preferences, connected apps, and account deletion. It
//! follows the same shape as `settings.rs` — a resource that owns a
//! `crossbeam_channel`, a `poll_results`/`auto_load`/`clicks` triad, and a
//! build-once body with reactive bindings. Every blocking API call runs on a
//! worker thread (the crate convention) and reports back over the channel.

use bevy::prelude::*;
use crossbeam_channel::{unbounded, Receiver, Sender};
use renzora::core::AuthSignOutRequest;
use renzora::SplashState;
use renzora_auth::account::{
    self, AppGrant, CommunicationPrefs, ProfileUpdate, SocialConnection, SOCIAL_PLATFORMS,
};
use renzora_auth::social::MeResponse;
use renzora_auth::AuthSession;
use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::reactive::{bind_2way, bind_display, keyed_list_tokened, Bound, KeyedSnapshot};
use renzora_ember::settings_sections::RegisterSettingsSection;
use renzora_ember::theme::*;
use renzora_ember::widgets::{
    bind_text_input, dropdown, password_input, text_input, EmberTextInput,
};

use crate::toasts::{ToastQueue, Tone};
use crate::util::{self, hash64, session_clone};

/// Warning/destructive red (matches the profile overlay's `RED`).
const RED: (u8, u8, u8) = (224, 80, 80);
/// Grace window (seconds) between the two delete-account clicks: the first arms
/// and warns, a second within this window actually deletes.
const DELETE_CONFIRM_SECS: f64 = 5.0;

/// Results posted back from worker threads.
#[allow(clippy::large_enum_variant)]
pub(crate) enum AccountResult {
    /// Own-account snapshot — seeds username/email.
    Me(Result<MeResponse, String>),
    Connections(Result<Vec<SocialConnection>, String>),
    Comm(Result<CommunicationPrefs, String>),
    Grants(Result<Vec<AppGrant>, String>),
    /// A generic action outcome shown as a toast (`Ok` message / `Err` error).
    Toast(Result<String, String>),
    /// Account deletion — on success we sign the user out.
    Deleted(Result<(), String>),
}

#[derive(Resource)]
pub(crate) struct AccountSettings {
    pub username: String,
    pub email: String,
    pub comm: CommunicationPrefs,
    pub connections: Vec<SocialConnection>,
    pub grants: Vec<AppGrant>,
    /// Set (to an `elapsed_secs` deadline) when the delete button is armed; a
    /// second click before it expires performs the deletion. `None` = disarmed.
    pub delete_armed_until: Option<f64>,
    pub loaded: bool,
    pub loading: bool,
    pub version: u64,
    pub tx: Sender<AccountResult>,
    rx: Receiver<AccountResult>,
}

impl Default for AccountSettings {
    fn default() -> Self {
        let (tx, rx) = unbounded();
        Self {
            username: String::new(),
            email: String::new(),
            comm: CommunicationPrefs::default(),
            connections: Vec::new(),
            grants: Vec::new(),
            delete_armed_until: None,
            loaded: false,
            loading: false,
            version: 0,
            tx,
            rx,
        }
    }
}

impl AccountSettings {
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
    app.init_resource::<AccountSettings>();
    app.register_settings_section("account", "Account", "user-circle", build);
    app.add_systems(
        Update,
        (poll_results, auto_load, clicks).run_if(in_state(SplashState::Editor)),
    );
}

// ── Systems ──────────────────────────────────────────────────────────────────

fn poll_results(
    mut acc: ResMut<AccountSettings>,
    mut toasts: ResMut<ToastQueue>,
    mut commands: Commands,
) {
    let mut got = Vec::new();
    while let Ok(r) = acc.rx.try_recv() {
        got.push(r);
    }
    for r in got {
        match r {
            AccountResult::Me(Ok(me)) => {
                acc.username = me.username;
                acc.email = me.email;
                acc.loading = false;
                acc.bump();
            }
            AccountResult::Me(Err(_)) => {
                acc.loading = false;
            }
            AccountResult::Connections(Ok(list)) => {
                acc.connections = list;
                acc.bump();
            }
            AccountResult::Connections(Err(_)) => {}
            AccountResult::Comm(Ok(prefs)) => {
                acc.comm = prefs;
                acc.bump();
            }
            AccountResult::Comm(Err(_)) => {}
            AccountResult::Grants(Ok(list)) => {
                acc.grants = list;
                acc.bump();
            }
            AccountResult::Grants(Err(_)) => {}
            AccountResult::Toast(Ok(msg)) => {
                toasts.push(Tone::Success, msg, None);
            }
            AccountResult::Toast(Err(e)) => {
                toasts.push(Tone::Error, e, None);
            }
            AccountResult::Deleted(Ok(())) => {
                toasts.push(Tone::Success, "Account deleted", None);
                // Drop the session — the shell reacts to this exactly like the
                // user-menu "Sign out" item does.
                commands.insert_resource(AuthSignOutRequest);
            }
            AccountResult::Deleted(Err(e)) => {
                toasts.push(Tone::Error, format!("Couldn't delete account: {e}"), None);
            }
        }
    }
}

fn auto_load(mut acc: ResMut<AccountSettings>, session: Res<AuthSession>) {
    if session.is_signed_in() && !acc.loaded && !acc.loading {
        // Marked loaded at SPAWN time so a failed load never auto-retries.
        acc.loaded = true;
        acc.loading = true;
        let tx = acc.tx.clone();
        let s = session_clone(&session);
        spawn_thread(move || {
            let _ = tx.send(AccountResult::Me(renzora_auth::social::get_me(&s)));
            let _ = tx.send(AccountResult::Connections(account::list_connections(&s)));
            let _ = tx.send(AccountResult::Comm(account::get_communication(&s)));
            let _ = tx.send(AccountResult::Grants(account::list_app_grants(&s)));
        });
    }
    // Re-load after account switches.
    if !session.is_signed_in() && acc.loaded {
        *acc = AccountSettings::default();
    }
}

// ── Clicks ───────────────────────────────────────────────────────────────────

#[derive(Component)]
struct SaveProfileBtn;
#[derive(Component)]
struct CurrentPwInput;
#[derive(Component)]
struct NewPwInput;
#[derive(Component)]
struct ConfirmPwInput;
#[derive(Component)]
struct UpdatePwBtn;
#[derive(Component)]
struct AddPlatformDropdown;
#[derive(Component)]
struct AddConnectionInput;
#[derive(Component)]
struct AddConnectionBtn;
#[derive(Component)]
struct RemoveConnectionBtn(String);
#[derive(Component)]
struct RevokeAppBtn(String);
#[derive(Component)]
struct DeletePasswordInput;
#[derive(Component)]
struct DeleteAccountBtn;

#[allow(clippy::too_many_arguments)]
fn clicks(
    mut acc: ResMut<AccountSettings>,
    session: Res<AuthSession>,
    time: Res<Time>,
    save_profile: Query<&Interaction, (With<SaveProfileBtn>, Changed<Interaction>)>,
    update_pw: Query<&Interaction, (With<UpdatePwBtn>, Changed<Interaction>)>,
    pw_current: Query<&EmberTextInput, With<CurrentPwInput>>,
    pw_new: Query<&EmberTextInput, With<NewPwInput>>,
    pw_confirm: Query<&EmberTextInput, With<ConfirmPwInput>>,
    add_conn: Query<&Interaction, (With<AddConnectionBtn>, Changed<Interaction>)>,
    add_platform: Query<&Bound<usize>, With<AddPlatformDropdown>>,
    add_input: Query<&EmberTextInput, With<AddConnectionInput>>,
    remove_conn: Query<(&Interaction, &RemoveConnectionBtn), Changed<Interaction>>,
    revoke: Query<(&Interaction, &RevokeAppBtn), Changed<Interaction>>,
    delete_btn: Query<&Interaction, (With<DeleteAccountBtn>, Changed<Interaction>)>,
    delete_pw: Query<&EmberTextInput, With<DeletePasswordInput>>,
    mut toasts: ResMut<ToastQueue>,
) {
    let pressed = |i: &Interaction| *i == Interaction::Pressed;
    let now = time.elapsed_secs_f64();

    // Disarm a stale delete confirmation so its warning banner clears itself.
    if acc.delete_armed_until.is_some_and(|t| now >= t) {
        acc.delete_armed_until = None;
        acc.bump();
    }

    // ── Save profile (username + email) ──
    for i in &save_profile {
        if pressed(i) {
            let username = acc.username.trim().to_string();
            let email = acc.email.trim().to_string();
            if username.is_empty() || email.is_empty() {
                toasts.push(Tone::Error, "Username and email can't be empty", None);
                continue;
            }
            let tx = acc.tx.clone();
            let s = session_clone(&session);
            spawn_thread(move || {
                let update = ProfileUpdate {
                    username: Some(username),
                    email: Some(email),
                    ..Default::default()
                };
                let r = account::update_profile(&s, &update).map(|_| "Profile updated".to_string());
                let _ = tx.send(AccountResult::Toast(r));
            });
        }
    }

    // ── Change password ──
    for i in &update_pw {
        if pressed(i) {
            let current = pw_current.single().map(|x| x.value.clone()).unwrap_or_default();
            let new = pw_new.single().map(|x| x.value.clone()).unwrap_or_default();
            let confirm = pw_confirm.single().map(|x| x.value.clone()).unwrap_or_default();
            if new.chars().count() < 8 {
                toasts.push(Tone::Error, "New password must be at least 8 characters", None);
                continue;
            }
            if new != confirm {
                toasts.push(Tone::Error, "New passwords don't match", None);
                continue;
            }
            let tx = acc.tx.clone();
            let s = session_clone(&session);
            spawn_thread(move || {
                let r = account::change_password(&s, &current, &new)
                    .map(|_| "Password updated".to_string());
                let _ = tx.send(AccountResult::Toast(r));
            });
        }
    }

    // ── Add a social connection ──
    for i in &add_conn {
        if pressed(i) {
            let idx = add_platform.single().map(|b| b.0).unwrap_or(0);
            let Some((platform, _, _)) = SOCIAL_PLATFORMS.get(idx) else {
                continue;
            };
            let username = add_input
                .single()
                .map(|x| x.value.trim().to_string())
                .unwrap_or_default();
            if username.is_empty() {
                toasts.push(Tone::Error, "Enter a username for the connection", None);
                continue;
            }
            let platform = platform.to_string();
            let tx = acc.tx.clone();
            let s = session_clone(&session);
            spawn_thread(move || {
                let r = account::add_connection(&s, &platform, &username, None)
                    .map(|_| "Connection added".to_string());
                let _ = tx.send(AccountResult::Toast(r));
                // Refetch so the list reflects the (unverified) new entry.
                let _ = tx.send(AccountResult::Connections(account::list_connections(&s)));
            });
        }
    }

    // ── Remove a social connection ──
    for (i, b) in &remove_conn {
        if pressed(i) {
            acc.connections.retain(|c| c.platform != b.0);
            acc.bump();
            let platform = b.0.clone();
            let tx = acc.tx.clone();
            let s = session_clone(&session);
            spawn_thread(move || {
                let r = account::remove_connection(&s, &platform)
                    .map(|_| "Connection removed".to_string());
                let _ = tx.send(AccountResult::Toast(r));
                let _ = tx.send(AccountResult::Connections(account::list_connections(&s)));
            });
        }
    }

    // ── Revoke a connected app ──
    for (i, b) in &revoke {
        if pressed(i) {
            acc.grants.retain(|g| g.app_id != b.0);
            acc.bump();
            let app_id = b.0.clone();
            let tx = acc.tx.clone();
            let s = session_clone(&session);
            spawn_thread(move || {
                let r = account::revoke_app_grant(&s, &app_id)
                    .map(|_| "App access revoked".to_string());
                let _ = tx.send(AccountResult::Toast(r));
                let _ = tx.send(AccountResult::Grants(account::list_app_grants(&s)));
            });
        }
    }

    // ── Delete account (two-step confirm) ──
    for i in &delete_btn {
        if pressed(i) {
            let pw = delete_pw.single().map(|x| x.value.clone()).unwrap_or_default();
            if pw.is_empty() {
                toasts.push(Tone::Error, "Enter your password to delete your account", None);
                continue;
            }
            let armed = acc.delete_armed_until.is_some_and(|t| now < t);
            if !armed {
                // First click: arm + warn, don't delete yet.
                acc.delete_armed_until = Some(now + DELETE_CONFIRM_SECS);
                acc.bump();
                toasts.push(
                    Tone::Info,
                    "Click 'Delete account' again to permanently delete your account",
                    None,
                );
                continue;
            }
            // Second click inside the window: do it.
            acc.delete_armed_until = None;
            acc.bump();
            let tx = acc.tx.clone();
            let s = session_clone(&session);
            spawn_thread(move || {
                let r = account::delete_account(&s, &pw).map(|_| ());
                let _ = tx.send(AccountResult::Deleted(r));
            });
        }
    }
}

// ── Build ────────────────────────────────────────────────────────────────────

/// A label + description column and its row (mirrors `settings::setting_row`), so
/// a control can be appended to the returned row.
fn setting_row(commands: &mut Commands, fonts: &EmberFonts, label: &str, description: &str) -> Entity {
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
    row
}

/// A subsection header (with a little top breathing room).
fn section_header(commands: &mut Commands, fonts: &EmberFonts, title: &str) -> Entity {
    commands
        .spawn((
            Text::new(title.to_string()),
            ui_font(&fonts.ui, 12.5),
            TextColor(rgb(text_primary())),
            Node { margin: UiRect::top(Val::Px(8.0)), ..default() },
        ))
        .id()
}

/// A small caption line under a header.
fn caption(commands: &mut Commands, fonts: &EmberFonts, text: &str) -> Entity {
    commands
        .spawn((Text::new(text.to_string()), ui_font(&fonts.ui, 9.5), TextColor(rgb(text_muted()))))
        .id()
}

/// A field label stacked above its input control.
fn field(commands: &mut Commands, fonts: &EmberFonts, label: &str, input: Entity) -> Entity {
    let col = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(3.0),
            ..default()
        })
        .id();
    let l = commands
        .spawn((Text::new(label.to_string()), ui_font(&fonts.ui, 10.0), TextColor(rgb(text_muted()))))
        .id();
    commands.entity(col).add_children(&[l, input]);
    col
}

/// One communication-preference switch row, bound both ways to a field of
/// `AccountSettings.comm`. `get`/`set` are plain fn pointers (Copy, 'static), so
/// they satisfy the reactive binding's bounds without extra allocation.
fn comm_switch(
    commands: &mut Commands,
    fonts: &EmberFonts,
    body: Entity,
    label: &str,
    desc: &str,
    get: fn(&CommunicationPrefs) -> bool,
    set: fn(&mut CommunicationPrefs, bool),
) {
    let row = setting_row(commands, fonts, label, desc);
    let sw = renzora_ember::widgets::toggle_switch(commands, true);
    bind_2way::<bool, _, _>(
        commands,
        sw,
        move |w| w.get_resource::<AccountSettings>().map(|a| get(&a.comm)).unwrap_or(true),
        move |w, on| {
            let prefs = {
                let Some(mut a) = w.get_resource_mut::<AccountSettings>() else {
                    return;
                };
                set(&mut a.comm, *on);
                a.comm
            };
            persist_comm(w, prefs);
        },
    );
    commands.entity(row).add_child(sw);
    commands.entity(body).add_child(row);
}

/// Persist the communication prefs to the account on a worker thread.
fn persist_comm(w: &mut World, prefs: CommunicationPrefs) {
    let Some(acc) = w.get_resource::<AccountSettings>() else {
        return;
    };
    let tx = acc.tx.clone();
    let Some(session) = w.get_resource::<AuthSession>() else {
        return;
    };
    let s = session_clone(session);
    spawn_thread(move || {
        let _ = tx.send(AccountResult::Comm(account::update_communication(&s, &prefs)));
    });
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

    let signed_out = commands
        .spawn((
            Text::new("Sign in to manage your account."),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(text_muted())),
        ))
        .id();
    bind_display(commands, signed_out, |w| !util::signed_in(w));

    let body = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(8.0),
            ..default()
        })
        .id();
    bind_display(commands, body, util::signed_in);

    // ── Profile & email ──
    let h_profile = section_header(commands, fonts, "Profile & email");
    let username_in = text_input(commands, &fonts.ui, "Username", "");
    bind_text_input(
        commands,
        username_in,
        |w| w.get_resource::<AccountSettings>().map(|a| a.username.clone()).unwrap_or_default(),
        |w, v| {
            if let Some(mut a) = w.get_resource_mut::<AccountSettings>() {
                a.username = v;
            }
        },
    );
    let username_field = field(commands, fonts, "Username", username_in);
    let email_in = text_input(commands, &fonts.ui, "you@example.com", "");
    bind_text_input(
        commands,
        email_in,
        |w| w.get_resource::<AccountSettings>().map(|a| a.email.clone()).unwrap_or_default(),
        |w, v| {
            if let Some(mut a) = w.get_resource_mut::<AccountSettings>() {
                a.email = v;
            }
        },
    );
    let email_field = field(commands, fonts, "Email", email_in);
    let save_profile = util::pill_button(commands, fonts, "Save", accent(), (255, 255, 255));
    commands.entity(save_profile).insert((
        SaveProfileBtn,
        Node {
            align_self: AlignSelf::FlexStart,
            padding: UiRect::axes(Val::Px(12.0), Val::Px(5.0)),
            border_radius: BorderRadius::all(Val::Px(5.0)),
            ..default()
        },
    ));
    commands
        .entity(body)
        .add_children(&[h_profile, username_field, email_field, save_profile]);

    // ── Change password ──
    let h_pw = section_header(commands, fonts, "Change password");
    let cur_in = password_input(commands, &fonts.ui, "Current password", "");
    commands.entity(cur_in).insert(CurrentPwInput);
    let cur_field = field(commands, fonts, "Current password", cur_in);
    let new_in = password_input(commands, &fonts.ui, "New password (min 8 characters)", "");
    commands.entity(new_in).insert(NewPwInput);
    let new_field = field(commands, fonts, "New password", new_in);
    let confirm_in = password_input(commands, &fonts.ui, "Confirm new password", "");
    commands.entity(confirm_in).insert(ConfirmPwInput);
    let confirm_field = field(commands, fonts, "Confirm password", confirm_in);
    let update_pw = util::pill_button(commands, fonts, "Update password", hover_bg(), text_primary());
    commands.entity(update_pw).insert((
        UpdatePwBtn,
        Node {
            align_self: AlignSelf::FlexStart,
            padding: UiRect::axes(Val::Px(12.0), Val::Px(5.0)),
            border_radius: BorderRadius::all(Val::Px(5.0)),
            ..default()
        },
    ));
    commands
        .entity(body)
        .add_children(&[h_pw, cur_field, new_field, confirm_field, update_pw]);

    // ── Social links ──
    let h_social = section_header(commands, fonts, "Social links");
    let social_cap = caption(commands, fonts, "Link accounts to show on your public profile");
    let social_list = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(3.0),
            ..default()
        })
        .id();
    keyed_list_tokened(
        commands,
        social_list,
        |w| w.get_resource::<AccountSettings>().map(|a| a.version).unwrap_or(0),
        connections_snapshot,
    );
    // Add row: platform dropdown + username input + Add.
    let add_row = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(6.0),
            flex_wrap: FlexWrap::Wrap,
            row_gap: Val::Px(6.0),
            ..default()
        })
        .id();
    let labels: Vec<&str> = SOCIAL_PLATFORMS.iter().map(|(_, l, _)| *l).collect();
    let platform_dd = dropdown(commands, fonts, &labels, 0);
    commands.entity(platform_dd).insert(AddPlatformDropdown);
    let add_in = text_input(commands, &fonts.ui, "Username or profile URL", "");
    commands.entity(add_in).insert(AddConnectionInput);
    let add_btn = util::pill_button(commands, fonts, "Add", accent(), (255, 255, 255));
    commands.entity(add_btn).insert(AddConnectionBtn);
    commands.entity(add_row).add_children(&[platform_dd, add_in, add_btn]);
    commands
        .entity(body)
        .add_children(&[h_social, social_cap, social_list, add_row]);

    // ── Communication ──
    let h_comm = section_header(commands, fonts, "Communication");
    commands.entity(body).add_child(h_comm);
    comm_switch(
        commands,
        fonts,
        body,
        "Product updates",
        "News about new features and releases.",
        |c| c.product_updates,
        |c, v| c.product_updates = v,
    );
    comm_switch(
        commands,
        fonts,
        body,
        "Marketplace",
        "When someone purchases or reviews your assets.",
        |c| c.marketplace,
        |c, v| c.marketplace = v,
    );
    comm_switch(
        commands,
        fonts,
        body,
        "Comments",
        "When someone replies to your comments.",
        |c| c.comments,
        |c, v| c.comments = v,
    );
    comm_switch(
        commands,
        fonts,
        body,
        "Security alerts",
        "Sign-in from new devices and password changes.",
        |c| c.security,
        |c, v| c.security = v,
    );

    // ── Connected apps ──
    let h_apps = section_header(commands, fonts, "Connected apps");
    let apps_list = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(3.0),
            ..default()
        })
        .id();
    keyed_list_tokened(
        commands,
        apps_list,
        |w| w.get_resource::<AccountSettings>().map(|a| a.version).unwrap_or(0),
        grants_snapshot,
    );
    commands.entity(body).add_children(&[h_apps, apps_list]);

    // ── Danger zone ──
    let danger = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(6.0),
                padding: UiRect::all(Val::Px(10.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(8.0)),
                margin: UiRect::top(Val::Px(8.0)),
                ..default()
            },
            BackgroundColor(rgba([RED.0, RED.1, RED.2, 20])),
            BorderColor::all(rgba([RED.0, RED.1, RED.2, 120])),
        ))
        .id();
    let danger_h = commands
        .spawn((Text::new("Danger zone"), ui_font(&fonts.ui, 12.5), TextColor(rgb(RED))))
        .id();
    let danger_cap = caption(
        commands,
        fonts,
        "Permanently delete your account and all associated data. This cannot be undone.",
    );
    let del_pw = password_input(commands, &fonts.ui, "Enter your password to confirm", "");
    commands.entity(del_pw).insert(DeletePasswordInput);
    let del_btn = util::pill_button(commands, fonts, "Delete account", RED, (255, 255, 255));
    commands.entity(del_btn).insert((
        DeleteAccountBtn,
        Node {
            align_self: AlignSelf::FlexStart,
            padding: UiRect::axes(Val::Px(12.0), Val::Px(5.0)),
            border_radius: BorderRadius::all(Val::Px(5.0)),
            ..default()
        },
    ));
    // Armed-state warning: only shown between the two confirmation clicks.
    let armed_warn = commands
        .spawn((
            Text::new("This is permanent — click 'Delete account' again to confirm."),
            ui_font(&fonts.ui, 9.5),
            TextColor(rgb(RED)),
        ))
        .id();
    bind_display(commands, armed_warn, |w| {
        w.get_resource::<AccountSettings>()
            .map(|a| a.delete_armed_until.is_some())
            .unwrap_or(false)
    });
    commands
        .entity(danger)
        .add_children(&[danger_h, danger_cap, del_pw, del_btn, armed_warn]);
    commands.entity(body).add_child(danger);

    commands.entity(root).add_children(&[signed_out, body]);
    root
}

// ── Snapshots ─────────────────────────────────────────────────────────────────

fn connections_snapshot(w: &World) -> KeyedSnapshot {
    let Some(acc) = w.get_resource::<AccountSettings>() else {
        return util::empty_snapshot();
    };
    if acc.connections.is_empty() {
        return KeyedSnapshot {
            items: vec![(u64::MAX, 1)],
            build: Box::new(|commands, fonts, _| {
                commands
                    .spawn((
                        Text::new("No social links yet"),
                        ui_font(&fonts.ui, 10.0),
                        TextColor(rgb(placeholder())),
                    ))
                    .id()
            }),
        };
    }
    let conns = acc.connections.clone();
    let keys = conns
        .iter()
        .map(|c| (hash64(&c.platform), hash64(&(&c.platform_username, c.verified))))
        .collect();
    KeyedSnapshot {
        items: keys,
        build: Box::new(move |commands, fonts, i| {
            let c = &conns[i];
            let (label, icon) = SOCIAL_PLATFORMS
                .iter()
                .find(|(v, _, _)| *v == c.platform)
                .map(|(_, l, ic)| (*l, *ic))
                .unwrap_or((c.platform.as_str(), "link"));
            let row = commands
                .spawn((
                    Node {
                        width: Val::Percent(100.0),
                        flex_direction: FlexDirection::Row,
                        align_items: AlignItems::Center,
                        column_gap: Val::Px(8.0),
                        padding: UiRect::all(Val::Px(6.0)),
                        border_radius: BorderRadius::all(Val::Px(5.0)),
                        ..default()
                    },
                    BackgroundColor(rgb(section_bg())),
                ))
                .id();
            let ic = icon_text(commands, &fonts.phosphor, icon, text_muted(), 15.0);
            let name = commands
                .spawn((
                    Text::new(if c.platform_username.is_empty() {
                        label.to_string()
                    } else {
                        format!("{label} · {}", c.platform_username)
                    }),
                    ui_font(&fonts.ui, 11.0),
                    TextColor(rgb(text_primary())),
                    Node { flex_grow: 1.0, ..default() },
                ))
                .id();
            let mut kids = vec![ic, name];
            if c.verified {
                kids.push(icon_text(commands, &fonts.phosphor, "seal-check", (82, 196, 120), 13.0));
            }
            let remove = util::pill_button(commands, fonts, "Remove", hover_bg(), text_primary());
            commands.entity(remove).insert(RemoveConnectionBtn(c.platform.clone()));
            kids.push(remove);
            commands.entity(row).add_children(&kids);
            row
        }),
    }
}

fn grants_snapshot(w: &World) -> KeyedSnapshot {
    let Some(acc) = w.get_resource::<AccountSettings>() else {
        return util::empty_snapshot();
    };
    if acc.grants.is_empty() {
        return KeyedSnapshot {
            items: vec![(u64::MAX, 1)],
            build: Box::new(|commands, fonts, _| {
                commands
                    .spawn((
                        Text::new("No connected apps."),
                        ui_font(&fonts.ui, 10.0),
                        TextColor(rgb(placeholder())),
                    ))
                    .id()
            }),
        };
    }
    let grants = acc.grants.clone();
    let keys = grants
        .iter()
        .map(|g| (hash64(&g.app_id), hash64(&(&g.app_name, g.scopes_granted.len()))))
        .collect();
    KeyedSnapshot {
        items: keys,
        build: Box::new(move |commands, fonts, i| {
            let g = &grants[i];
            let row = commands
                .spawn((
                    Node {
                        width: Val::Percent(100.0),
                        flex_direction: FlexDirection::Row,
                        align_items: AlignItems::Center,
                        column_gap: Val::Px(10.0),
                        padding: UiRect::all(Val::Px(8.0)),
                        border: UiRect::all(Val::Px(1.0)),
                        border_radius: BorderRadius::all(Val::Px(8.0)),
                        ..default()
                    },
                    BackgroundColor(rgb(card_bg())),
                    BorderColor::all(rgba([255, 255, 255, 12])),
                ))
                .id();
            let icon = crate::avatars::avatar_image(commands, fonts, g.app_icon_url.as_deref(), 32.0);
            let col = commands
                .spawn(Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(2.0),
                    flex_grow: 1.0,
                    ..default()
                })
                .id();
            let name = commands
                .spawn((
                    Text::new(if g.app_name.is_empty() { g.app_id.clone() } else { g.app_name.clone() }),
                    ui_font(&fonts.ui, 11.5),
                    TextColor(rgb(text_primary())),
                ))
                .id();
            let mut meta_parts: Vec<String> = Vec::new();
            if !g.scopes_granted.is_empty() {
                meta_parts.push(g.scopes_granted.join(", "));
            }
            let date = g.granted_at.get(0..10).unwrap_or("").to_string();
            if !date.is_empty() {
                meta_parts.push(format!("granted {date}"));
            }
            let meta = commands
                .spawn((
                    Text::new(meta_parts.join(" · ")),
                    ui_font(&fonts.ui, 9.5),
                    TextColor(rgb(text_muted())),
                ))
                .id();
            commands.entity(col).add_children(&[name, meta]);
            let revoke = util::pill_button(commands, fonts, "Revoke", hover_bg(), text_primary());
            commands.entity(revoke).insert(RevokeAppBtn(g.app_id.clone()));
            commands.entity(row).add_children(&[icon, col, revoke]);
            row
        }),
    }
}
