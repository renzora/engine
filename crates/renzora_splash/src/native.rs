//! Bevy-native (ember) splash launcher — an open, chrome-less project launcher
//! floating over the animated city + grid/network background: a search field at
//! top-centre, the "Renzora" title + a narrow recent-projects list in the
//! middle, and the New/Open actions + social links at bottom-centre. Window
//! controls float in the top-right; the whole background is a drag handle.
//!
//! Renders while in [`SplashState::Splash`].

use bevy::ecs::world::CommandQueue;
use bevy::math::CompassOctant;
use bevy::prelude::*;
use bevy::time::Real;
use bevy::ui::FocusPolicy;
use bevy::window::SystemCursorIcon;

use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::reactive::{bind_bg, bind_display, bind_text, bind_text_color, keyed_list, react, KeyedSnapshot};
use renzora_ember::widgets::{bind_text_input, text_input};
use renzora_ember::cursor_icon::HoverCursor;
use renzora_ui::window_chrome::{WindowAction, WindowActionQueue};

use crate::config::AppConfig;
use crate::github::{format_count, GithubStats};
#[cfg(not(target_arch = "wasm32"))]
use crate::project::create_project;
use crate::project::open_project;
use crate::SplashState;

// ── Palette ──────────────────────────────────────────────────────────────────

fn c(r: u8, g: u8, b: u8) -> Color {
    Color::srgb_u8(r, g, b)
}
fn ca(r: u8, g: u8, b: u8, a: u8) -> Color {
    Color::srgba_u8(r, g, b, a)
}

