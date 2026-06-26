//! Bevy-native (ember) loading UI — the bevy_ui counterpart to the egui
//! `paint_loading_overlay`. Two consumers, same letterbox + animated bar:
//!
//! * **Loading state** — a fullscreen dark screen between project pick and the
//!   editor. Always native (egui kept as a pre-fonts fallback).
//! * **Editor overlay** — a dimmed, pointer-blocking modal painted over the
//!   editor while a tab's GLBs decode. Native only under the `BevyUi` backend
//!   (under `Egui` the editor itself is egui, which paints over bevy_ui, so the
//!   egui overlay is used instead).
//!
//! The egui neon rosette is painter line-art with no plain-bevy_ui equivalent;
//! the native letterbox is a flat panel.

use bevy::ecs::world::CommandQueue;
use bevy::prelude::*;
use bevy::ui::FocusPolicy;

use renzora_ember::font::{ui_font, EmberFonts};
use renzora_ember::reactive::{bind_text, bind_with};
use renzora_ember::widgets::OverlaySurface;

use crate::loading::{EditorLoadingOverlayActive, LoadingBytes, LoadingTasks, TextureLoadProgress};
use crate::SplashState;

#[derive(Component)]
pub(crate) struct LoadingScreenRoot;
#[derive(Component)]
pub(crate) struct EditorOverlayRoot;
#[derive(Component)]
struct LoadingFill;
#[derive(Component)]
struct LoadingPercent;
#[derive(Component)]
struct LoadingDetail;
#[derive(Component)]
struct LoadingTerminal;

/// Smoothed display progress. Real byte progress for a single large GLB is a
/// 0→100% step (Bevy loads it as one asset with no sub-file reporting), so we
/// ease a displayed fraction up toward a cap while it's in flight and snap to
/// 1.0 the moment the real load actually completes. The *total* bytes and the
/// completion are real; only the in-between motion is interpolated.
#[derive(Resource, Default)]
struct LoadingAnim {
    displayed: f32,
    elapsed: f32,
}

pub(crate) fn register(app: &mut App) {
    app.init_resource::<LoadingAnim>();
    app.add_systems(
        Update,
        (manage_loading_screen, manage_editor_overlay, tick_loading_anim),
    );
}

/// Compute the *real* load fraction: the GLB/scene phase is the first half, the
/// texture-decode phase the second half. Both halves are backed by real signals
/// (bytes read, then textures decoded).
fn real_load_fraction(bytes: &LoadingBytes, tex: &TextureLoadProgress) -> f32 {
    if tex.total > 0 {
        // GLB done (textures only resolve after) → second half tracks decode.
        let tf = (tex.loaded as f32 / tex.total as f32).clamp(0.0, 1.0);
        0.5 + 0.5 * tf
    } else if bytes.total > 0 {
        // GLB read phase (first half). For a single big GLB this is a 0→1 step.
        let bf = (bytes.loaded as f32 / bytes.total as f32).clamp(0.0, 1.0);
        0.5 * bf
    } else {
        0.0
    }
}

/// Ease the displayed progress toward the real fraction. During an opaque phase
/// (e.g. a single big GLB parse, where the real value is stuck at 0) a slow time
/// creep keeps the bar moving; the real signal takes over whenever it's higher.
fn tick_loading_anim(
    time: Res<Time>,
    state: Res<State<SplashState>>,
    bytes: Res<LoadingBytes>,
    tex: Res<TextureLoadProgress>,
    mut anim: ResMut<LoadingAnim>,
) {
    if !matches!(state.get(), SplashState::Loading) {
        anim.displayed = 0.0; // reset for the next load
        anim.elapsed = 0.0;
        return;
    }
    let dt = time.delta_secs();
    anim.elapsed += dt;
    let real = real_load_fraction(&bytes, &tex);
    // Slow creep toward 0.9 so opaque phases still show motion; real wins when higher.
    let creep = 0.9 * (1.0 - (-anim.elapsed * 0.3).exp());
    let target = real.max(creep).min(1.0);
    let k = 1.0 - (-dt * 1.3).exp();
    anim.displayed += (target - anim.displayed) * k;
    anim.displayed = anim.displayed.clamp(0.0, 1.0);
}

