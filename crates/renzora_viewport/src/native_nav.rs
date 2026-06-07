//! Native (bevy_ui) viewport nav overlay — pan/zoom drag-buttons + grid/scene-icon
//! toggles stacked on the right side of each viewport. Replaces the egui
//! `toolbar::render_nav_overlay`.
//!
//! Pan/Zoom are press-and-drag: while held they accumulate `MouseMotion` into
//! [`NavOverlayState`]'s atomic deltas (the same ones the camera system already
//! consumes). Grid/Icons are click toggles on [`ViewportSettings`]. The cluster
//! is an [`OverlaySurface`] so hovering it suppresses viewport hover (the camera
//! won't orbit / box-select won't start under the buttons).

use std::sync::atomic::Ordering;

use bevy::input::mouse::MouseMotion;
use bevy::prelude::*;
use bevy::ui::RelativeCursorPosition;

use renzora::core::viewport_types::{NavOverlayState, ViewportSettings};
use renzora_editor_framework::SplashState;
use renzora_ember::font::{icon_glyph, icon_text, EmberFonts};
use renzora_ember::theme::{accent, hover_bg, panel_bg, rgb};
use renzora_ember::widgets::OverlaySurface;

use crate::{AXIS_GIZMO_MARGIN, AXIS_GIZMO_SIZE};

const BTN: f32 = 36.0;

#[derive(Component, Clone, Copy, PartialEq, Eq)]
enum NavButton {
    Pan,
    Zoom,
    Grid,
    Icons,
}

/// Marks the glyph text of the Icons button so its eye/eye-slash can be swapped.
#[derive(Component)]
struct NavIconsGlyph;

/// Which nav drag-button is currently latched (continues off the button until
/// mouse release, mirroring egui's pointer-latched drag).
#[derive(Resource, Default)]
struct NavDragLatch(Option<NavButton>);

pub(crate) fn register(app: &mut App) {
    app.init_resource::<NavDragLatch>();
    app.add_systems(
        Update,
        (nav_drag, nav_click, nav_visuals).run_if(in_state(SplashState::Editor)),
    );
}

fn resting() -> Color {
    let (r, g, b) = panel_bg();
    Color::srgba(r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, 0.55)
}

/// Build the nav cluster as an absolutely-positioned column on the right edge of
/// a viewport content node (below where the axis gizmo sits). Returns the cluster
/// root.
pub(crate) fn build(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let cluster = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                right: Val::Px(8.0),
                top: Val::Px(AXIS_GIZMO_SIZE + AXIS_GIZMO_MARGIN + 24.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(1.0),
                ..default()
            },
            RelativeCursorPosition::default(),
            OverlaySurface,
            Name::new("nav-overlay"),
        ))
        .id();

    let pan = nav_btn(commands, fonts, NavButton::Pan, "hand", 0.0, false);
    let zoom = nav_btn(commands, fonts, NavButton::Zoom, "magnifying-glass", 0.0, false);
    let grid = nav_btn(commands, fonts, NavButton::Grid, "grid-four", 6.0, false);
    let icons = nav_btn(commands, fonts, NavButton::Icons, "eye", 0.0, true);
    commands
        .entity(cluster)
        .add_children(&[pan, zoom, grid, icons]);
    cluster
}

fn nav_btn(
    commands: &mut Commands,
    fonts: &EmberFonts,
    kind: NavButton,
    icon: &str,
    top_margin: f32,
    tag_glyph: bool,
) -> Entity {
    let b = commands
        .spawn((
            Node {
                width: Val::Px(BTN),
                height: Val::Px(BTN),
                margin: UiRect::top(Val::Px(top_margin)),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border_radius: BorderRadius::all(Val::Px(BTN / 2.0)),
                ..default()
            },
            BackgroundColor(resting()),
            Interaction::default(),
            kind,
            Name::new("nav-btn"),
        ))
        .id();
    let g = icon_text(commands, &fonts.phosphor, icon, (235, 235, 240), 16.0);
    if tag_glyph {
        commands.entity(g).insert(NavIconsGlyph);
    }
    commands.entity(b).add_child(g);
    b
}