fn panel_hover() -> Color {
    ca(30, 34, 52, 250)
}
fn border_soft() -> Color {
    c(36, 40, 56)
}
fn btn_dark() -> Color {
    ca(12, 14, 22, 235)
}
fn btn_dark_hover() -> Color {
    ca(26, 30, 46, 245)
}
fn text() -> Color {
    c(224, 228, 240)
}
fn text_muted() -> Color {
    c(150, 158, 178)
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

const CONTENT_W: f32 = 460.0;

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

/// The recents search/filter text.
#[derive(Resource, Default)]
struct SplashFilter(String);

/// Smoothed real-time FPS shown in the splash corner. The splash is
/// GPU-light, so this is a baseline for "is the app/window itself smooth?"
/// to compare against the editor's much heavier per-frame render cost.
#[derive(Resource, Default)]
struct SplashFps(f32);

pub(crate) fn register(app: &mut App) {
    app.init_resource::<SplashFilter>().init_resource::<SplashFps>().add_systems(
        Update,
        (
            native_reopen,
            native_splash_poll.run_if(in_state(SplashState::Splash)),
            update_fps.run_if(in_state(SplashState::Splash)),
            manage_splash,
            window_btn_click,
            drag_handle,
            resize_zone_click,
            new_project_click,
            open_project_click,
            recent_open_click,
            recent_remove_click,
            url_click,
        ),
    );
}

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

fn native_splash_poll(mut stats: ResMut<GithubStats>) {
    stats.poll();
}

/// Exponentially-smoothed real FPS, updated only while the splash is shown.
fn update_fps(time: Res<Time<Real>>, mut fps: ResMut<SplashFps>) {
    let dt = time.delta_secs();
    if dt > 0.0 {
        let instant = 1.0 / dt;
        fps.0 = if fps.0 <= 0.0 { instant } else { fps.0 * 0.9 + instant * 0.1 };
    }
}

// ── Lifecycle ────────────────────────────────────────────────────────────────

fn manage_splash(world: &mut World) {
    let want = matches!(world.resource::<State<SplashState>>().get(), SplashState::Splash);
    let mut q = world.query_filtered::<Entity, With<SplashRoot>>();
    let existing: Vec<Entity> = q.iter(world).collect();

    if want && existing.is_empty() {
        if world.get_resource::<EmberFonts>().is_none() {
            return;
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
    // The root is also the window drag handle — clicking empty background space
    // (the city/shader children are click-through) drags the borderless window.
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
            BackgroundColor(c(5, 4, 10)),
            GlobalZIndex(500),
            FocusPolicy::Block,
            Interaction::default(),
            SplashDragHandle,
            SplashRoot,
            Name::new("splash-root"),
        ))
        .id();

    let backdrop = commands
        .spawn((
            fullscreen_abs(),
            FocusPolicy::Pass,
            crate::native_bg::BgBackground,
            Name::new("splash-bg"),
        ))
        .id();
    let city = commands
        .spawn((fullscreen_abs(), FocusPolicy::Pass, crate::native_city::CityView, Name::new("splash-city")))
        .id();

    let layout = build_layout(commands, fonts);
    let controls = build_window_controls(commands, fonts);

    commands.entity(root).add_children(&[backdrop, city, layout, controls]);
    build_resize_zones(commands, root);
}

fn fullscreen_abs() -> Node {
    Node {
        position_type: PositionType::Absolute,
        left: Val::Px(0.0),
        top: Val::Px(0.0),
        right: Val::Px(0.0),
        bottom: Val::Px(0.0),
        ..default()
    }
}

// ── Main layout (top search · middle title+recents · bottom actions) ─────────

fn build_layout(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let col = commands
        .spawn((
            Node {
                flex_grow: 1.0,
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                padding: UiRect::vertical(Val::Px(20.0)),
                ..default()
            },
            FocusPolicy::Pass,
            Name::new("splash-layout"),
        ))
        .id();

    // ── Top: search, centred ──
    let top = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                justify_content: JustifyContent::Center,
                padding: UiRect::top(Val::Px(8.0)),
                ..default()
            },
            FocusPolicy::Pass,
        ))
        .id();
    let search = build_search(commands, fonts);
    commands.entity(top).add_child(search);

    // ── Middle: actions + recents, vertically centred ──
    let middle = commands
        .spawn((
            Node {
                flex_grow: 1.0,
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                row_gap: Val::Px(30.0),
                ..default()
            },
            FocusPolicy::Pass,
        ))
        .id();

    // Recents block: heading + capped scroll list + empty state.
    let recents = commands
        .spawn((Node { width: Val::Px(CONTENT_W), flex_direction: FlexDirection::Column, row_gap: Val::Px(8.0), ..default() }, FocusPolicy::Pass))
        .id();
    let heading = commands
        .spawn((Text::new("Recent Projects".to_string()), ui_font(&fonts.ui, 13.0), TextColor(text_muted()), FocusPolicy::Pass))
        .id();
    let list = commands
        .spawn((
            Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(8.0), ..default() },
            RecentsContainer,
        ))
        .id();
    keyed_list(commands, list, recents_snapshot);
    let scroll = renzora_ember::widgets::scroll_area(commands, list, 320.0);
    let empty = commands
        .spawn((Text::new("No recent projects yet.".to_string()), ui_font(&fonts.ui, 12.5), TextColor(text_muted()), FocusPolicy::Pass))
        .id();
    commands.entity(empty).insert(Node { margin: UiRect::top(Val::Px(6.0)), align_self: AlignSelf::Center, ..default() });
    bind_display(commands, empty, |w| filtered_rows(w).is_empty());
    commands.entity(recents).add_children(&[heading, scroll, empty]);

    // Actions (New / Open) occupy the spot the large title used to.
    let actions = commands
        .spawn((Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(10.0), ..default() }, FocusPolicy::Pass))
        .id();
    let new = pill_button(commands, fonts, "plus", "New Project", true);
    commands.entity(new).insert(NewProjectBtn);
    let open = pill_button(commands, fonts, "folder-open", "Open Project", false);
    commands.entity(open).insert(OpenProjectBtn);
    commands.entity(actions).add_children(&[new, open]);

    commands.entity(middle).add_children(&[actions, recents]);

    // ── Bottom: social links, centred ──
    let bottom = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: Val::Px(12.0),
                padding: UiRect::bottom(Val::Px(8.0)),
                ..default()
            },
            FocusPolicy::Pass,
        ))
        .id();
    let socials = commands
        .spawn((Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(8.0), ..default() }, FocusPolicy::Pass))
        .id();
    let website = social_button(commands, fonts, "globe", "Website", WEBSITE_URL, false);
    let youtube = social_button(commands, fonts, "youtube-logo", "YouTube", YOUTUBE_URL, false);
    let discord = social_button(commands, fonts, "discord-logo", "Discord", DISCORD_URL, false);
    let star = social_button(commands, fonts, "star", "Star us on GitHub", GITHUB_URL, true);
    commands.entity(socials).add_children(&[website, youtube, discord, star]);

    // Status line (FPS · version) centred under the social buttons.
    let status = commands
        .spawn((
            Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(8.0), ..default() },
            FocusPolicy::Pass,
        ))
        .id();
    let fps = build_fps(commands, fonts);
    let dot = commands
        .spawn((Text::new("·".to_string()), ui_font(&fonts.ui, 11.0), TextColor(text_muted()), FocusPolicy::Pass))
        .id();
    let version = commands
        .spawn((Text::new(format!("Renzora Engine · version {VERSION}")), ui_font(&fonts.ui, 11.0), TextColor(text_muted()), FocusPolicy::Pass))
        .id();
    commands.entity(status).add_children(&[fps, dot, version]);

    commands.entity(bottom).add_children(&[socials, status]);

    commands.entity(col).add_children(&[top, middle, bottom]);
    col
}


