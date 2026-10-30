//! The companion's window: a header, a rail of six screens, a footer.
//!
//! Built once ember's fonts exist. Text that changes is found by marker and
//! rewritten only when what it shows changed; buttons carry the [`UiAction`]
//! they perform, and one system handles every press.

use std::path::PathBuf;

use bevy::prelude::*;
use renzora_ember::cursor_icon::HoverCursor;
use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::theme::*;
use renzora_ember::UiInteraction;

use super::capture::Capture;
use super::games::{ConnRequest, ConnState, Connection, GameInfo};
use super::jobs::JobQueue;

mod cache_page;
pub(crate) mod canvas;
pub(crate) mod debug_page;
mod games_page;
mod jobs_page;
mod profiler;
mod render_graph;

/// The screens on the rail.
#[derive(Resource, Clone, Copy, PartialEq, Eq, Default, Debug)]
pub(crate) enum Page {
    #[default]
    Profiler,
    Render,
    Debug,
    Jobs,
    Cache,
    Games,
}

/// Whether views follow the newest frame. Off is "record only": data keeps
/// arriving and is kept, but nothing redraws, so the companion costs the
/// machine almost nothing while you measure.
#[derive(Resource)]
pub(crate) struct LiveView(pub bool);

impl Default for LiveView {
    fn default() -> Self {
        Self(true)
    }
}

/// A short message in the footer, e.g. where a capture was saved.
#[derive(Resource, Default)]
pub(crate) struct Notice(pub Option<String>);

/// What a button does.
#[derive(Component, Clone, Debug, PartialEq)]
pub(crate) enum UiAction {
    Page(Page),
    ToggleRecord,
    ToggleLive,
    SaveCapture,
    OpenCapture(PathBuf),
    Connect(GameInfo),
    Disconnect,
    CancelJob(u64),
    /// Start a job a recording is holding.
    RunJobAnyway(u64),
    Timeline(profiler::TimelineOp),
    /// Show or hide a probe on the Debug screen.
    ToggleProbe(String),
    /// Use a probe's toggle or button.
    ProbeAct { probe: String, item: String, value: Option<bool> },
    /// One of the Cache screen's buttons.
    Cache(crate::app::cache::CacheRequest),
}

/// The root of a page; shown when its page is active.
#[derive(Component)]
struct PageRoot(Page);

/// A rail button, highlighted while its page is active.
#[derive(Component)]
struct RailButton(Page);

#[derive(Component)]
enum HeaderText {
    Game,
    Record,
    Live,
}

#[derive(Component)]
struct FooterText;

pub(crate) fn build(app: &mut App) {
    app.init_resource::<Page>()
        .init_resource::<LiveView>()
        .init_resource::<Notice>()
        .add_systems(
            Update,
            (
                spawn_shell.run_if(resource_exists::<EmberFonts>),
                handle_actions,
                tint_buttons,
                show_page,
                refresh_header,
                refresh_footer,
            ),
        );
    canvas::build(app);
    profiler::build(app);
    render_graph::build(app);
    debug_page::build(app);
    jobs_page::build(app);
    cache_page::build(app);
    games_page::build(app);
}

// ── Building blocks ─────────────────────────────────────────────────────────

/// A span or system name cut to its last `::` segment and without generics,
/// so `bevy_ui::layout::ui_layout_system` reads as `ui_layout_system`.
pub(crate) fn short(name: &str) -> &str {
    let trimmed = name.split('<').next().unwrap_or(name);
    trimmed.rsplit("::").next().unwrap_or(trimmed)
}

/// How much larger the companion's text is than the sizes written at each call
/// site. The companion is a window you glance at from across the desk, beside
/// the editor and a running game, so it reads a size up from the editor's
/// dense panels.
pub(crate) const TEXT_UP: f32 = 2.0;

pub(crate) fn text(commands: &mut Commands, fonts: &EmberFonts, s: impl Into<String>, size: f32, color: (u8, u8, u8)) -> Entity {
    commands
        .spawn((Text::new(s), ui_font(&fonts.ui, size + TEXT_UP), TextColor(rgb(color))))
        .id()
}

pub(crate) fn mono(commands: &mut Commands, fonts: &EmberFonts, s: impl Into<String>, size: f32, color: (u8, u8, u8)) -> Entity {
    commands
        .spawn((Text::new(s), ui_font(&fonts.mono, size + TEXT_UP), TextColor(rgb(color))))
        .id()
}

/// A rounded panel on the window background.
pub(crate) fn card(commands: &mut Commands) -> Entity {
    commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(12.0)),
                row_gap: Val::Px(8.0),
                border_radius: BorderRadius::all(Val::Px(8.0)),
                ..default()
            },
            BackgroundColor(rgb(panel_bg())),
        ))
        .id()
}