// ── Colours (mirror loading.rs) ──────────────────────────────────────────────

fn c(r: u8, g: u8, b: u8) -> Color {
    Color::srgb_u8(r, g, b)
}
fn ca(r: u8, g: u8, b: u8, a: u8) -> Color {
    Color::srgba_u8(r, g, b, a)
}

// ── Lifecycle ────────────────────────────────────────────────────────────────

fn manage_loading_screen(world: &mut World) {
    let want = matches!(world.resource::<State<SplashState>>().get(), SplashState::Loading);
    let mut q = world.query_filtered::<Entity, With<LoadingScreenRoot>>();
    let existing: Vec<Entity> = q.iter(world).collect();

    if want && existing.is_empty() {
        if world.get_resource::<EmberFonts>().is_none() {
            return;
        }
        let fonts = world.resource::<EmberFonts>().clone();
        let (pname, ppath) = project_info(world);
        let mut queue = CommandQueue::default();
        {
            let mut commands = Commands::new(&mut queue, world);
            spawn_loading(&mut commands, &fonts, false, &pname, &ppath);
        }
        queue.apply(world);
    } else if !want && !existing.is_empty() {
        for e in existing {
            world.entity_mut(e).despawn();
        }
    }
}

/// The active project's display name and folder path (empty if none open yet).
fn project_info(world: &World) -> (String, String) {
    world
        .get_resource::<renzora::CurrentProject>()
        .map(|p| (p.config.name.clone(), p.path.display().to_string()))
        .unwrap_or_default()
}

fn manage_editor_overlay(world: &mut World) {
    let in_editor = matches!(world.resource::<State<SplashState>>().get(), SplashState::Editor);
    let active = world.get_resource::<EditorLoadingOverlayActive>().is_some_and(|a| a.0);
    let want = in_editor && active;

    let mut q = world.query_filtered::<Entity, With<EditorOverlayRoot>>();
    let existing: Vec<Entity> = q.iter(world).collect();

    if want && existing.is_empty() {
        if world.get_resource::<EmberFonts>().is_none() {
            return;
        }
        let fonts = world.resource::<EmberFonts>().clone();
        let mut queue = CommandQueue::default();
        {
            let mut commands = Commands::new(&mut queue, world);
            spawn_loading(&mut commands, &fonts, true, "", "");
        }
        queue.apply(world);
    } else if !want && !existing.is_empty() {
        for e in existing {
            world.entity_mut(e).despawn();
        }
    }
}

fn spawn_loading(
    commands: &mut Commands,
    fonts: &EmberFonts,
    modal: bool,
    project_name: &str,
    project_path: &str,
) {
    let mut backdrop = commands.spawn((
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
        BackgroundColor(if modal { ca(8, 10, 16, 180) } else { c(3, 5, 4) }),
        GlobalZIndex(if modal { 9600 } else { 480 }),
        Name::new(if modal { "editor-loading-overlay" } else { "loading-screen" }),
    ));
    if modal {
        backdrop.insert((
            FocusPolicy::Block,
            Interaction::default(),
            bevy::ui::RelativeCursorPosition::default(),
            OverlaySurface,
            EditorOverlayRoot,
        ));
    } else {
        backdrop.insert(LoadingScreenRoot);
    }
    let backdrop = backdrop.id();

    // Modal editor overlay stays the plain letterbox; the standalone loading screen
    // gets a drifting particle network behind a hacky terminal.
    if modal {
        let panel = build_letterbox(commands, fonts);
        commands.entity(backdrop).add_child(panel);
    } else {
        let network = commands
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
                crate::native_post::NetworkView,
                Name::new("loading-network"),
            ))
            .id();
        let panel = build_terminal(commands, fonts, project_name, project_path);
        commands.entity(backdrop).add_children(&[network, panel]);
    }
}

