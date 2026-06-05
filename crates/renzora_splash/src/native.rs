//! Bevy-native (ember) splash launcher — the bevy_ui counterpart to the egui
//! `render_splash` in [`crate::ui`]. Same structure: a borderless window chrome
//! strip, a sign-in / welcome card, a roadmap card, a projects + recents card,
//! and a bottom social bar. It drives the same `SplashAuth`, `AppConfig`,
//! `GithubStats` and project open/create APIs the egui path uses.
//!
//! Unlike the editor's native panels (which gate on `EditorUiBackend`), the
//! splash runs at startup *before* the F10 backend toggle is meaningful and the
//! backend defaults to `Egui`. So this renders unconditionally while in
//! [`SplashState::Splash`], and the egui splash only paints as a fallback until
//! the native root has spawned (see `native_splash_absent`).
//!
//! The animated synthwave background and the loading rosette are pure egui
//! painter line-art and have no plain-bevy_ui equivalent; the native splash
//! uses a static dark backdrop instead.

use bevy::ecs::world::CommandQueue;
use bevy::math::CompassOctant;
use bevy::prelude::*;
use bevy::ui::FocusPolicy;
use bevy::window::SystemCursorIcon;

use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::reactive::{bind_bg, bind_text, keyed_list, react, KeyedSnapshot};
use renzora_ember::widgets::{bind_text_input, password_input, text_input};
use renzora_hui::cursor_icon::HoverCursor;
use renzora_ui::window_chrome::{WindowAction, WindowActionQueue};

use crate::auth::SplashAuth;
use crate::config::AppConfig;
use crate::github::{format_count, GithubStats};
#[cfg(not(target_arch = "wasm32"))]
use crate::project::create_project;
use crate::project::open_project;
use crate::SplashState;

// ── Palette (mirrors crate::ui's egui colours) ───────────────────────────────

fn c(r: u8, g: u8, b: u8) -> Color {
    Color::srgb_u8(r, g, b)
}
fn ca(r: u8, g: u8, b: u8, a: u8) -> Color {
    Color::srgba_u8(r, g, b, a)
}

fn bg_color() -> Color {
    c(5, 4, 10)
}
fn panel_bg() -> Color {
    ca(18, 20, 30, 235)
}
fn panel_hover() -> Color {
    ca(26, 30, 46, 245)
}
fn border() -> Color {
    c(48, 54, 74)
}
fn border_soft() -> Color {
    c(36, 40, 56)
}
fn text() -> Color {
    c(224, 228, 240)
}
fn text_muted() -> Color {
    c(130, 138, 160)
}
fn accent() -> Color {
    c(110, 150, 255)
}
fn accent_hover() -> Color {
    c(140, 175, 255)
}
fn error_color() -> Color {
    c(239, 68, 68)
}
fn white() -> Color {
    Color::WHITE
}

const VERSION: &str = "r1-alpha5";
const WEBSITE_URL: &str = "https://renzora.com";
const YOUTUBE_URL: &str = "https://youtube.com/@renzoragame";
const DISCORD_URL: &str = "https://discord.gg/9UHUGUyDJv";
const GITHUB_URL: &str = "https://github.com/renzora/engine";
const ROADMAP_URL: &str = "https://github.com/renzora/engine/blob/main/docs/roadmap.md";

const ROADMAP_ITEMS: &[&str] = &[
    "Prefab / template system for reusable entities",
    "Batch property editing across multiple entities",
    "Property-level undo/redo history in inspector",
    "Advanced asset search (regex, type, size filters)",
    "File tagging and categories in asset browser",
];

// ── Markers / resources ──────────────────────────────────────────────────────

#[derive(Component)]
pub(crate) struct SplashRoot;
#[derive(Component)]
struct SplashDragHandle;
#[derive(Component, Clone, Copy)]
enum WinBtn {
    Min,
    Max,
    Close,
}
#[derive(Component)]
struct SplashWinBtn(WinBtn);
#[derive(Component)]
struct SplashResizeZone(CompassOctant);
#[derive(Component)]
struct SignInBody {
    sig: Option<u64>,
}
#[derive(Component)]
struct SignInSubmit;
#[derive(Component)]
struct SignOutBtn;
#[derive(Component)]
struct NewProjectBtn;
#[derive(Component)]
struct OpenProjectBtn;
#[derive(Component)]
struct RecentsContainer;
#[derive(Component, Clone)]
struct RecentOpen(std::path::PathBuf);
#[derive(Component, Clone)]
struct RecentRemove(std::path::PathBuf);
#[derive(Component, Clone)]
struct SplashUrl(String);

/// The "New project name" text the user has typed (the egui path kept this in
/// egui memory; bevy_ui needs a real resource to bind the input to).
#[derive(Resource, Default)]
struct SplashNewName(String);

pub(crate) fn register(app: &mut App) {
    app.init_resource::<SplashNewName>().add_systems(
        Update,
        (
            native_reopen,
            native_splash_poll.run_if(in_state(SplashState::Splash)),
            manage_splash,
            rebuild_signin,
            window_btn_click,
            drag_handle,
            resize_zone_click,
            signin_submit_click,
            signout_click,
            new_project_click,
            open_project_click,
            recent_open_click,
            recent_remove_click,
            url_click,
        ),
    );
}

/// Run condition for the egui fallback: true until the native root exists.
pub(crate) fn native_splash_absent(q: Query<(), With<SplashRoot>>) -> bool {
    q.is_empty()
}