/// A button: a label (and optional icon) that performs `action` when pressed.
pub(crate) fn button(
    commands: &mut Commands,
    fonts: &EmberFonts,
    icon: Option<&str>,
    label: &str,
    action: UiAction,
) -> Entity {
    let b = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(7.0),
                height: Val::Px(32.0),
                padding: UiRect::axes(Val::Px(14.0), Val::Px(0.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                flex_shrink: 0.0,
                ..default()
            },
            BackgroundColor(rgb(panel_bg())),
            BorderColor::all(rgb(border())),
            UiInteraction::default(),
            HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            action,
        ))
        .id();
    let mut kids = Vec::new();
    if let Some(icon) = icon {
        kids.push(icon_text(commands, &fonts.phosphor, icon, text_muted(), 16.0));
    }
    if !label.is_empty() {
        kids.push(text(commands, fonts, label, 12.0, text_primary()));
    }
    commands.entity(b).add_children(&kids);
    b
}

// ── The shell ───────────────────────────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
fn spawn_shell(
    mut commands: Commands,
    fonts: Res<EmberFonts>,
    mut built: Local<bool>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut layers: ResMut<canvas::NextLayer>,
) {
    if *built {
        return;
    }
    *built = true;
    commands.spawn((Camera2d, bevy::ui::IsDefaultUiCamera, Name::new("companion-ui-camera")));

    // Header.
    let logo = icon_text(&mut commands, &fonts.phosphor, "pulse", accent(), 18.0);
    let title = text(&mut commands, &fonts, "Renzora Companion", 14.0, text_primary());
    let game = button(&mut commands, &fonts, Some("game-controller"), "No game", UiAction::Page(Page::Games));
    commands.entity(game).insert(HeaderText::Game);
    let spacer = commands.spawn(Node { flex_grow: 1.0, ..default() }).id();
    let record = button(&mut commands, &fonts, Some("record"), "Record", UiAction::ToggleRecord);
    commands.entity(record).insert(HeaderText::Record);
    let live = button(&mut commands, &fonts, Some("eye"), "Live view", UiAction::ToggleLive);
    commands.entity(live).insert(HeaderText::Live);
    let save = button(&mut commands, &fonts, Some("floppy-disk"), "Save capture", UiAction::SaveCapture);
    let header = commands
        .spawn((
            Node {
                height: Val::Px(46.0),
                flex_shrink: 0.0,
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(10.0),
                padding: UiRect::axes(Val::Px(14.0), Val::Px(0.0)),
                border: UiRect::bottom(Val::Px(1.0)),
                ..default()
            },
            BorderColor::all(rgb(divider())),
        ))
        .id();
    commands
        .entity(header)
        .add_children(&[logo, title, game, spacer, record, live, save]);

    // Rail.
    let rail = commands
        .spawn((
            Node {
                width: Val::Px(76.0),
                flex_shrink: 0.0,
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: Val::Px(4.0),
                padding: UiRect::top(Val::Px(10.0)),
                border: UiRect::right(Val::Px(1.0)),
                ..default()
            },
            BorderColor::all(rgb(divider())),
        ))
        .id();
    for (page, icon, label) in [
        (Page::Profiler, "pulse", "Profiler"),
        (Page::Render, "tree-structure", "Render"),
        (Page::Debug, "bug", "Debug"),
        (Page::Jobs, "list-checks", "Jobs"),
        (Page::Cache, "database", "Cache"),
        (Page::Games, "game-controller", "Games"),
    ] {
        let item = commands
            .spawn((
                Node {
                    width: Val::Px(64.0),
                    padding: UiRect::axes(Val::Px(0.0), Val::Px(8.0)),
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    row_gap: Val::Px(4.0),
                    border_radius: BorderRadius::all(Val::Px(8.0)),
                    ..default()
                },
                BackgroundColor(Color::NONE),
                UiInteraction::default(),
                HoverCursor(bevy::window::SystemCursorIcon::Pointer),
                UiAction::Page(page),
                RailButton(page),
            ))
            .id();
        let glyph = icon_text(&mut commands, &fonts.phosphor, icon, text_muted(), 20.0);
        let name = text(&mut commands, &fonts, label, 11.0, text_muted());
        commands.entity(item).add_children(&[glyph, name]);
        commands.entity(rail).add_child(item);
    }

    // Pages.
    let pages = commands
        .spawn(Node { flex_grow: 1.0, min_width: Val::Px(0.0), min_height: Val::Px(0.0), ..default() })
        .id();
    let profiler = profiler::spawn(&mut commands, &fonts, &mut meshes, &mut materials, &mut layers.0);
    let render = render_graph::spawn(&mut commands, &fonts);
    let debug = debug_page::spawn(&mut commands, &fonts);
    let jobs = jobs_page::spawn(&mut commands, &fonts);
    let cache = cache_page::spawn(&mut commands, &fonts);
    let games = games_page::spawn(&mut commands, &fonts);
    for (root, page) in [
        (profiler, Page::Profiler),
        (render, Page::Render),
        (debug, Page::Debug),
        (jobs, Page::Jobs),
        (cache, Page::Cache),
        (games, Page::Games),
    ] {
        commands.entity(root).insert(PageRoot(page));
        commands.entity(pages).add_child(root);
    }

    let body = commands
        .spawn(Node {
            flex_grow: 1.0,
            min_height: Val::Px(0.0),
            flex_direction: FlexDirection::Row,
            ..default()
        })
        .id();
    commands.entity(body).add_children(&[rail, pages]);

    // Footer.
    let footer_text = text(&mut commands, &fonts, "", 11.0, text_muted());
    commands.entity(footer_text).insert(FooterText);
    let footer = commands
        .spawn((
            Node {
                height: Val::Px(26.0),
                flex_shrink: 0.0,
                align_items: AlignItems::Center,
                padding: UiRect::axes(Val::Px(14.0), Val::Px(0.0)),
                border: UiRect::top(Val::Px(1.0)),
                ..default()
            },
            BorderColor::all(rgb(divider())),
        ))
        .id();
    commands.entity(footer).add_child(footer_text);

    let root = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                ..default()
            },
            BackgroundColor(rgb(window_bg())),
            Name::new("companion-root"),
        ))
        .id();
    commands.entity(root).add_children(&[header, body, footer]);
}

