//! Bevy-native (ember) sign-in modal. Three views (Sign In / Create Account /
//! Reset Password) with their fields, links, status/error messages and async
//! API calls (reusing `spawn_auth_request` / `poll_auth_result`).

use bevy::prelude::*;
use bevy::ecs::world::CommandQueue;
use bevy::ui::FocusPolicy;

use renzora_ember::font::{ui_font, EmberFonts};
use renzora_ember::theme::{accent, border, popup_bg, rgb, text_muted, text_primary};
use renzora_ember::widgets::{bind_text_input, password_input, text_input, EmberTextInput, OverlaySurface};

use crate::{api, spawn_auth_request, AuthResult, AuthSession, AuthState, AuthView};

const GREEN: (u8, u8, u8) = (34, 197, 94);
const RED: (u8, u8, u8) = (239, 68, 68);

#[derive(Component)]
struct AuthBackdrop;
#[derive(Component)]
struct AuthContent {
    sig: Option<u64>,
}
#[derive(Component)]
struct AuthSubmit;
#[derive(Component)]
struct AuthLink(AuthView);
#[derive(Component)]
struct AuthFirstField;

pub(crate) fn register(app: &mut App) {
    app.add_systems(
        Update,
        (
            native_auth_poll,
            manage_auth_modal,
            rebuild_auth_modal,
            focus_auth_field,
            auth_submit_click,
            auth_link_click,
            auth_backdrop_click,
            auth_escape,
        ),
    );
}

// ── Async result polling ─────────────────────────────────────────────────────

fn native_auth_poll(world: &mut World) {
    let mut auth = world.remove_resource::<AuthState>();
    let mut session = world.remove_resource::<AuthSession>();
    let mut signed = false;
    if let (Some(a), Some(s)) = (&mut auth, &mut session) {
        crate::poll_auth_result(a, s);
        if a.just_signed_in {
            a.just_signed_in = false;
            signed = true;
        }
    }
    if let Some(a) = auth {
        world.insert_resource(a);
    }
    if let Some(s) = session {
        world.insert_resource(s);
    }
    if signed {
        world.insert_resource(renzora::core::AuthJustSignedIn);
    }
}

// ── Modal lifecycle ──────────────────────────────────────────────────────────

fn modal_wanted(world: &World) -> bool {
    let open = world.get_resource::<AuthState>().is_some_and(|a| a.window_open);
    let signed = world.get_resource::<AuthSession>().is_some_and(|s| s.is_signed_in());
    open && !signed
}

fn manage_auth_modal(world: &mut World) {
    // Signed in while open → close (mirrors the egui guard).
    let open = world.get_resource::<AuthState>().is_some_and(|a| a.window_open);
    let signed = world.get_resource::<AuthSession>().is_some_and(|s| s.is_signed_in());
    if signed && open {
        if let Some(mut a) = world.get_resource_mut::<AuthState>() {
            a.window_open = false;
        }
    }

    let want = modal_wanted(world);
    let mut q = world.query_filtered::<Entity, With<AuthBackdrop>>();
    let existing: Vec<Entity> = q.iter(world).collect();

    if want && existing.is_empty() {
        let mut queue = CommandQueue::default();
        {
            let mut commands = Commands::new(&mut queue, world);
            spawn_modal(&mut commands);
        }
        queue.apply(world);
    } else if !want && !existing.is_empty() {
        for e in existing {
            world.entity_mut(e).despawn();
        }
    }
}

fn spawn_modal(commands: &mut Commands) {
    let backdrop = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                right: Val::Px(0.0),
                bottom: Val::Px(0.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.47)),
            GlobalZIndex(9400),
            FocusPolicy::Block,
            Interaction::default(),
            bevy::ui::RelativeCursorPosition::default(),
            OverlaySurface,
            AuthBackdrop,
            Name::new("auth-modal"),
        ))
        .id();
    let panel = commands
        .spawn((
            Node {
                width: Val::Px(320.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(16.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(8.0)),
                ..default()
            },
            BackgroundColor(rgb(popup_bg())),
            BorderColor::all(rgb(border())),
            FocusPolicy::Block,
            Name::new("auth-panel"),
        ))
        .id();
    let content = commands
        .spawn((Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(6.0), ..default() }, AuthContent { sig: None }))
        .id();
    commands.entity(panel).add_child(content);
    commands.entity(backdrop).add_child(panel);
}

// ── Content (rebuilt on view / status / error / loading change) ───────────────

fn content_sig(a: &AuthState) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    (a.view as u8).hash(&mut h);
    a.status.hash(&mut h);
    a.error.hash(&mut h);
    a.loading.hash(&mut h);
    h.finish()
}