/// Mirror of `splash_ui_system`'s reopen guard: when the editor's File→Open
/// inserts `PendingProjectReopen`, jump straight to Loading (the egui guard is
/// gated off once the native root is active).
fn native_reopen(
    mut commands: Commands,
    reopen: Option<Res<crate::PendingProjectReopen>>,
    mut next_state: ResMut<NextState<SplashState>>,
) {
    if reopen.is_some() {
        commands.remove_resource::<crate::PendingProjectReopen>();
        next_state.set(SplashState::Loading);
    }
}

// ── Async polling (egui did this inside render_splash) ───────────────────────

fn native_splash_poll(mut auth: ResMut<SplashAuth>, mut stats: ResMut<GithubStats>) {
    auth.poll();
    stats.poll();
}

// ── Lifecycle ────────────────────────────────────────────────────────────────

fn manage_splash(world: &mut World) {
    let want = matches!(
        world.resource::<State<SplashState>>().get(),
        SplashState::Splash
    );
    let mut q = world.query_filtered::<Entity, With<SplashRoot>>();
    let existing: Vec<Entity> = q.iter(world).collect();

    if want && existing.is_empty() {
        if world.get_resource::<EmberFonts>().is_none() {
            return; // fonts not ready yet — egui fallback covers this frame
        }
        let fonts = world.resource::<EmberFonts>().clone();
        let mut queue = CommandQueue::default();
        {
            let mut commands = Commands::new(&mut queue, world);
            spawn_splash(&mut commands, &fonts);
        }
        queue.apply(world);
    } else if !want && !existing.is_empty() {
        for e in existing {
            world.entity_mut(e).despawn();
        }
    }
}

fn spawn_splash(commands: &mut Commands, fonts: &EmberFonts) {
    let root = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                right: Val::Px(0.0),
                bottom: Val::Px(0.0),
                flex_direction: FlexDirection::Column,
                ..default()
            },
            BackgroundColor(bg_color()),
            GlobalZIndex(500),
            FocusPolicy::Block,
            SplashRoot,
            Name::new("splash-root"),
        ))
        .id();

    // Animated synthwave background — first child so it paints behind the rest.
    let backdrop = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                right: Val::Px(0.0),
                bottom: Val::Px(0.0),
                ..default()
            },
            FocusPolicy::Pass,
            crate::native_bg::BgBackground,
            Name::new("splash-bg"),
        ))
        .id();

    let chrome = build_chrome(commands, fonts);
    let content = build_content(commands, fonts);
    let bottom = build_bottom_bar(commands, fonts);
    commands
        .entity(root)
        .add_children(&[backdrop, chrome, content, bottom]);

    // Resize zones float above everything (drawn last, win hit-testing).
    build_resize_zones(commands, root);
}

// ── Window chrome strip ──────────────────────────────────────────────────────

fn build_chrome(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let bar = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(36.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                border: UiRect::bottom(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(c(12, 14, 22)),
            BorderColor::all(border_soft()),
            Name::new("splash-chrome"),
        ))
        .id();

    let drag = commands
        .spawn((
            Node {
                flex_grow: 1.0,
                height: Val::Percent(100.0),
                ..default()
            },
            Interaction::default(),
            SplashDragHandle,
            HoverCursor(SystemCursorIcon::Grab),
            Name::new("splash-drag"),
        ))
        .id();

    let min = win_button(commands, fonts, WinBtn::Min, "minus", false);
    let max = win_button(commands, fonts, WinBtn::Max, "square", false);
    let close = win_button(commands, fonts, WinBtn::Close, "x", true);

    commands.entity(bar).add_children(&[drag, min, max, close]);
    bar
}

