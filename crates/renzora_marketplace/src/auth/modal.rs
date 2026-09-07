//! Bevy-native (ember) sign-in modal. Three views (Sign In / Create Account /
//! Reset Password) with their fields, links, status/error messages and async
//! API calls (reusing `spawn_auth_request` / `poll_auth_result`).

use bevy::prelude::*;
use bevy::ecs::world::CommandQueue;
use bevy::ui::FocusPolicy;

use renzora_ember::reactive::Rx;
use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::theme::{accent, border, divider, popup_bg, rgb, text_muted, text_primary};
use renzora_ember::widgets::{
    bind_text_input, password_input, text_input, EmberForm, EmberTextInput, OverlaySurface,
};

use super::{api, spawn_auth_request, AuthResult, AuthSession, AuthState, AuthView};

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
/// The header's X.
#[derive(Component)]
struct AuthClose;

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
            auth_close_click,
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
        super::poll_auth_result(a, s);
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

fn modal_wanted(world: &Rx) -> bool {
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

    let want = modal_wanted(&Rx::new(&*world));
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
                width: Val::Px(392.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(28.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(12.0)),
                ..default()
            },
            BackgroundColor(rgb(popup_bg())),
            BorderColor::all(rgb(border())),
            FocusPolicy::Block,
            Name::new("auth-panel"),
        ))
        .id();
    let content = commands
        .spawn((Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(14.0), ..default() }, AuthContent { sig: None }))
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
    if !modal_wanted(&Rx::new(&*world)) {
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
    // Each view says what it is *and* what it gets you. A modal that only says
    // "Sign In" over two boxes leaves the obvious question unanswered, which is
    // why anyone is being asked at all — the subtitle is the answer, and it is
    // also what stops the panel reading as a bare form.
    let (title, subtitle, glyph) = match view {
        AuthView::SignIn => (
            "Sign In",
            "Sign in to install, publish and manage marketplace content.",
            "user-circle",
        ),
        AuthView::Register => (
            "Create Account",
            "A renzora.com account is free, and is only needed for the marketplace.",
            "user-plus",
        ),
        AuthView::ForgotPassword => (
            "Reset Password",
            "Enter your email and we'll send you a link to set a new password.",
            "key",
        ),
    };
    let mut kids: Vec<Entity> = vec![header(commands, fonts, title, subtitle, glyph)];

    if let Some(msg) = status {
        kids.push(banner(commands, fonts, msg, GREEN));
    }
    if let Some(err) = error {
        kids.push(banner(commands, fonts, err, RED));
    }

    // The fields as one block with their own tighter rhythm, so the gap between
    // Email and Password reads as smaller than the gap between the block and the
    // button — the grouping does the work a heavier border would.
    let fields = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(12.0),
            ..default()
        })
        .id();
    let submit;
    let mut field_kids: Vec<Entity> = Vec::new();
    let mut footer: Vec<Entity> = Vec::new();
    match view {
        AuthView::SignIn => {
            field_kids.push(field(commands, fonts, "Email", "you@example.com", g_email, s_email, false, true));
            field_kids.push(field(commands, fonts, "Password", "Password", g_password, s_password, true, false));
            field_kids.push(link_row(commands, fonts, None, "Forgot password?", AuthView::ForgotPassword, true));
            submit = submit_button(commands, fonts, if loading { "Signing in..." } else { "Sign In" }, loading);
            footer.push(link_row(commands, fonts, Some("Don't have an account?"), "Register", AuthView::Register, false));
        }
        AuthView::Register => {
            field_kids.push(field(commands, fonts, "Username", "Username", g_username, s_username, false, true));
            field_kids.push(field(commands, fonts, "Email", "you@example.com", g_email, s_email, false, false));
            field_kids.push(field(commands, fonts, "Password", "Password", g_password, s_password, true, false));
            field_kids.push(field(commands, fonts, "Confirm Password", "Confirm password", g_confirm, s_confirm, true, false));
            submit = submit_button(commands, fonts, if loading { "Creating account..." } else { "Create Account" }, loading);
            footer.push(link_row(commands, fonts, Some("Already have an account?"), "Sign In", AuthView::SignIn, false));
        }
        AuthView::ForgotPassword => {
            field_kids.push(field(commands, fonts, "Email", "you@example.com", g_email, s_email, false, true));
            submit = submit_button(commands, fonts, if loading { "Sending..." } else { "Send Reset Link" }, loading);
            footer.push(link_row(commands, fonts, None, "Back to Sign In", AuthView::SignIn, false));
        }
    }
    commands.entity(fields).add_children(&field_kids);
    kids.push(fields);
    kids.push(submit);
    if !footer.is_empty() {
        kids.push(rule(commands));
        kids.extend(footer);
    }

    // Tab cycles the fields; Enter in any of them presses the submit button.
    commands.entity(container).insert(EmberForm { submit });
    commands.entity(container).add_children(&kids);
}