fn rebuild_auth_modal(world: &mut World) {
    if !modal_wanted(world) {
        return;
    }
    let Some(fonts) = world.get_resource::<EmberFonts>().cloned() else { return };
    let (view, status, error, loading, sig) = {
        let Some(a) = world.get_resource::<AuthState>() else { return };
        (a.view, a.status.clone(), a.error.clone(), a.loading, content_sig(a))
    };

    let mut q = world.query::<(Entity, &AuthContent)>();
    let Some((container, old_sig)) = q.iter(world).map(|(e, c)| (e, c.sig)).next() else { return };
    if old_sig == Some(sig) {
        return;
    }

    let existing: Vec<Entity> = world.get::<Children>(container).map(|c| c.iter().collect()).unwrap_or_default();
    let mut queue = CommandQueue::default();
    {
        let mut commands = Commands::new(&mut queue, world);
        for ch in existing {
            commands.entity(ch).despawn();
        }
        build_content(&mut commands, &fonts, container, view, status.as_deref(), error.as_deref(), loading);
    }
    queue.apply(world);
    if let Some(mut c) = world.get_mut::<AuthContent>(container) {
        c.sig = Some(sig);
    }
}

fn build_content(commands: &mut Commands, fonts: &EmberFonts, container: Entity, view: AuthView, status: Option<&str>, error: Option<&str>, loading: bool) {
    let title = match view {
        AuthView::SignIn => "Sign In",
        AuthView::Register => "Create Account",
        AuthView::ForgotPassword => "Reset Password",
    };
    let mut kids: Vec<Entity> = Vec::new();
    kids.push(text_node(commands, fonts, title, 15.0, text_primary()));

    if let Some(msg) = status {
        kids.push(text_node(commands, fonts, msg, 11.0, GREEN));
    }
    if let Some(err) = error {
        kids.push(text_node(commands, fonts, err, 11.0, RED));
    }

    match view {
        AuthView::SignIn => {
            kids.push(field(commands, fonts, "Email", "you@example.com", g_email, s_email, false, true));
            kids.push(field(commands, fonts, "Password", "Password", g_password, s_password, true, false));
            kids.push(link_row(commands, fonts, None, "Forgot password?", AuthView::ForgotPassword, true));
            kids.push(submit_button(commands, fonts, if loading { "Signing in..." } else { "Sign In" }));
            kids.push(link_row(commands, fonts, Some("Don't have an account?"), "Register", AuthView::Register, false));
        }
        AuthView::Register => {
            kids.push(field(commands, fonts, "Username", "Username", g_username, s_username, false, true));
            kids.push(field(commands, fonts, "Email", "you@example.com", g_email, s_email, false, false));
            kids.push(field(commands, fonts, "Password", "Password", g_password, s_password, true, false));
            kids.push(field(commands, fonts, "Confirm Password", "Confirm password", g_confirm, s_confirm, true, false));
            kids.push(submit_button(commands, fonts, if loading { "Creating account..." } else { "Create Account" }));
            kids.push(link_row(commands, fonts, Some("Already have an account?"), "Sign In", AuthView::SignIn, false));
        }
        AuthView::ForgotPassword => {
            kids.push(text_node(commands, fonts, "Enter your email and we'll send you a link to reset your password.", 11.0, text_muted()));
            kids.push(field(commands, fonts, "Email", "you@example.com", g_email, s_email, false, true));
            kids.push(submit_button(commands, fonts, if loading { "Sending..." } else { "Send Reset Link" }));
            kids.push(link_row(commands, fonts, None, "Back to Sign In", AuthView::SignIn, false));
        }
    }

    commands.entity(container).add_children(&kids);
}

fn text_node(commands: &mut Commands, fonts: &EmberFonts, text: &str, size: f32, color: (u8, u8, u8)) -> Entity {
    commands.spawn((Text::new(text.to_string()), ui_font(&fonts.ui, size), TextColor(rgb(color)))).id()
}

#[allow(clippy::too_many_arguments)]
fn field(
    commands: &mut Commands,
    fonts: &EmberFonts,
    label: &str,
    placeholder: &str,
    get: fn(&World) -> String,
    set: fn(&mut World, String),
    password: bool,
    first: bool,
) -> Entity {
    let col = commands.spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(2.0), ..default() }).id();
    let lbl = text_node(commands, fonts, label, 11.0, text_muted());
    let input = if password {
        password_input(commands, &fonts.ui, placeholder, "")
    } else {
        text_input(commands, &fonts.ui, placeholder, "")
    };
    commands.entity(input).insert(Node {
        width: Val::Percent(100.0),
        height: Val::Px(28.0),
        align_items: AlignItems::Center,
        padding: UiRect::horizontal(Val::Px(8.0)),
        border: UiRect::all(Val::Px(1.0)),
        border_radius: BorderRadius::all(Val::Px(4.0)),
        ..default()
    });
    if first {
        commands.entity(input).insert(AuthFirstField);
    }
    bind_text_input(commands, input, get, set);
    commands.entity(col).add_children(&[lbl, input]);
    col
}

fn submit_button(commands: &mut Commands, fonts: &EmberFonts, text: &str) -> Entity {
    let btn = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(32.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                margin: UiRect::vertical(Val::Px(4.0)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(rgb(accent())),
            Interaction::default(),
            AuthSubmit,
            Name::new("auth-submit"),
        ))
        .id();
    let t = commands.spawn((Text::new(text.to_string()), ui_font(&fonts.ui, 13.0), TextColor(Color::WHITE), FocusPolicy::Pass)).id();
    commands.entity(btn).add_child(t);
    btn
}