fn win_button(
    commands: &mut Commands,
    fonts: &EmberFonts,
    kind: WinBtn,
    icon: &str,
    is_close: bool,
) -> Entity {
    let btn = commands
        .spawn((
            Node {
                width: Val::Px(40.0),
                height: Val::Percent(100.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            BackgroundColor(Color::NONE),
            Interaction::default(),
            SplashWinBtn(kind),
            HoverCursor(SystemCursorIcon::Pointer),
            Name::new("splash-win-btn"),
        ))
        .id();
    bind_bg(commands, btn, move |w| {
        let hov = matches!(
            w.get::<Interaction>(btn),
            Some(Interaction::Hovered) | Some(Interaction::Pressed)
        );
        if hov {
            if is_close {
                c(232, 17, 35)
            } else {
                ca(255, 255, 255, 34)
            }
        } else {
            Color::NONE
        }
    });

    let glyph = icon_text(commands, &fonts.phosphor, icon, (224, 228, 240), 14.0);
    commands.entity(glyph).insert(FocusPolicy::Pass);
    // The maximize button's icon flips with the window's maximized state.
    if matches!(kind, WinBtn::Max) {
        let square = renzora_ember::font::icon_glyph("square").unwrap_or('\u{E4C6}');
        let restore = renzora_ember::font::icon_glyph("arrows-in-simple").unwrap_or('\u{E4C6}');
        bind_text(commands, glyph, move |w| {
            let maxed = w
                .get_resource::<WindowActionQueue>()
                .map(|q| q.maximized)
                .unwrap_or(false);
            (if maxed { restore } else { square }).to_string()
        });
    }
    commands.entity(btn).add_child(glyph);
    btn
}

fn build_resize_zones(commands: &mut Commands, root: Entity) {
    let t = Val::Px(8.0);
    let cz = Val::Px(16.0);
    // (octant, left, right, top, bottom, width, height, cursor)
    let edges: [(CompassOctant, Edge); 8] = [
        (CompassOctant::North, Edge::horiz_top(t)),
        (CompassOctant::South, Edge::horiz_bottom(t)),
        (CompassOctant::West, Edge::vert_left(t)),
        (CompassOctant::East, Edge::vert_right(t)),
        (CompassOctant::NorthWest, Edge::corner(true, true, cz)),
        (CompassOctant::NorthEast, Edge::corner(false, true, cz)),
        (CompassOctant::SouthWest, Edge::corner(true, false, cz)),
        (CompassOctant::SouthEast, Edge::corner(false, false, cz)),
    ];
    for (octant, e) in edges {
        let cursor = resize_cursor(octant);
        let zone = commands
            .spawn((
                e.into_node(),
                BackgroundColor(Color::NONE),
                GlobalZIndex(560),
                Interaction::default(),
                SplashResizeZone(octant),
                HoverCursor(cursor),
                Name::new("splash-resize"),
            ))
            .id();
        commands.entity(root).add_child(zone);
    }
}

/// Absolute-position spec for a resize hit-zone.
struct Edge {
    left: Val,
    right: Val,
    top: Val,
    bottom: Val,
    width: Val,
    height: Val,
}
impl Edge {
    fn horiz_top(t: Val) -> Self {
        Self { left: Val::Px(16.0), right: Val::Px(16.0), top: Val::Px(0.0), bottom: Val::Auto, width: Val::Auto, height: t }
    }
    fn horiz_bottom(t: Val) -> Self {
        Self { left: Val::Px(16.0), right: Val::Px(16.0), top: Val::Auto, bottom: Val::Px(0.0), width: Val::Auto, height: t }
    }
    fn vert_left(t: Val) -> Self {
        Self { left: Val::Px(0.0), right: Val::Auto, top: Val::Px(16.0), bottom: Val::Px(16.0), width: t, height: Val::Auto }
    }
    fn vert_right(t: Val) -> Self {
        Self { left: Val::Auto, right: Val::Px(0.0), top: Val::Px(16.0), bottom: Val::Px(16.0), width: t, height: Val::Auto }
    }
    fn corner(left_side: bool, top_side: bool, cz: Val) -> Self {
        Self {
            left: if left_side { Val::Px(0.0) } else { Val::Auto },
            right: if left_side { Val::Auto } else { Val::Px(0.0) },
            top: if top_side { Val::Px(0.0) } else { Val::Auto },
            bottom: if top_side { Val::Auto } else { Val::Px(0.0) },
            width: cz,
            height: cz,
        }
    }
    fn into_node(self) -> Node {
        Node {
            position_type: PositionType::Absolute,
            left: self.left,
            right: self.right,
            top: self.top,
            bottom: self.bottom,
            width: self.width,
            height: self.height,
            ..default()
        }
    }
}

fn resize_cursor(octant: CompassOctant) -> SystemCursorIcon {
    match octant {
        CompassOctant::North | CompassOctant::South => SystemCursorIcon::NsResize,
        CompassOctant::East | CompassOctant::West => SystemCursorIcon::EwResize,
        CompassOctant::NorthWest | CompassOctant::SouthEast => SystemCursorIcon::NwseResize,
        CompassOctant::NorthEast | CompassOctant::SouthWest => SystemCursorIcon::NeswResize,
    }
}

// ── Centre content: left column (sign-in + roadmap), right column (projects) ─

fn build_content(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let area = commands
        .spawn((
            Node {
                flex_grow: 1.0,
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                column_gap: Val::Px(16.0),
                padding: UiRect::all(Val::Px(24.0)),
                ..default()
            },
            Name::new("splash-content"),
        ))
        .id();

    let left = commands
        .spawn(Node {
            width: Val::Px(340.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(16.0),
            ..default()
        })
        .id();
    let signin = build_signin_card(commands, fonts);
    let roadmap = build_roadmap_card(commands, fonts);
    commands.entity(left).add_children(&[signin, roadmap]);

    let projects = build_projects_card(commands, fonts);
    commands.entity(area).add_children(&[left, projects]);
    area
}

fn card(commands: &mut Commands) -> Entity {
    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(22.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(10.0)),
                row_gap: Val::Px(8.0),
                ..default()
            },
            BackgroundColor(panel_bg()),
            BorderColor::all(border()),
        ))
        .id()
}

fn label(commands: &mut Commands, fonts: &EmberFonts, txt: &str, size: f32, color: Color) -> Entity {
    commands
        .spawn((
            Text::new(txt.to_string()),
            ui_font(&fonts.ui, size),
            TextColor(color),
        ))
        .id()
}

// ── Sign-in card ─────────────────────────────────────────────────────────────

fn build_signin_card(commands: &mut Commands, _fonts: &EmberFonts) -> Entity {
    let card = card(commands);
    let body = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(8.0),
                ..default()
            },
            SignInBody { sig: None },
        ))
        .id();
    commands.entity(card).add_child(body);
    card
}

fn signin_sig(auth: &SplashAuth) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    auth.is_signed_in().hash(&mut h);
    if let Some(u) = &auth.user {
        u.username.hash(&mut h);
        u.email.hash(&mut h);
        u.role.hash(&mut h);
        u.credit_balance.hash(&mut h);
    }
    auth.error.hash(&mut h);
    auth.loading.hash(&mut h);
    h.finish()
}