/// Every button press in the window.
#[allow(clippy::too_many_arguments)]
fn handle_actions(
    pressed: Query<(&UiInteraction, &UiAction), Changed<UiInteraction>>,
    mut page: ResMut<Page>,
    mut live: ResMut<LiveView>,
    mut notice: ResMut<Notice>,
    mut requests: MessageWriter<ConnRequest>,
    conn: Res<Connection>,
    mut capture: ResMut<Capture>,
    mut jobs: ResMut<JobQueue>,
    mut timeline: MessageWriter<profiler::TimelineOp>,
    mut selected: ResMut<profiler::SelectedFrame>,
    mut debug: ResMut<debug_page::DebugView>,
    mut cache: MessageWriter<crate::app::cache::CacheRequest>,
) {
    for (interaction, action) in &pressed {
        if *interaction != UiInteraction::Pressed {
            continue;
        }
        match action.clone() {
            UiAction::Page(p) => *page = p,
            UiAction::ToggleRecord => {
                if conn.state == ConnState::Connected {
                    requests.write(ConnRequest::Record(!conn.recording));
                } else {
                    *page = Page::Games;
                }
            }
            UiAction::ToggleLive => live.0 = !live.0,
            UiAction::SaveCapture => {
                notice.0 = Some(save_capture(&capture));
            }
            UiAction::OpenCapture(path) => match Capture::open(&path) {
                Ok(opened) => {
                    requests.write(ConnRequest::Disconnect);
                    let revision = capture.revision + 1;
                    *capture = opened;
                    capture.revision = revision;
                    selected.0 = capture.frames.back().map(|f| f.index);
                    *page = Page::Profiler;
                    notice.0 = Some(format!("Opened {}", path.display()));
                }
                Err(e) => notice.0 = Some(format!("Could not open {}: {e}", path.display())),
            },
            UiAction::Connect(game) => {
                requests.write(ConnRequest::Connect(game));
                *page = Page::Profiler;
            }
            UiAction::Disconnect => {
                requests.write(ConnRequest::Disconnect);
            }
            UiAction::CancelJob(id) => jobs.cancel(id),
            UiAction::RunJobAnyway(id) => jobs.run_anyway(id),
            UiAction::Timeline(op) => {
                timeline.write(op);
            }
            UiAction::ToggleProbe(id) => {
                if !debug.watching.remove(&id) {
                    debug.watching.insert(id);
                }
            }
            UiAction::ProbeAct { probe, item, value } => {
                requests.write(ConnRequest::ProbeAct { probe, item, value });
            }
            UiAction::Cache(request) => {
                cache.write(request);
            }
        }
    }
}

fn save_capture(capture: &Capture) -> String {
    if capture.frames.is_empty() {
        return "Nothing recorded yet to save".to_string();
    }
    let Some(folder) = Capture::folder() else {
        return "No home folder to save captures in".to_string();
    };
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let safe: String = capture
        .source
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
        .collect();
    let path = folder.join(format!("{}-{stamp}.rzprof", if safe.is_empty() { "capture" } else { &safe }));
    match capture.save(&path) {
        Ok(()) => format!("Saved {} frames to {}", capture.frames.len(), path.display()),
        Err(e) => format!("Could not save the capture: {e}"),
    }
}