/// Track + animated fill + percent label (the smooth bar used by the modal).
fn build_progress_row(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let row = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(10.0),
            ..default()
        })
        .id();
    let track = commands
        .spawn((
            Node {
                flex_grow: 1.0,
                height: Val::Px(12.0),
                overflow: Overflow::clip(),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(ca(6, 6, 12, 220)),
            BorderColor::all(c(48, 50, 72)),
        ))
        .id();
    let fill = commands
        .spawn((
            Node { width: Val::Percent(0.0), height: Val::Percent(100.0), ..default() },
            BackgroundColor(c(90, 180, 255)),
            LoadingFill,
        ))
        .id();
    bind_with(commands, fill, loading_fraction, |world, target, v: &OrderedF32| {
        if let Some(mut n) = world.get_mut::<Node>(target) {
            n.width = Val::Percent((v.0 * 100.0).clamp(0.0, 100.0));
        }
    });
    bind_with(commands, fill, loading_fill_color, |world, target, v: &[u8; 3]| {
        if let Some(mut bg) = world.get_mut::<BackgroundColor>(target) {
            bg.0 = c(v[0], v[1], v[2]);
        }
    });
    commands.entity(track).add_child(fill);

    let percent = commands
        .spawn((
            Node { min_width: Val::Px(44.0), ..default() },
            Text::new("0%".to_string()),
            ui_font(&fonts.mono, 13.0),
            TextColor(c(220, 225, 240)),
            LoadingPercent,
        ))
        .id();
    bind_text(commands, percent, loading_percent_text);
    commands.entity(row).add_children(&[track, percent]);
    row
}

/// The plain centered letterbox (used for the modal editor overlay).
fn build_letterbox(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let letterbox = commands
        .spawn((
            Node {
                width: Val::Px(600.0),
                max_width: Val::Percent(94.0),
                height: Val::Px(180.0),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                padding: UiRect::all(Val::Px(18.0)),
                row_gap: Val::Px(14.0),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                ..default()
            },
            BackgroundColor(ca(20, 22, 32, 215)),
            BorderColor::all(c(48, 50, 72)),
        ))
        .id();
    let title = commands
        .spawn((Text::new("Loading".to_string()), ui_font(&fonts.ui, 18.0), TextColor(c(240, 240, 248))))
        .id();
    let detail = commands
        .spawn((Text::new(String::new()), ui_font(&fonts.ui, 12.0), TextColor(c(170, 175, 190)), LoadingDetail))
        .id();
    bind_text(commands, detail, loading_detail_text);
    let row = build_progress_row(commands, fonts);
    commands.entity(letterbox).add_children(&[title, detail, row]);
    letterbox
}