fn rebuild_signin(world: &mut World) {
    if world.query_filtered::<(), With<SplashRoot>>().iter(world).next().is_none() {
        return;
    }
    let Some(fonts) = world.get_resource::<EmberFonts>().cloned() else {
        return;
    };
    let sig = world.get_resource::<SplashAuth>().map(signin_sig);
    let Some(sig) = sig else { return };

    let mut q = world.query::<(Entity, &SignInBody)>();
    let Some((body, old)) = q.iter(world).map(|(e, b)| (e, b.sig)).next() else {
        return;
    };
    if old == Some(sig) {
        return;
    }

    let existing: Vec<Entity> = world
        .get::<Children>(body)
        .map(|c| c.iter().collect())
        .unwrap_or_default();
    let mut queue = CommandQueue::default();
    {
        let mut commands = Commands::new(&mut queue, world);
        for ch in existing {
            commands.entity(ch).despawn();
        }
        build_signin_body(&mut commands, &fonts, body, world.resource::<SplashAuth>());
    }
    queue.apply(world);
    if let Some(mut b) = world.get_mut::<SignInBody>(body) {
        b.sig = Some(sig);
    }
}

fn build_signin_body(commands: &mut Commands, fonts: &EmberFonts, body: Entity, auth: &SplashAuth) {
    if let Some(user) = auth.user.clone() {
        // ── Signed-in welcome ──
        let mut kids = vec![
            label(commands, fonts, "SIGNED IN", 10.0, text_muted()),
            label(commands, fonts, &format!("Welcome, {}", user.username), 18.0, white()),
            label(commands, fonts, &user.email, 11.0, text_muted()),
        ];

        // Credits pill.
        let pill = commands
            .spawn((
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Px(48.0),
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::SpaceBetween,
                    padding: UiRect::horizontal(Val::Px(14.0)),
                    margin: UiRect::vertical(Val::Px(8.0)),
                    border: UiRect::all(Val::Px(1.0)),
                    border_radius: BorderRadius::all(Val::Px(8.0)),
                    ..default()
                },
                BackgroundColor(ca(110, 150, 255, 26)),
                BorderColor::all(ca(110, 150, 255, 90)),
            ))
            .id();
        let credits_col = commands
            .spawn(Node { flex_direction: FlexDirection::Column, ..default() })
            .id();
        let cl = label(commands, fonts, "CREDITS", 10.0, text_muted());
        let cv = label(commands, fonts, &format!("{}", user.credit_balance), 15.0, white());
        commands.entity(credits_col).add_children(&[cl, cv]);
        let role = label(commands, fonts, &user.role.to_uppercase(), 10.5, accent());
        commands.entity(pill).add_children(&[credits_col, role]);
        kids.push(pill);

        // Sign-out button.
        let out = action_button(commands, fonts, "sign-out", "Sign out", false);
        commands.entity(out).insert(SignOutBtn);
        kids.push(out);

        commands.entity(body).add_children(&kids);
        return;
    }

    // ── Sign-in form ──
    let mut kids = vec![
        label(commands, fonts, "SIGN IN", 10.0, text_muted()),
        label(commands, fonts, "Welcome back to Renzora", 16.0, white()),
        label(commands, fonts, "Email", 11.0, text_muted()),
    ];

    let email = text_input(commands, &fonts.ui, "you@example.com", &auth.email);
    style_field(commands, email);
    bind_text_input(commands, email, g_email, s_email);
    kids.push(email);

    kids.push(label(commands, fonts, "Password", 11.0, text_muted()));
    let pw = password_input(commands, &fonts.ui, "Password", &auth.password);
    style_field(commands, pw);
    bind_text_input(commands, pw, g_password, s_password);
    kids.push(pw);

    if let Some(err) = &auth.error {
        kids.push(label(commands, fonts, err, 11.0, error_color()));
    }

    let submit = action_button(
        commands,
        fonts,
        "sign-in",
        if auth.loading { "Signing in…" } else { "Sign In" },
        true,
    );
    commands.entity(submit).insert(SignInSubmit);
    kids.push(submit);

    // "No account? Create one   Forgot password?"
    let links = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::SpaceBetween,
            ..default()
        })
        .id();
    let left = commands
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(5.0),
            ..default()
        })
        .id();
    let no_acct = label(commands, fonts, "No account?", 11.0, text_muted());
    let create = link_text(commands, fonts, "Create one", accent(), &format!("{WEBSITE_URL}/register"));
    commands.entity(left).add_children(&[no_acct, create]);
    let forgot = link_text(commands, fonts, "Forgot password?", text_muted(), &format!("{WEBSITE_URL}/forgot"));
    commands.entity(links).add_children(&[left, forgot]);
    kids.push(links);

    commands.entity(body).add_children(&kids);
}

fn style_field(commands: &mut Commands, input: Entity) {
    commands.entity(input).insert(Node {
        width: Val::Percent(100.0),
        height: Val::Px(34.0),
        align_items: AlignItems::Center,
        padding: UiRect::horizontal(Val::Px(12.0)),
        border: UiRect::all(Val::Px(1.0)),
        border_radius: BorderRadius::all(Val::Px(6.0)),
        ..default()
    });
}

/// A clickable text link that opens `url`.
fn link_text(commands: &mut Commands, fonts: &EmberFonts, txt: &str, color: Color, url: &str) -> Entity {
    commands
        .spawn((
            Text::new(txt.to_string()),
            ui_font(&fonts.ui, 11.0),
            TextColor(color),
            Interaction::default(),
            SplashUrl(url.to_string()),
            HoverCursor(SystemCursorIcon::Pointer),
        ))
        .id()
}