/// FPS readout for the centred status line — a quick render-health baseline.
/// Color-coded green/amber/red.
fn build_fps(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let label = commands
        .spawn((
            Text::new(String::new()),
            ui_font(&fonts.mono, 11.0),
            TextColor(text_muted()),
            FocusPolicy::Pass,
            Name::new("splash-fps"),
        ))
        .id();
    bind_text(commands, label, |w| {
        let fps = w.get_resource::<SplashFps>().map(|f| f.0).unwrap_or(0.0);
        format!("{fps:.0} FPS")
    });
    bind_text_color(commands, label, |w| {
        let fps = w.get_resource::<SplashFps>().map(|f| f.0).unwrap_or(0.0);
        if fps >= 58.0 {
            c(100, 200, 100)
        } else if fps >= 30.0 {
            c(200, 200, 100)
        } else {
            c(200, 100, 100)
        }
    });
    label
}

fn build_search(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let row = commands
        .spawn((
            Node {
                width: Val::Px(380.0),
                height: Val::Px(40.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(8.0),
                padding: UiRect::horizontal(Val::Px(12.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(10.0)),
                ..default()
            },
            BackgroundColor(ca(10, 12, 20, 225)),
            BorderColor::all(border_soft()),
        ))
        .id();
    let mag = icon_text(commands, &fonts.phosphor, "magnifying-glass", (150, 158, 178), 14.0);
    commands.entity(mag).insert(FocusPolicy::Pass);
    let search = text_input(commands, &fonts.ui, "Search projects…", "");
    commands.entity(search).insert(Node { flex_grow: 1.0, height: Val::Percent(100.0), align_items: AlignItems::Center, ..default() });
    commands.entity(search).insert((BackgroundColor(Color::NONE), BorderColor::all(Color::NONE)));
    bind_text_input(commands, search, g_filter, s_filter);
    commands.entity(row).add_children(&[mag, search]);
    row
}

// ── Floating window controls (top-right) ─────────────────────────────────────

fn build_window_controls(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let row = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(0.0),
                right: Val::Px(0.0),
                height: Val::Px(36.0),
                flex_direction: FlexDirection::Row,
                ..default()
            },
            GlobalZIndex(600),
            Name::new("splash-window-controls"),
        ))
        .id();
    let min = win_button(commands, fonts, WinBtn::Min, "minus", false);
    let max = win_button(commands, fonts, WinBtn::Max, "square", false);
    let close = win_button(commands, fonts, WinBtn::Close, "x", true);
    commands.entity(row).add_children(&[min, max, close]);
    row
}