/// The hacky terminal window: a header bar, a streaming boot log that fills in
/// with loading progress (blinking cursor), and a green progress bar footer.
fn build_terminal(
    commands: &mut Commands,
    fonts: &EmberFonts,
    project_name: &str,
    project_path: &str,
) -> Entity {
    let win = commands
        .spawn((
            Node {
                width: Val::Px(720.0),
                max_width: Val::Percent(94.0),
                height: Val::Px(420.0),
                max_height: Val::Percent(90.0),
                flex_direction: FlexDirection::Column,
                overflow: Overflow::clip(),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                ..default()
            },
            BackgroundColor(ca(4, 9, 6, 248)),
            BorderColor::all(c(40, 130, 80)),
        ))
        .id();

    // Title bar.
    let header = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(26.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(8.0),
                padding: UiRect::horizontal(Val::Px(12.0)),
                ..default()
            },
            BackgroundColor(ca(10, 26, 16, 255)),
        ))
        .id();
    let htitle = commands
        .spawn((
            Text::new("renzora://boot  —  secure shell".to_string()),
            ui_font(&fonts.mono, 11.5),
            TextColor(c(90, 200, 130)),
        ))
        .id();
    commands.entity(header).add_child(htitle);

    // Streaming log body.
    let body = commands
        .spawn(Node {
            flex_grow: 1.0,
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            justify_content: JustifyContent::FlexStart,
            row_gap: Val::Px(4.0),
            padding: UiRect::all(Val::Px(18.0)),
            overflow: Overflow::clip(),
            ..default()
        })
        .id();
    let log = commands
        .spawn((Text::new(String::new()), ui_font(&fonts.mono, 15.0), TextColor(c(160, 255, 180)), LoadingTerminal))
        .id();
    bind_text(commands, log, terminal_text);
    commands.entity(body).add_child(log);

    // Footer: the active project (name + path), then a full-width loading bar
    // spanning the whole console, with a percent readout.
    let footer = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(7.0),
            padding: UiRect::all(Val::Px(16.0)),
            ..default()
        })
        .id();

    let proj = commands
        .spawn((
            Text::new(format!("project: {project_name}")),
            ui_font(&fonts.mono, 12.5),
            TextColor(c(120, 235, 160)),
        ))
        .id();
    let loc = commands
        .spawn((
            Text::new(format!("path:    {project_path}")),
            ui_font(&fonts.mono, 11.0),
            TextColor(c(80, 150, 110)),
        ))
        .id();

    // Full-width track + animated fill (eased progress).
    let track = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(14.0),
                margin: UiRect::top(Val::Px(2.0)),
                overflow: Overflow::clip(),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(3.0)),
                ..default()
            },
            BackgroundColor(ca(4, 14, 8, 255)),
            BorderColor::all(c(40, 130, 80)),
        ))
        .id();
    let fill = commands
        .spawn((
            Node { width: Val::Percent(0.0), height: Val::Percent(100.0), ..default() },
            BackgroundColor(c(90, 235, 140)),
            LoadingFill,
        ))
        .id();
    bind_with(commands, fill, loading_fraction_eased, |world, target, v: &OrderedF32| {
        if let Some(mut n) = world.get_mut::<Node>(target) {
            n.width = Val::Percent((v.0 * 100.0).clamp(0.0, 100.0));
        }
    });
    commands.entity(track).add_child(fill);

    let pct = commands
        .spawn((
            Text::new("0%".to_string()),
            ui_font(&fonts.mono, 12.0),
            TextColor(c(120, 235, 160)),
            LoadingPercent,
        ))
        .id();
    bind_text(commands, pct, loading_percent_eased_text);

    commands.entity(footer).add_children(&[proj, loc, track, pct]);

    commands.entity(win).add_children(&[header, body, footer]);
    win
}

// ── Metrics read from LoadingTasks ───────────────────────────────────────────

/// `f32` doesn't implement `Eq`, so wrap it for `bind_with`'s `PartialEq` diff.
#[derive(PartialEq)]
struct OrderedF32(f32);

fn aggregate(world: &World) -> (f32, u32) {
    let Some(tasks) = world.get_resource::<LoadingTasks>() else {
        return (1.0, 100);
    };
    let (sc, st): (u64, u64) = tasks
        .tasks()
        .iter()
        .fold((0, 0), |(sc, st), (_, t)| (sc + t.completed as u64, st + t.total as u64));
    let frac = if st == 0 { 1.0 } else { (sc as f32 / st as f32).clamp(0.0, 1.0) };
    (frac, (frac * 100.0).round() as u32)
}

/// Display fraction. When we have real byte totals, use the eased [`LoadingAnim`]
/// value (smooth motion); otherwise fall back to the task-count aggregate.
fn progress_fraction(world: &World) -> (f32, u32) {
    let has_data = world.get_resource::<LoadingBytes>().is_some_and(|b| b.total > 0)
        || world.get_resource::<TextureLoadProgress>().is_some_and(|t| t.total > 0);
    if has_data {
        let f = world.get_resource::<LoadingAnim>().map(|a| a.displayed).unwrap_or(0.0).clamp(0.0, 1.0);
        return (f, (f * 100.0).round() as u32);
    }
    aggregate(world)
}

/// Real (loaded, total) bytes when the byte loader has data.
fn real_bytes(world: &World) -> Option<(u64, u64)> {
    world
        .get_resource::<LoadingBytes>()
        .filter(|b| b.total > 0)
        .map(|b| (b.loaded, b.total))
}

/// Real (loaded, total) texture counts during the decode phase.
fn real_textures(world: &World) -> Option<(u32, u32)> {
    world
        .get_resource::<TextureLoadProgress>()
        .filter(|t| t.total > 0)
        .map(|t| (t.loaded, t.total))
}