/// A full-width button with a leading phosphor icon. `primary` paints it accent.
fn action_button(commands: &mut Commands, fonts: &EmberFonts, icon: &str, txt: &str, primary: bool) -> Entity {
    let btn = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(38.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                column_gap: Val::Px(8.0),
                margin: UiRect::vertical(Val::Px(4.0)),
                border: if primary { UiRect::ZERO } else { UiRect::all(Val::Px(1.0)) },
                border_radius: BorderRadius::all(Val::Px(6.0)),
                ..default()
            },
            BackgroundColor(if primary { accent() } else { ca(255, 255, 255, 12) }),
            BorderColor::all(if primary { Color::NONE } else { border() }),
            Interaction::default(),
            HoverCursor(SystemCursorIcon::Pointer),
        ))
        .id();
    if primary {
        bind_bg(commands, btn, move |w| {
            let hov = matches!(
                w.get::<Interaction>(btn),
                Some(Interaction::Hovered) | Some(Interaction::Pressed)
            );
            if hov { accent_hover() } else { accent() }
        });
    } else {
        bind_bg(commands, btn, move |w| {
            let hov = matches!(
                w.get::<Interaction>(btn),
                Some(Interaction::Hovered) | Some(Interaction::Pressed)
            );
            if hov { ca(255, 255, 255, 24) } else { ca(255, 255, 255, 12) }
        });
    }
    let ic = icon_text(commands, &fonts.phosphor, icon, if primary { (255, 255, 255) } else { (224, 228, 240) }, 14.0);
    commands.entity(ic).insert(FocusPolicy::Pass);
    let t = commands
        .spawn((
            Text::new(txt.to_string()),
            ui_font(&fonts.ui, 13.0),
            TextColor(if primary { white() } else { text() }),
            FocusPolicy::Pass,
        ))
        .id();
    commands.entity(btn).add_children(&[ic, t]);
    btn
}

// ── Roadmap card ─────────────────────────────────────────────────────────────

fn build_roadmap_card(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let card = card(commands);
    let header = commands
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(6.0),
            ..default()
        })
        .id();
    let rocket = icon_text(commands, &fonts.phosphor, "rocket", (110, 150, 255), 14.0);
    let tag = label(commands, fonts, "ROADMAP", 10.0, text_muted());
    commands.entity(header).add_children(&[rocket, tag]);

    let title = label(commands, fonts, "What's coming next", 14.0, white());

    let mut kids = vec![header, title];
    for item in ROADMAP_ITEMS {
        let row = commands
            .spawn(Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(10.0),
                height: Val::Px(22.0),
                ..default()
            })
            .id();
        let dot = commands
            .spawn((
                Node { width: Val::Px(5.0), height: Val::Px(5.0), border_radius: BorderRadius::all(Val::Px(3.0)), ..default() },
                BackgroundColor(accent()),
            ))
            .id();
        let txt = label(commands, fonts, item, 12.5, text());
        commands.entity(row).add_children(&[dot, txt]);
        kids.push(row);
    }

    let view = action_button(commands, fonts, "arrow-square-out", "View full roadmap on GitHub", false);
    commands.entity(view).insert(SplashUrl(ROADMAP_URL.to_string()));
    kids.push(view);

    commands.entity(card).add_children(&kids);
    card
}

// ── Projects + recents card ──────────────────────────────────────────────────

fn build_projects_card(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let card = commands
        .spawn((
            Node {
                width: Val::Px(500.0),
                height: Val::Percent(100.0),
                max_height: Val::Px(576.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(22.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(10.0)),
                row_gap: Val::Px(12.0),
                ..default()
            },
            BackgroundColor(panel_bg()),
            BorderColor::all(border()),
        ))
        .id();

    // Header: PROJECTS … [+ New] [Open Project]
    let header = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::SpaceBetween,
            ..default()
        })
        .id();
    let tag = label(commands, fonts, "PROJECTS", 10.0, text_muted());
    let btns = commands
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(6.0),
            ..default()
        })
        .id();
    let new = compact_button(commands, fonts, "+ New", true);
    commands.entity(new).insert(NewProjectBtn);
    let open = compact_button(commands, fonts, "Open Project", false);
    commands.entity(open).insert(OpenProjectBtn);
    commands.entity(btns).add_children(&[new, open]);
    commands.entity(header).add_children(&[tag, btns]);

    // New-project name input.
    let name_input = text_input(commands, &fonts.ui, "New project name…", "");
    commands.entity(name_input).insert(Node {
        width: Val::Percent(100.0),
        height: Val::Px(38.0),
        align_items: AlignItems::Center,
        padding: UiRect::horizontal(Val::Px(12.0)),
        border: UiRect::all(Val::Px(1.0)),
        border_radius: BorderRadius::all(Val::Px(6.0)),
        ..default()
    });
    bind_text_input(commands, name_input, g_newname, s_newname);

    let recents_label = label(commands, fonts, "Recents", 15.0, white());

    // Scrollable recents list (keyed to recent_projects).
    let list = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(8.0),
                ..default()
            },
            RecentsContainer,
        ))
        .id();
    keyed_list(commands, list, recents_snapshot);
    let scroll = renzora_ember::widgets::scroll_view(commands, list);

    commands
        .entity(card)
        .add_children(&[header, name_input, recents_label, scroll]);
    card
}