fn win_button(commands: &mut Commands, fonts: &EmberFonts, kind: WinBtn, icon: &str, is_close: bool) -> Entity {
    let btn = commands
        .spawn((
            Node {
                width: Val::Px(44.0),
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
        if is_hovered(w, btn) {
            if is_close { c(232, 17, 35) } else { ca(255, 255, 255, 34) }
        } else {
            Color::NONE
        }
    });
    let glyph = icon_text(commands, &fonts.phosphor, icon, (224, 228, 240), 14.0);
    commands.entity(glyph).insert(FocusPolicy::Pass);
    if matches!(kind, WinBtn::Max) {
        let square = renzora_ember::font::icon_glyph("square").unwrap_or('\u{E4C6}');
        let restore = renzora_ember::font::icon_glyph("arrows-in-simple").unwrap_or('\u{E4C6}');
        bind_text(commands, glyph, move |w| {
            let maxed = w.get_resource::<WindowActionQueue>().map(|q| q.maximized).unwrap_or(false);
            (if maxed { restore } else { square }).to_string()
        });
    }
    commands.entity(btn).add_child(glyph);
    btn
}

fn is_hovered(w: &World, e: Entity) -> bool {
    matches!(w.get::<Interaction>(e), Some(Interaction::Hovered) | Some(Interaction::Pressed))
}

// ── Buttons ──────────────────────────────────────────────────────────────────

/// Icon + label action button (New / Open).
fn pill_button(commands: &mut Commands, fonts: &EmberFonts, icon: &str, label_txt: &str, primary: bool) -> Entity {
    let btn = commands
        .spawn((
            Node {
                height: Val::Px(36.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                column_gap: Val::Px(7.0),
                padding: UiRect::horizontal(Val::Px(16.0)),
                border_radius: BorderRadius::all(Val::Px(8.0)),
                ..default()
            },
            BackgroundColor(if primary { accent() } else { btn_dark() }),
            Interaction::default(),
            HoverCursor(SystemCursorIcon::Pointer),
        ))
        .id();
    bind_bg(commands, btn, move |w| {
        let hov = is_hovered(w, btn);
        if primary {
            if hov { accent_hover() } else { accent() }
        } else if hov {
            btn_dark_hover()
        } else {
            btn_dark()
        }
    });
    let ic = icon_text(commands, &fonts.phosphor, icon, if primary { (255, 255, 255) } else { (224, 228, 240) }, 14.0);
    commands.entity(ic).insert(FocusPolicy::Pass);
    let t = commands
        .spawn((Text::new(label_txt.to_string()), ui_font(&fonts.ui, 13.0), TextColor(if primary { white() } else { text() }), FocusPolicy::Pass))
        .id();
    commands.entity(btn).add_children(&[ic, t]);
    btn
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
                border_radius: BorderRadius::all(Val::Px(7.0)),
                ..default()
            },
            BackgroundColor(btn_dark()),
            Interaction::default(),
            SplashUrl(url.to_string()),
            HoverCursor(SystemCursorIcon::Pointer),
        ))
        .id();
    bind_bg(commands, btn, move |w| if is_hovered(w, btn) { btn_dark_hover() } else { btn_dark() });
    let col = if starred { (235, 195, 80) } else { (224, 228, 240) };
    let ic = icon_text(commands, &fonts.phosphor, icon, col, 13.0);
    commands.entity(ic).insert(FocusPolicy::Pass);
    let t = commands
        .spawn((Text::new(txt.to_string()), ui_font(&fonts.ui, 12.5), TextColor(if starred { c(235, 195, 80) } else { text() }), FocusPolicy::Pass))
        .id();
    commands.entity(btn).add_children(&[ic, t]);
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

// ── Recents ──────────────────────────────────────────────────────────────────

#[derive(Clone)]
struct RowData {
    name: String,
    path: std::path::PathBuf,
    path_display: String,
    exists: bool,
}

fn all_rows(world: &World) -> Vec<RowData> {
    let Some(cfg) = world.get_resource::<AppConfig>() else {
        return Vec::new();
    };
    cfg.recent_projects
        .iter()
        .map(|p| {
            let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("Unknown Project").to_string();
            let path_display = p.to_string_lossy().to_string();
            #[cfg(not(target_arch = "wasm32"))]
            let exists = p.join("project.toml").exists();
            #[cfg(target_arch = "wasm32")]
            let exists = true;
            RowData { name, path: p.clone(), path_display, exists }
        })
        .collect()
}

fn filtered_rows(world: &World) -> Vec<RowData> {
    let filter = world.get_resource::<SplashFilter>().map(|f| f.0.to_lowercase()).unwrap_or_default();
    let filter = filter.trim();
    let rows = all_rows(world);
    if filter.is_empty() {
        return rows;
    }
    rows.into_iter()
        .filter(|r| r.name.to_lowercase().contains(filter) || r.path_display.to_lowercase().contains(filter))
        .collect()
}