/// Title block: the view's icon inline beside its name, a close button on the
/// right of that row, and a line underneath saying what the view is for.
///
/// The icon used to sit in a tinted rounded badge on its own line above the
/// title. It made the header three rows tall for two pieces of information, and
/// the badge was decoration standing in for a product mark it was not. Inline it
/// reads as one heading.
fn header(commands: &mut Commands, fonts: &EmberFonts, title: &str, subtitle: &str, glyph: &str) -> Entity {
    let col = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(6.0),
            margin: UiRect::bottom(Val::Px(2.0)),
            ..default()
        })
        .id();
    let row = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(10.0),
            ..default()
        })
        .id();
    let ic = icon_text(commands, &fonts.phosphor, glyph, accent(), 22.0);
    let t = text_node(commands, fonts, title, 21.0, text_primary());
    let spacer = commands.spawn(Node { flex_grow: 1.0, ..default() }).id();
    // Escape and a backdrop click already close this (`auth_escape`,
    // `auth_backdrop_click`), but neither is visible. A modal with no way out you
    // can see reads as one you are stuck in.
    let close = commands
        .spawn((
            Node {
                width: Val::Px(26.0),
                height: Val::Px(26.0),
                flex_shrink: 0.0,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border_radius: BorderRadius::all(Val::Px(5.0)),
                ..default()
            },
            BackgroundColor(Color::NONE),
            Interaction::default(),
            renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            AuthClose,
            Name::new("auth-close"),
        ))
        .id();
    let close_icon = icon_text(commands, &fonts.phosphor, "x", text_muted(), 14.0);
    commands.entity(close_icon).insert(FocusPolicy::Pass);
    commands.entity(close).add_child(close_icon);
    commands.entity(row).add_children(&[ic, t, spacer, close]);
    let s = commands
        .spawn((
            Text::new(subtitle.to_string()),
            ui_font(&fonts.ui, 12.5),
            TextColor(rgb(text_muted())),
        ))
        .id();
    commands.entity(col).add_children(&[row, s]);
    col
}

/// A status or error message as a tinted, rounded strip rather than a loose line
/// of coloured text — at this size a bare sentence reads as body copy, and an
/// error has to be the thing you see first.
fn banner(commands: &mut Commands, fonts: &EmberFonts, msg: &str, color: (u8, u8, u8)) -> Entity {
    let (r, g, b) = color;
    let row = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                padding: UiRect::axes(Val::Px(11.0), Val::Px(9.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(7.0)),
                ..default()
            },
            BackgroundColor(Color::srgba_u8(r, g, b, 30)),
            BorderColor::all(Color::srgba_u8(r, g, b, 90)),
        ))
        .id();
    let t = text_node(commands, fonts, msg, 12.0, color);
    commands.entity(row).add_child(t);
    row
}