fn compact_button(commands: &mut Commands, fonts: &EmberFonts, txt: &str, primary: bool) -> Entity {
    let btn = commands
        .spawn((
            Node {
                height: Val::Px(28.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                padding: UiRect::horizontal(Val::Px(12.0)),
                border: if primary { UiRect::ZERO } else { UiRect::all(Val::Px(1.0)) },
                border_radius: BorderRadius::all(Val::Px(6.0)),
                ..default()
            },
            BackgroundColor(if primary { accent() } else { ca(255, 255, 255, 10) }),
            BorderColor::all(if primary { Color::NONE } else { border() }),
            Interaction::default(),
            HoverCursor(SystemCursorIcon::Pointer),
        ))
        .id();
    bind_bg(commands, btn, move |w| {
        let hov = matches!(
            w.get::<Interaction>(btn),
            Some(Interaction::Hovered) | Some(Interaction::Pressed)
        );
        if primary {
            if hov { accent_hover() } else { accent() }
        } else if hov {
            ca(255, 255, 255, 22)
        } else {
            ca(255, 255, 255, 10)
        }
    });
    let t = commands
        .spawn((
            Text::new(txt.to_string()),
            ui_font(&fonts.ui, 11.5),
            TextColor(if primary { white() } else { text() }),
            FocusPolicy::Pass,
        ))
        .id();
    commands.entity(btn).add_child(t);
    btn
}

#[derive(Clone)]
struct RowData {
    name: String,
    path: std::path::PathBuf,
    path_display: String,
    exists: bool,
}

fn recent_rows(world: &World) -> Vec<RowData> {
    let Some(cfg) = world.get_resource::<AppConfig>() else {
        return Vec::new();
    };
    cfg.recent_projects
        .iter()
        .map(|p| {
            let name = p
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("Unknown Project")
                .to_string();
            let path_display = p.to_string_lossy().to_string();
            #[cfg(not(target_arch = "wasm32"))]
            let exists = p.join("project.toml").exists();
            #[cfg(target_arch = "wasm32")]
            let exists = true;
            RowData { name, path: p.clone(), path_display, exists }
        })
        .collect()
}

fn recents_snapshot(world: &World) -> KeyedSnapshot {
    use std::hash::{Hash, Hasher};
    let rows = recent_rows(world);
    let items: Vec<(u64, u64)> = rows
        .iter()
        .map(|r| {
            let mut k = std::collections::hash_map::DefaultHasher::new();
            r.path.hash(&mut k);
            let key = k.finish();
            let mut h = std::collections::hash_map::DefaultHasher::new();
            r.name.hash(&mut h);
            r.exists.hash(&mut h);
            (key, h.finish())
        })
        .collect();
    KeyedSnapshot {
        items,
        build: Box::new(move |commands, fonts, i| build_recent_row(commands, fonts, &rows[i])),
    }
}

fn build_recent_row(commands: &mut Commands, fonts: &EmberFonts, row: &RowData) -> Entity {
    let container = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(60.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::SpaceBetween,
                padding: UiRect::horizontal(Val::Px(14.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(8.0)),
                ..default()
            },
            BackgroundColor(ca(255, 255, 255, 8)),
            BorderColor::all(border_soft()),
            Interaction::default(),
        ))
        .id();
    if row.exists {
        commands
            .entity(container)
            .insert((RecentOpen(row.path.clone()), HoverCursor(SystemCursorIcon::Pointer)));
        let cc = container;
        bind_bg(commands, container, move |w| {
            let hov = matches!(
                w.get::<Interaction>(cc),
                Some(Interaction::Hovered) | Some(Interaction::Pressed)
            );
            if hov { panel_hover() } else { ca(255, 255, 255, 8) }
        });
    }

    let info = commands
        .spawn((
            Node { flex_direction: FlexDirection::Column, row_gap: Val::Px(4.0), ..default() },
            FocusPolicy::Pass,
        ))
        .id();
    let name_txt = if row.exists {
        row.name.clone()
    } else {
        format!("{}  (missing)", row.name)
    };
    let name_color = if row.exists { text() } else { text_muted() };
    let name = commands
        .spawn((Text::new(name_txt), ui_font(&fonts.ui, 14.0), TextColor(name_color), FocusPolicy::Pass))
        .id();
    let path_str = elide_path(&row.path_display, 60);
    let path = commands
        .spawn((Text::new(path_str), ui_font(&fonts.mono, 10.0), TextColor(text_muted()), FocusPolicy::Pass))
        .id();
    commands.entity(info).add_children(&[name, path]);
    commands.entity(container).add_child(info);

    if !row.exists {
        let remove = commands
            .spawn((
                Node {
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    padding: UiRect::axes(Val::Px(8.0), Val::Px(4.0)),
                    border_radius: BorderRadius::all(Val::Px(4.0)),
                    ..default()
                },
                Interaction::default(),
                RecentRemove(row.path.clone()),
                HoverCursor(SystemCursorIcon::Pointer),
            ))
            .id();
        let rt = commands
            .spawn((Text::new("Remove".to_string()), ui_font(&fonts.ui, 10.5), TextColor(text_muted()), FocusPolicy::Pass))
            .id();
        bind_text_color(commands, rt, remove);
        commands.entity(remove).add_child(rt);
        commands.entity(container).add_child(remove);
    }

    container
}

/// Tint the Remove label red on hover (the button entity owns the Interaction).
fn bind_text_color(commands: &mut Commands, text_e: Entity, btn: Entity) {
    react(commands, move |world: &mut World| {
        if world.get_entity(text_e).is_err() || world.get_entity(btn).is_err() {
            return false;
        }
        let hov = matches!(
            world.get::<Interaction>(btn),
            Some(Interaction::Hovered) | Some(Interaction::Pressed)
        );
        let col = if hov { error_color() } else { text_muted() };
        if let Some(mut c) = world.get_mut::<TextColor>(text_e) {
            c.0 = col;
        }
        true
    });
}

fn elide_path(s: &str, max: usize) -> String {
    if s.chars().count() > max {
        let tail: String = s.chars().rev().take(max).collect::<Vec<_>>().into_iter().rev().collect();
        format!("…{tail}")
    } else {
        s.to_string()
    }
}