/// Recolor buttons from state and swap the Icons glyph (eye / eye-slash).
fn nav_visuals(
    nav: Res<NavOverlayState>,
    settings: Option<Res<ViewportSettings>>,
    mut buttons: Query<(&NavButton, &Interaction, &mut BackgroundColor)>,
    glyphs: Query<Entity, With<NavIconsGlyph>>,
    mut texts: Query<&mut Text>,
) {
    let (show_grid, show_icons) = settings
        .map(|s| (s.show_grid, s.show_scene_icons))
        .unwrap_or((true, true));

    for (kind, interaction, mut bg) in &mut buttons {
        let active = match kind {
            NavButton::Pan => nav.pan_dragging.load(Ordering::Relaxed),
            NavButton::Zoom => nav.zoom_dragging.load(Ordering::Relaxed),
            NavButton::Grid => show_grid,
            NavButton::Icons => show_icons,
        };
        bg.0 = if active {
            rgb(accent())
        } else if matches!(interaction, Interaction::Hovered | Interaction::Pressed) {
            rgb(hover_bg())
        } else {
            resting()
        };
    }

    let ch = icon_glyph(if show_icons { "eye" } else { "eye-slash" }).unwrap_or('\u{E4C6}');
    for e in &glyphs {
        if let Ok(mut t) = texts.get_mut(e) {
            *t = Text::new(ch.to_string());
        }
    }
}

/// Pan/Zoom press-and-drag → accumulate into the camera-consumed atomics.
fn nav_drag(
    mouse: Res<ButtonInput<MouseButton>>,
    mut motion: MessageReader<MouseMotion>,
    nav: Res<NavOverlayState>,
    mut latch: ResMut<NavDragLatch>,
    buttons: Query<(&NavButton, &Interaction)>,
) {
    if mouse.just_pressed(MouseButton::Left) {
        for (kind, interaction) in &buttons {
            if *interaction == Interaction::Pressed
                && matches!(kind, NavButton::Pan | NavButton::Zoom)
            {
                latch.0 = Some(*kind);
                nav.pan_dragging
                    .store(*kind == NavButton::Pan, Ordering::Relaxed);
                nav.zoom_dragging
                    .store(*kind == NavButton::Zoom, Ordering::Relaxed);
                break;
            }
        }
    }
    if mouse.just_released(MouseButton::Left) {
        latch.0 = None;
        nav.pan_dragging.store(false, Ordering::Relaxed);
        nav.zoom_dragging.store(false, Ordering::Relaxed);
    }

    let Some(kind) = latch.0 else {
        // Drain so a future latch doesn't pick up pre-drag motion.
        for _ in motion.read() {}
        return;
    };
    let mut delta = Vec2::ZERO;
    for ev in motion.read() {
        delta += ev.delta;
    }
    if delta == Vec2::ZERO {
        return;
    }
    match kind {
        NavButton::Pan => {
            nav.pan_delta_x
                .fetch_add((delta.x * 1000.0) as i32, Ordering::Relaxed);
            nav.pan_delta_y
                .fetch_add((delta.y * 1000.0) as i32, Ordering::Relaxed);
        }
        NavButton::Zoom => {
            nav.zoom_delta_y
                .fetch_add((delta.y * 1000.0) as i32, Ordering::Relaxed);
        }
        _ => {}
    }
}

/// Grid / Icons click toggles.
fn nav_click(
    buttons: Query<(&NavButton, &Interaction), Changed<Interaction>>,
    settings: Option<ResMut<ViewportSettings>>,
) {
    let Some(mut settings) = settings else {
        return;
    };
    for (kind, interaction) in &buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match kind {
            NavButton::Grid => settings.show_grid = !settings.show_grid,
            NavButton::Icons => settings.show_scene_icons = !settings.show_scene_icons,
            _ => {}
        }
    }
}