/// Hover and active tints for every button.
fn tint_buttons(
    mut buttons: Query<(&UiInteraction, &mut BackgroundColor, Option<&RailButton>), (Changed<UiInteraction>, With<UiAction>)>,
    page: Res<Page>,
) {
    for (interaction, mut bg, rail) in &mut buttons {
        let active = rail.is_some_and(|r| r.0 == *page);
        bg.0 = match (interaction, active) {
            (_, true) => rgb(tab_active()),
            (UiInteraction::Hovered | UiInteraction::Pressed, false) => rgb(hover_bg()),
            (_, false) if rail.is_some() => Color::NONE,
            _ => rgb(panel_bg()),
        };
    }
}

fn show_page(
    page: Res<Page>,
    mut roots: Query<(&PageRoot, &mut Node)>,
    added: Query<(), Added<PageRoot>>,
    mut rail: Query<(&RailButton, &mut BackgroundColor, &Children)>,
    mut texts: Query<&mut TextColor>,
) {
    // Also when the pages have just been built: they are built after `Page`
    // first changed, and every page starts visible, so without this all four
    // sat side by side until the first click on the rail.
    if !page.is_changed() && added.is_empty() {
        return;
    }
    for (root, mut node) in &mut roots {
        let display = if root.0 == *page { Display::Flex } else { Display::None };
        if node.display != display {
            node.display = display;
        }
    }
    for (button, mut bg, children) in &mut rail {
        let active = button.0 == *page;
        bg.0 = if active { rgb(tab_active()) } else { Color::NONE };
        for child in children.iter() {
            if let Ok(mut color) = texts.get_mut(child) {
                color.0 = rgb(if active { text_primary() } else { text_muted() });
            }
        }
    }
}

fn refresh_header(
    conn: Res<Connection>,
    live: Res<LiveView>,
    capture: Res<Capture>,
    buttons: Query<(&HeaderText, &Children)>,
    added: Query<(), Added<HeaderText>>,
    mut texts: Query<&mut Text>,
    mut colors: Query<&mut TextColor>,
) {
    // Also when the header has just been built: it is built after these
    // resources first changed, so waiting for a change would leave it blank.
    if !conn.is_changed() && !live.is_changed() && !capture.is_changed() && added.is_empty() {
        return;
    }
    for (which, children) in &buttons {
        let (label, color) = match which {
            HeaderText::Game => match (&conn.game, &capture.from_file) {
                (Some(game), _) => (
                    match &conn.state {
                        ConnState::Connected => format!("{} · {}", game.name, game.place()),
                        ConnState::Connecting => format!("Connecting to {}…", game.name),
                        ConnState::Failed(why) => format!("{}: {why}", game.name),
                        ConnState::Idle => game.name.clone(),
                    },
                    text_primary(),
                ),
                (None, Some(path)) => (
                    format!("Capture: {}", path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()),
                    text_primary(),
                ),
                (None, None) => ("Pick a game to profile".to_string(), text_muted()),
            },
            HeaderText::Record => {
                if conn.recording {
                    ("Recording".to_string(), close_red())
                } else {
                    ("Record".to_string(), text_primary())
                }
            }
            HeaderText::Live => {
                if live.0 {
                    ("Live view".to_string(), text_primary())
                } else {
                    ("Record only (view paused)".to_string(), warn_amber())
                }
            }
        };
        // The label is the button's last child; its icon, if any, comes first.
        if let Some(last) = children.iter().last() {
            if let Ok(mut t) = texts.get_mut(last) {
                if t.0 != label {
                    t.0 = label;
                }
            }
            if let Ok(mut c) = colors.get_mut(last) {
                c.0 = rgb(color);
            }
        }
    }
}

fn refresh_footer(
    conn: Res<Connection>,
    capture: Res<Capture>,
    jobs: Res<JobQueue>,
    notice: Res<Notice>,
    mut footer: Query<&mut Text, With<FooterText>>,
) {
    if !conn.is_changed() && !capture.is_changed() && !jobs.is_changed() && !notice.is_changed() {
        return;
    }
    let mut parts = Vec::new();
    if conn.recording {
        parts.push(format!("Recording · {} frames kept", capture.frames.len()));
    } else if !capture.frames.is_empty() {
        parts.push(format!("{} frames", capture.frames.len()));
    }
    let gpu = capture.frames.back().is_some_and(|f| !f.gpu.is_empty());
    if !capture.frames.is_empty() {
        parts.push(if gpu { "GPU timings on".into() } else { "No GPU timings from this game".into() });
    }
    let running = jobs.running.len();
    let waiting = jobs.waiting().count();
    if running + waiting > 0 {
        parts.push(format!("Jobs: {running} running, {waiting} waiting"));
    }
    if let Some(n) = &notice.0 {
        parts.push(n.clone());
    }
    let line = parts.join("   ·   ");
    for mut t in &mut footer {
        if t.0 != line {
            t.0.clone_from(&line);
        }
    }
}