// ── Bottom bar ───────────────────────────────────────────────────────────────

fn build_bottom_bar(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let bar = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(56.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::SpaceBetween,
                padding: UiRect::horizontal(Val::Px(24.0)),
                border: UiRect::top(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(ca(8, 10, 18, 220)),
            BorderColor::all(border_soft()),
            Name::new("splash-bottom"),
        ))
        .id();

    // Left: wordmark + version pill.
    let left = commands
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(10.0),
            ..default()
        })
        .id();
    let wordmark = label(commands, fonts, "Renzora Engine", 13.0, white());
    let pill = commands
        .spawn((
            Node {
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                padding: UiRect::axes(Val::Px(7.0), Val::Px(2.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(9.0)),
                ..default()
            },
            BackgroundColor(ca(110, 150, 255, 40)),
            BorderColor::all(ca(110, 150, 255, 140)),
        ))
        .id();
    let pill_t = commands
        .spawn((Text::new(VERSION.to_string()), ui_font(&fonts.mono, 10.5), TextColor(accent())))
        .id();
    commands.entity(pill).add_child(pill_t);
    commands.entity(left).add_children(&[wordmark, pill]);

    // Right: social pills.
    let right = commands
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(8.0),
            ..default()
        })
        .id();
    let website = social_button(commands, fonts, "globe", "Website", WEBSITE_URL, false);
    let youtube = social_button(commands, fonts, "youtube-logo", "YouTube", YOUTUBE_URL, false);
    let discord = social_button(commands, fonts, "discord-logo", "Discord", DISCORD_URL, false);
    let star = social_button(commands, fonts, "star", "Star us on GitHub", GITHUB_URL, true);
    commands.entity(right).add_children(&[website, youtube, discord, star]);

    commands.entity(bar).add_children(&[left, right]);
    bar
}

