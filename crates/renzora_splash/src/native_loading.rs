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

use crate::loading::{EditorLoadingOverlayActive, LoadingTasks};
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

pub(crate) fn register(app: &mut App) {
    app.add_systems(
        Update,
        (manage_loading_screen, manage_editor_overlay),
    );
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
        let mut queue = CommandQueue::default();
        {
            let mut commands = Commands::new(&mut queue, world);
            spawn_loading(&mut commands, &fonts, false);
        }
        queue.apply(world);
    } else if !want && !existing.is_empty() {
        for e in existing {
            world.entity_mut(e).despawn();
        }
    }
}

fn manage_editor_overlay(world: &mut World) {
    let in_editor = matches!(world.resource::<State<SplashState>>().get(), SplashState::Editor);
    let active = world.get_resource::<EditorLoadingOverlayActive>().is_some_and(|a| a.0);
    let bevy_ui = world
        .get_resource::<renzora::core::EditorUiBackend>()
        .is_some_and(|b| b.is_bevy_ui());
    let want = in_editor && active && bevy_ui;

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
            spawn_loading(&mut commands, &fonts, true);
        }
        queue.apply(world);
    } else if !want && !existing.is_empty() {
        for e in existing {
            world.entity_mut(e).despawn();
        }
    }
}

fn spawn_loading(commands: &mut Commands, fonts: &EmberFonts, modal: bool) {
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
        BackgroundColor(if modal { ca(8, 10, 16, 180) } else { c(10, 10, 14) }),
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

    // Centered letterbox.
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

    // Title + current-task detail.
    let title = commands
        .spawn((Text::new("Loading".to_string()), ui_font(&fonts.ui, 18.0), TextColor(c(240, 240, 248))))
        .id();
    let detail = commands
        .spawn((Text::new(String::new()), ui_font(&fonts.ui, 12.0), TextColor(c(170, 175, 190)), LoadingDetail))
        .id();
    bind_text(commands, detail, loading_detail_text);

    // Bar row: track (with fill) + percent.
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
            Node { min_width: Val::Px(40.0), ..default() },
            Text::new("0%".to_string()),
            ui_font(&fonts.mono, 13.0),
            TextColor(c(220, 225, 240)),
            LoadingPercent,
        ))
        .id();
    bind_text(commands, percent, loading_percent_text);
    commands.entity(row).add_children(&[track, percent]);

    commands.entity(letterbox).add_children(&[title, detail, row]);
    commands.entity(backdrop).add_child(letterbox);
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