/// A row with an optional muted prefix label + a clickable accent link that
/// switches to `target` view. `right` right-aligns it (the "Forgot password?").
fn link_row(commands: &mut Commands, fonts: &EmberFonts, prefix: Option<&str>, link: &str, target: AuthView, right: bool) -> Entity {
    let row = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(5.0),
            justify_content: if right { JustifyContent::FlexEnd } else { JustifyContent::FlexStart },
            ..default()
        })
        .id();
    let mut kids = Vec::new();
    if let Some(p) = prefix {
        kids.push(text_node(commands, fonts, p, 11.0, text_muted()));
    }
    let link_e = commands
        .spawn((
            Text::new(link.to_string()),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(accent())),
            Interaction::default(),
            AuthLink(target),
            Name::new("auth-link"),
        ))
        .id();
    kids.push(link_e);
    commands.entity(row).add_children(&kids);
    row
}

// ── Field accessors ──────────────────────────────────────────────────────────

fn g_email(w: &World) -> String { w.get_resource::<AuthState>().map(|a| a.email.clone()).unwrap_or_default() }
fn s_email(w: &mut World, v: String) { if let Some(mut a) = w.get_resource_mut::<AuthState>() { a.email = v; } }
fn g_password(w: &World) -> String { w.get_resource::<AuthState>().map(|a| a.password.clone()).unwrap_or_default() }
fn s_password(w: &mut World, v: String) { if let Some(mut a) = w.get_resource_mut::<AuthState>() { a.password = v; } }
fn g_username(w: &World) -> String { w.get_resource::<AuthState>().map(|a| a.username.clone()).unwrap_or_default() }
fn s_username(w: &mut World, v: String) { if let Some(mut a) = w.get_resource_mut::<AuthState>() { a.username = v; } }
fn g_confirm(w: &World) -> String { w.get_resource::<AuthState>().map(|a| a.confirm_password.clone()).unwrap_or_default() }
fn s_confirm(w: &mut World, v: String) { if let Some(mut a) = w.get_resource_mut::<AuthState>() { a.confirm_password = v; } }

// ── Interaction ──────────────────────────────────────────────────────────────

fn focus_auth_field(mut q: Query<&mut EmberTextInput, Added<AuthFirstField>>) {
    for mut inp in &mut q {
        inp.focused = true;
    }
}

fn auth_submit_click(q: Query<&Interaction, (With<AuthSubmit>, Changed<Interaction>)>, mut commands: Commands) {
    if q.iter().any(|i| *i == Interaction::Pressed) {
        commands.queue(do_submit);
    }
}

fn do_submit(world: &mut World) {
    let Some(mut auth) = world.get_resource_mut::<AuthState>() else { return };
    if auth.loading {
        return;
    }
    match auth.view {
        AuthView::SignIn => {
            let (email, password) = (auth.email.clone(), auth.password.clone());
            spawn_auth_request(&mut auth, move || match api::login(&email, &password) {
                Ok(r) => AuthResult::Success(r),
                Err(e) => AuthResult::Error(e),
            });
        }
        AuthView::Register => {
            if auth.password != auth.confirm_password {
                auth.error = Some("Passwords do not match".into());
            } else if auth.password.len() < 8 {
                auth.error = Some("Password must be at least 8 characters".into());
            } else if auth.username.len() < 3 {
                auth.error = Some("Username must be at least 3 characters".into());
            } else {
                let (u, e, p) = (auth.username.clone(), auth.email.clone(), auth.password.clone());
                spawn_auth_request(&mut auth, move || match api::register(&u, &e, &p) {
                    Ok(r) => AuthResult::Success(r),
                    Err(err) => AuthResult::Error(err),
                });
            }
        }
        AuthView::ForgotPassword => {
            let email = auth.email.clone();
            spawn_auth_request(&mut auth, move || match api::forgot_password(&email) {
                Ok(r) => AuthResult::ForgotSuccess(r.message),
                Err(e) => AuthResult::Error(e),
            });
        }
    }
}

fn auth_link_click(q: Query<(&Interaction, &AuthLink), Changed<Interaction>>, mut auth: Option<ResMut<AuthState>>) {
    let Some(auth) = auth.as_mut() else { return };
    for (interaction, link) in &q {
        if *interaction == Interaction::Pressed {
            auth.view = link.0;
            auth.error = None;
            auth.status = None;
        }
    }
}

fn auth_backdrop_click(q: Query<&Interaction, (With<AuthBackdrop>, Changed<Interaction>)>, mut auth: Option<ResMut<AuthState>>) {
    let Some(auth) = auth.as_mut() else { return };
    if q.iter().any(|i| *i == Interaction::Pressed) {
        auth.window_open = false;
    }
}

fn auth_escape(keys: Res<ButtonInput<KeyCode>>, mut auth: Option<ResMut<AuthState>>) {
    let Some(auth) = auth.as_mut() else { return };
    if auth.window_open && keys.just_pressed(KeyCode::Escape) {
        auth.window_open = false;
    }
}