/// A hairline between the form and the "switch to the other view" footer.
fn rule(commands: &mut Commands) -> Entity {
    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(1.0),
                margin: UiRect::vertical(Val::Px(2.0)),
                ..default()
            },
            BackgroundColor(rgb(divider())),
        ))
        .id()
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
    get: fn(&Rx) -> String,
    set: fn(&mut World, String),
    password: bool,
    first: bool,
) -> Entity {
    let col = commands.spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(5.0), ..default() }).id();
    let lbl = text_node(commands, fonts, label, 12.0, text_muted());
    let input = if password {
        password_input(commands, &fonts.ui, placeholder, "")
    } else {
        text_input(commands, &fonts.ui, placeholder, "")
    };
    // Replacing the widget's own `Node` wholesale, which is why every property it
    // set is restated here: the shared input is sized for a dense inspector row,
    // and this is a standalone form where a 28px box looks cramped.
    //
    // The horizontal padding must stay `PAD_X`. The caret and the click-to-caret
    // math measure from that constant rather than from the box's real padding,
    // so widening it here drew the caret to the left of the first character and
    // put every click a couple of characters off.
    commands.entity(input).insert(Node {
        width: Val::Percent(100.0),
        height: Val::Px(38.0),
        align_items: AlignItems::Center,
        padding: UiRect::horizontal(Val::Px(renzora_ember::widgets::PAD_X)),
        border: UiRect::all(Val::Px(1.0)),
        border_radius: BorderRadius::all(Val::Px(7.0)),
        overflow: bevy::ui::Overflow::clip(),
        ..default()
    });
    if first {
        commands.entity(input).insert(AuthFirstField);
    }
    bind_text_input(commands, input, get, set);
    commands.entity(col).add_children(&[lbl, input]);
    col
}

/// The primary action. Dimmed while a request is in flight, so "Signing in..."
/// looks like a button that is busy rather than one you should press again.
fn submit_button(commands: &mut Commands, fonts: &EmberFonts, text: &str, loading: bool) -> Entity {
    let (r, g, b) = accent();
    let fill = if loading {
        Color::srgba_u8(r, g, b, 150)
    } else {
        rgb(accent())
    };
    let btn = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(42.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border_radius: BorderRadius::all(Val::Px(8.0)),
                ..default()
            },
            BackgroundColor(fill),
            Interaction::default(),
            AuthSubmit,
            Name::new("auth-submit"),
        ))
        .id();
    let t = commands.spawn((Text::new(text.to_string()), ui_font(&fonts.ui, 14.5), TextColor(Color::WHITE), FocusPolicy::Pass)).id();
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
        kids.push(text_node(commands, fonts, p, 12.5, text_muted()));
    }
    let link_e = commands
        .spawn((
            Text::new(link.to_string()),
            ui_font(&fonts.ui, 12.5),
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

fn g_email(w: &Rx) -> String { w.get_resource::<AuthState>().map(|a| a.email.clone()).unwrap_or_default() }
fn s_email(w: &mut World, v: String) { if let Some(mut a) = w.get_resource_mut::<AuthState>() { a.email = v; } }
fn g_password(w: &Rx) -> String { w.get_resource::<AuthState>().map(|a| a.password.clone()).unwrap_or_default() }
fn s_password(w: &mut World, v: String) { if let Some(mut a) = w.get_resource_mut::<AuthState>() { a.password = v; } }
fn g_username(w: &Rx) -> String { w.get_resource::<AuthState>().map(|a| a.username.clone()).unwrap_or_default() }
fn s_username(w: &mut World, v: String) { if let Some(mut a) = w.get_resource_mut::<AuthState>() { a.username = v; } }
fn g_confirm(w: &Rx) -> String { w.get_resource::<AuthState>().map(|a| a.confirm_password.clone()).unwrap_or_default() }
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
    // Set when a request actually spawns (not on validation errors): the
    // password fields then clear like a normal form, while email/username stay
    // for a retry.
    let mut clear_passwords = false;
    {
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
                clear_passwords = true;
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
                    clear_passwords = true;
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
        if clear_passwords {
            auth.password.clear();
            auth.confirm_password.clear();
        }
    }
    if clear_passwords {
        // Also clear the widgets, not just the state: a focused password field
        // would otherwise push its old value straight back through its binding.
        let mut q = world.query::<&mut EmberTextInput>();
        for mut inp in q.iter_mut(world) {
            if inp.password && !inp.value.is_empty() {
                inp.value.clear();
                inp.caret_index = 0;
            }
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

/// The header's X closes the modal, the same way Escape does.
fn auth_close_click(
    q: Query<&Interaction, (With<AuthClose>, Changed<Interaction>)>,
    mut auth: Option<ResMut<AuthState>>,
) {
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