fn loading_fraction(world: &World) -> OrderedF32 {
    OrderedF32(aggregate(world).0)
}

fn loading_percent_text(world: &World) -> String {
    format!("{}%", aggregate(world).1)
}

fn loading_detail_text(world: &World) -> String {
    let Some(tasks) = world.get_resource::<LoadingTasks>() else {
        return String::new();
    };
    tasks
        .tasks()
        .iter()
        .find(|(_, t)| !t.is_done())
        .map(|(_, t)| match &t.detail {
            Some(d) => format!("{} — {}", t.label, d),
            None => t.label.clone(),
        })
        .unwrap_or_default()
}

fn loading_fill_color(world: &World) -> [u8; 3] {
    let t = world.get_resource::<Time>().map(|t| t.elapsed_secs()).unwrap_or(0.0);
    let pulse = 0.5 + 0.5 * (t * 3.2).sin();
    [(90.0 + 40.0 * pulse) as u8, (180.0 - 30.0 * pulse) as u8, 255]
}

/// Eased display fraction (the smooth [`LoadingAnim`] value), for the full-width
/// terminal bar.
fn loading_fraction_eased(world: &World) -> OrderedF32 {
    OrderedF32(progress_fraction(world).0)
}

/// Eased percent readout for the full-width terminal bar.
fn loading_percent_eased_text(world: &World) -> String {
    format!("{}%", progress_fraction(world).1)
}

// ── Hacky terminal boot log ──────────────────────────────────────────────────

/// Fake boot-sequence lines; revealed progressively as loading advances.
const BOOT_LINES: [&str; 12] = [
    "> initializing renzora runtime",
    "> mounting virtual filesystem ......... [ ok ]",
    "> spawning ecs world .................. [ ok ]",
    "> loading shader cache ................ [ ok ]",
    "> linking dynamic plugins ............. [ ok ]",
    "> decoding scene assets ...............",
    "> building render graph ...............",
    "> compiling gpu pipelines .............",
    "> warming material cache ..............",
    "> calibrating cameras ................. [ ok ]",
    "> resolving entity hierarchy ..........",
    "> finalizing world state ..............",
];

/// Build the streaming terminal log: the first N boot lines (N scales with
/// progress), then an active status line with a blinking cursor.
fn terminal_text(world: &World) -> String {
    let (frac, pct) = progress_fraction(world);
    let n = ((frac * BOOT_LINES.len() as f32).ceil() as usize).clamp(1, BOOT_LINES.len());
    let t = world.get_resource::<Time>().map(|t| t.elapsed_secs()).unwrap_or(0.0);
    let cursor = if (t * 2.0).floor() as i64 % 2 == 0 { "_" } else { " " };

    let frames = ['|', '/', '-', '\\'];
    let spinner = frames[((t * 10.0) as i64).rem_euclid(4) as usize];

    let mut s = String::new();
    for line in &BOOT_LINES[..n] {
        s.push_str(line);
        s.push('\n');
    }
    if pct >= 100 {
        s.push_str(&format!("> ready. launching editor {cursor}"));
    } else {
        let detail = loading_detail_text(world);
        if !detail.is_empty() {
            s.push_str(&format!("> {detail}\n"));
        }
        // Decode phase: show the real texture count (genuine sub-asset progress).
        // GLB-read phase: show real total MB. Otherwise just spinner + percent.
        if let Some((done, total)) = real_textures(world) {
            s.push_str(&format!("> decoding textures  {done} / {total}  {spinner}  [{pct:>3}%] {cursor}"));
        } else if let Some((_, total)) = real_bytes(world) {
            let tmb = total as f64 / 1_048_576.0;
            let lmb = frac as f64 * tmb * 2.0; // first half is the GLB read
            s.push_str(&format!("> reading  {lmb:6.2} / {tmb:6.2} MB  {spinner}  [{pct:>3}%] {cursor}"));
        } else {
            s.push_str(&format!("> working  {spinner}  [{pct:>3}%] {cursor}"));
        }
    }
    s
}