fn social_button(commands: &mut Commands, fonts: &EmberFonts, icon: &str, txt: &str, url: &str, starred: bool) -> Entity {
    let btn = commands
        .spawn((
            Node {
                height: Val::Px(30.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                column_gap: Val::Px(6.0),
                padding: UiRect::horizontal(Val::Px(12.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(7.0)),
                ..default()
            },
            BackgroundColor(ca(255, 255, 255, 12)),
            BorderColor::all(border_soft()),
            Interaction::default(),
            SplashUrl(url.to_string()),
            HoverCursor(SystemCursorIcon::Pointer),
        ))
        .id();
    bind_bg(commands, btn, move |w| {
        let hov = matches!(
            w.get::<Interaction>(btn),
            Some(Interaction::Hovered) | Some(Interaction::Pressed)
        );
        if hov { ca(255, 255, 255, 26) } else { ca(255, 255, 255, 12) }
    });
    let col = if starred { (235, 195, 80) } else { (224, 228, 240) };
    let ic = icon_text(commands, &fonts.phosphor, icon, col, 13.0);
    commands.entity(ic).insert(FocusPolicy::Pass);
    let t = commands
        .spawn((
            Text::new(txt.to_string()),
            ui_font(&fonts.ui, 12.5),
            TextColor(if starred { c(235, 195, 80) } else { text() }),
            FocusPolicy::Pass,
        ))
        .id();
    commands.entity(btn).add_children(&[ic, t]);
    // The GitHub star button shows the live star count once it loads.
    if starred {
        bind_text(commands, t, |w| {
            let stars = w.get_resource::<GithubStats>().and_then(|s| s.stars);
            match stars {
                Some(n) => format!("Star us on GitHub  ({})", format_count(n)),
                None => "Star us on GitHub".to_string(),
            }
        });
    }
    btn
}

// ── Field accessors ──────────────────────────────────────────────────────────

fn g_email(w: &World) -> String {
    w.get_resource::<SplashAuth>().map(|a| a.email.clone()).unwrap_or_default()
}
fn s_email(w: &mut World, v: String) {
    if let Some(mut a) = w.get_resource_mut::<SplashAuth>() {
        a.email = v;
    }
}
fn g_password(w: &World) -> String {
    w.get_resource::<SplashAuth>().map(|a| a.password.clone()).unwrap_or_default()
}
fn s_password(w: &mut World, v: String) {
    if let Some(mut a) = w.get_resource_mut::<SplashAuth>() {
        a.password = v;
    }
}
fn g_newname(w: &World) -> String {
    w.get_resource::<SplashNewName>().map(|n| n.0.clone()).unwrap_or_default()
}
fn s_newname(w: &mut World, v: String) {
    if let Some(mut n) = w.get_resource_mut::<SplashNewName>() {
        n.0 = v;
    }
}

// ── Interaction systems ──────────────────────────────────────────────────────

fn window_btn_click(
    q: Query<(&Interaction, &SplashWinBtn), Changed<Interaction>>,
    queue: Option<ResMut<WindowActionQueue>>,
) {
    let Some(mut queue) = queue else { return };
    for (interaction, btn) in &q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        queue.push(match btn.0 {
            WinBtn::Min => WindowAction::Minimize,
            WinBtn::Max => WindowAction::ToggleMaximize,
            WinBtn::Close => WindowAction::Close,
        });
    }
}

fn drag_handle(
    q: Query<&Interaction, (With<SplashDragHandle>, Changed<Interaction>)>,
    queue: Option<ResMut<WindowActionQueue>>,
) {
    let Some(mut queue) = queue else { return };
    if q.iter().any(|i| *i == Interaction::Pressed) {
        queue.push(WindowAction::StartDrag);
    }
}

fn resize_zone_click(
    q: Query<(&Interaction, &SplashResizeZone), Changed<Interaction>>,
    queue: Option<ResMut<WindowActionQueue>>,
) {
    let Some(mut queue) = queue else { return };
    for (interaction, zone) in &q {
        if *interaction == Interaction::Pressed {
            queue.push(WindowAction::StartResize(zone.0));
        }
    }
}

fn signin_submit_click(q: Query<&Interaction, (With<SignInSubmit>, Changed<Interaction>)>, mut commands: Commands) {
    if q.iter().any(|i| *i == Interaction::Pressed) {
        commands.queue(do_signin);
    }
}

fn do_signin(world: &mut World) {
    let Some(mut auth) = world.get_resource_mut::<SplashAuth>() else { return };
    if auth.loading {
        return;
    }
    if auth.email.trim().is_empty() || auth.password.is_empty() {
        auth.error = Some("Email and password are required".into());
    } else {
        auth.start_login();
    }
}

fn signout_click(q: Query<&Interaction, (With<SignOutBtn>, Changed<Interaction>)>, mut auth: Option<ResMut<SplashAuth>>) {
    let Some(auth) = auth.as_mut() else { return };
    if q.iter().any(|i| *i == Interaction::Pressed) {
        auth.sign_out();
    }
}

fn new_project_click(q: Query<&Interaction, (With<NewProjectBtn>, Changed<Interaction>)>, mut commands: Commands) {
    if q.iter().any(|i| *i == Interaction::Pressed) {
        commands.queue(do_new_project);
    }
}

fn open_project_click(q: Query<&Interaction, (With<OpenProjectBtn>, Changed<Interaction>)>, mut commands: Commands) {
    if q.iter().any(|i| *i == Interaction::Pressed) {
        commands.queue(do_open_project);
    }
}

fn recent_open_click(q: Query<(&Interaction, &RecentOpen), Changed<Interaction>>, mut commands: Commands) {
    for (interaction, open) in &q {
        if *interaction == Interaction::Pressed {
            let path = open.0.clone();
            commands.queue(move |world: &mut World| do_open_recent(world, &path));
        }
    }
}

fn recent_remove_click(q: Query<(&Interaction, &RecentRemove), Changed<Interaction>>, mut commands: Commands) {
    for (interaction, rm) in &q {
        if *interaction == Interaction::Pressed {
            let path = rm.0.clone();
            commands.queue(move |world: &mut World| {
                if let Some(mut cfg) = world.get_resource_mut::<AppConfig>() {
                    cfg.recent_projects.retain(|p| p != &path);
                    let _ = cfg.save();
                }
            });
        }
    }
}

fn url_click(q: Query<(&Interaction, &SplashUrl), Changed<Interaction>>) {
    for (interaction, url) in &q {
        if *interaction == Interaction::Pressed {
            open_url(&url.0);
        }
    }
}

// ── Project actions (queued so they run with &mut World) ─────────────────────

fn enter_project(world: &mut World, project: crate::project::CurrentProject) {
    if let Some(mut cfg) = world.get_resource_mut::<AppConfig>() {
        cfg.add_recent_project(project.path.clone());
        let _ = cfg.save();
    }
    world.insert_resource(project);
    world
        .resource_mut::<NextState<SplashState>>()
        .set(SplashState::Loading);
}

fn do_open_recent(world: &mut World, path: &std::path::Path) {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let toml = path.join("project.toml");
        match open_project(&toml) {
            Ok(p) => enter_project(world, p),
            Err(e) => error!("Failed to open project: {e}"),
        }
    }
    #[cfg(target_arch = "wasm32")]
    {
        let _ = (world, path);
    }
}

fn do_open_project(world: &mut World) {
    #[cfg(not(target_arch = "wasm32"))]
    {
        if let Some(file) = rfd::FileDialog::new()
            .set_title("Open Project")
            .add_filter("Project File", &["toml"])
            .pick_file()
        {
            match open_project(&file) {
                Ok(p) => enter_project(world, p),
                Err(e) => error!("Failed to open project: {e}"),
            }
        }
    }
    #[cfg(target_arch = "wasm32")]
    {
        let _ = world;
    }
}

fn do_new_project(world: &mut World) {
    let typed = world.get_resource::<SplashNewName>().map(|n| n.0.clone()).unwrap_or_default();
    let name = if typed.trim().is_empty() { "New Project".to_string() } else { typed.trim().to_string() };

    #[cfg(not(target_arch = "wasm32"))]
    {
        if let Some(folder) = rfd::FileDialog::new().set_title("Select Project Location").pick_folder() {
            let slug = name.replace(' ', "_").to_lowercase();
            let path = folder.join(&slug);
            match create_project(&path, &name) {
                Ok(p) => enter_project(world, p),
                Err(e) => error!("Failed to create project: {e}"),
            }
        }
    }
    #[cfg(target_arch = "wasm32")]
    {
        let _ = (world, name);
    }
}

fn open_url(url: &str) {
    #[cfg(not(target_arch = "wasm32"))]
    {
        #[cfg(target_os = "windows")]
        let _ = std::process::Command::new("cmd").args(["/C", "start", "", url]).spawn();
        #[cfg(target_os = "macos")]
        let _ = std::process::Command::new("open").arg(url).spawn();
        #[cfg(all(unix, not(target_os = "macos")))]
        let _ = std::process::Command::new("xdg-open").arg(url).spawn();
    }
    #[cfg(target_arch = "wasm32")]
    {
        let _ = url;
    }
}