fn recents_snapshot(world: &World) -> KeyedSnapshot {
    use std::hash::{Hash, Hasher};
    let rows = filtered_rows(world);
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
                height: Val::Px(58.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(13.0),
                padding: UiRect::horizontal(Val::Px(14.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(10.0)),
                ..default()
            },
            BackgroundColor(ca(16, 18, 28, 220)),
            BorderColor::all(border_soft()),
            Interaction::default(),
        ))
        .id();
    if row.exists {
        commands.entity(container).insert((RecentOpen(row.path.clone()), HoverCursor(SystemCursorIcon::Pointer)));
        let cc = container;
        bind_bg(commands, container, move |w| if is_hovered(w, cc) { panel_hover() } else { ca(16, 18, 28, 220) });
    }

    let icon = icon_text(commands, &fonts.phosphor, "folder", if row.exists { (110, 150, 255) } else { (150, 158, 178) }, 21.0);
    commands.entity(icon).insert(FocusPolicy::Pass);

    let info = commands
        .spawn((Node { flex_grow: 1.0, flex_direction: FlexDirection::Column, row_gap: Val::Px(3.0), ..default() }, FocusPolicy::Pass))
        .id();
    let name_txt = if row.exists { row.name.clone() } else { format!("{}  (missing)", row.name) };
    let name = commands
        .spawn((Text::new(name_txt), ui_font(&fonts.ui, 14.0), TextColor(if row.exists { text() } else { text_muted() }), FocusPolicy::Pass))
        .id();
    let path = commands
        .spawn((Text::new(elide_path(&row.path_display, 56)), ui_font(&fonts.mono, 10.0), TextColor(text_muted()), FocusPolicy::Pass))
        .id();
    commands.entity(info).add_children(&[name, path]);

    let remove = commands
        .spawn((
            Node { width: Val::Px(26.0), height: Val::Px(26.0), align_items: AlignItems::Center, justify_content: JustifyContent::Center, border_radius: BorderRadius::all(Val::Px(5.0)), ..default() },
            BackgroundColor(Color::NONE),
            Interaction::default(),
            RecentRemove(row.path.clone()),
            HoverCursor(SystemCursorIcon::Pointer),
        ))
        .id();
    let rc = remove;
    bind_bg(commands, remove, move |w| if is_hovered(w, rc) { ca(239, 68, 68, 40) } else { Color::NONE });
    let rx = icon_text(commands, &fonts.phosphor, "x", (150, 158, 178), 13.0);
    commands.entity(rx).insert(FocusPolicy::Pass);
    bind_text_color_on_hover(commands, rx, remove);
    commands.entity(remove).add_child(rx);

    commands.entity(container).add_children(&[icon, info, remove]);
    container
}

fn bind_text_color_on_hover(commands: &mut Commands, text_e: Entity, btn: Entity) {
    react(commands, move |world: &mut World| {
        if world.get_entity(text_e).is_err() || world.get_entity(btn).is_err() {
            return false;
        }
        let col = if is_hovered(world, btn) { error_color() } else { text_muted() };
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

// ── Resize zones ─────────────────────────────────────────────────────────────

fn build_resize_zones(commands: &mut Commands, root: Entity) {
    let t = Val::Px(8.0);
    let cz = Val::Px(16.0);
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

// ── Field accessors ──────────────────────────────────────────────────────────

fn g_filter(w: &World) -> String {
    w.get_resource::<SplashFilter>().map(|f| f.0.clone()).unwrap_or_default()
}
fn s_filter(w: &mut World, v: String) {
    if let Some(mut f) = w.get_resource_mut::<SplashFilter>() {
        f.0 = v;
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

// ── Project actions ──────────────────────────────────────────────────────────

fn enter_project(world: &mut World, project: crate::project::CurrentProject) {
    if let Some(mut cfg) = world.get_resource_mut::<AppConfig>() {
        cfg.add_recent_project(project.path.clone());
        let _ = cfg.save();
    }
    world.insert_resource(project);
    world.resource_mut::<NextState<SplashState>>().set(SplashState::Loading);
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

/// New Project = pick (or create) a folder in the OS dialog; that folder becomes
/// the project root, named after the folder.
fn do_new_project(world: &mut World) {
    #[cfg(not(target_arch = "wasm32"))]
    {
        if let Some(folder) = rfd::FileDialog::new().set_title("New Project — choose a folder").pick_folder() {
            let name = folder
                .file_name()
                .and_then(|n| n.to_str())
                .map(|s| s.to_string())
                .unwrap_or_else(|| "New Project".to_string());
            match create_project(&folder, &name) {
                Ok(p) => enter_project(world, p),
                Err(e) => error!("Failed to create project: {e}"),
            }
        }
    }
    #[cfg(target_arch = "wasm32")]
    {
        let _ = world;
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
