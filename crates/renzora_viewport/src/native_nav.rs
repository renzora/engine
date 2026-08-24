//! Native (bevy_ui) viewport nav overlay — the home, pan, zoom and grid buttons
//! on the right side of each viewport.
//!
//! Pan and zoom are press-and-drag: while held they accumulate `MouseMotion`
//! into [`NavOverlayState`]'s atomic deltas (the same ones the camera system
//! already consumes). Home and Grid are plain clicks — home raises
//! `ViewportSettings::pending_camera_home` for the camera controller, grid flips
//! `show_grid` directly. The cluster is an [`OverlaySurface`] so hovering it
//! suppresses viewport hover (the camera won't orbit / box-select won't start
//! under the buttons).
//!
//! Grid is a duplicate of the toolbar's Display dropdown switch and the one in
//! Settings → Viewport, which is why it was taken out of here once. It's back by
//! request: the grid gets flipped often enough while modelling that a click on
//! the overlay beats two clicks through a dropdown.

use std::sync::atomic::Ordering;

use bevy::input::mouse::MouseMotion;
use bevy::prelude::*;
use bevy::ui::RelativeCursorPosition;

use renzora::core::viewport_types::{NavOverlayState, ViewportSettings};
use renzora_editor_framework::SplashState;
use renzora_ember::font::{icon_text, EmberFonts};
use renzora_ember::theme::{accent, hover_bg, panel_bg, rgb};
use renzora_ember::widgets::OverlaySurface;

use crate::{AXIS_GIZMO_MARGIN, AXIS_GIZMO_SIZE};

const BTN: f32 = 36.0;

#[derive(Component, Clone, Copy, PartialEq, Eq)]
enum NavButton {
    /// Click: send the camera back to its default framing.
    Home,
    Pan,
    Zoom,
    /// Click: flip the floor grid.
    Grid,
}

/// Which nav drag-button is currently latched (continues off the button until
/// mouse release, mirroring egui's pointer-latched drag).
#[derive(Resource, Default)]
struct NavDragLatch(Option<NavButton>);

pub(crate) fn register(app: &mut App) {
    app.init_resource::<NavDragLatch>();
    app.add_systems(
        Update,
        (nav_input, nav_visuals).run_if(in_state(SplashState::Editor)),
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
                // No toolbar offset: the bar is above the scene now, not over it.
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

    let home = nav_btn(commands, fonts, NavButton::Home, "house", 0.0);
    let pan = nav_btn(commands, fonts, NavButton::Pan, "hand", 0.0);
    let zoom = nav_btn(commands, fonts, NavButton::Zoom, "magnifying-glass", 0.0);
    // Extra gap: grid is a toggle, not one of the camera controls above it, and
    // without the break it reads as a third thing you're meant to drag.
    let grid = nav_btn(commands, fonts, NavButton::Grid, "grid-four", 8.0);
    commands
        .entity(cluster)
        .add_children(&[home, pan, zoom, grid]);
    // Hide the nav buttons during play mode for a clean game view, and in 2D
    // view (they're 3D-orbit pan/zoom controls).
    renzora_ember::reactive::tracked::bind_display(commands, cluster, |w| {
        let in_play = w
            .get_resource::<renzora::core::PlayModeState>()
            .map(|p| p.is_in_play_mode())
            .unwrap_or(false);
        let in_2d = w
            .get_resource::<renzora::core::viewport_types::ViewportSettings>()
            .map(|s| s.viewport_view == renzora::core::viewport_types::ViewportView::Two)
            .unwrap_or(false);
        !in_play && !in_2d
    });
    cluster
}

fn nav_btn(
    commands: &mut Commands,
    fonts: &EmberFonts,
    kind: NavButton,
    icon: &str,
    top_margin: f32,
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
    // Let clicks fall through the glyph to the button, or a press on the icon
    // itself (dead-center of the button) never reaches the button's `Interaction`
    // and the Grid/Icons toggles silently do nothing.
    commands.entity(g).insert(bevy::picking::Pickable::IGNORE);
    commands.entity(b).add_child(g);
    b
}

/// Recolor the buttons: accent while their drag is latched (or, for Grid, while
/// the toggle is on), hover wash on hover.
fn nav_visuals(
    nav: Res<NavOverlayState>,
    settings: Option<Res<ViewportSettings>>,
    mut buttons: Query<(&NavButton, &Interaction, &mut BackgroundColor)>,
) {
    let grid_on = settings.is_some_and(|s| s.show_grid);
    for (kind, interaction, mut bg) in &mut buttons {
        let active = match kind {
            NavButton::Pan => nav.pan_dragging.load(Ordering::Relaxed),
            NavButton::Zoom => nav.zoom_dragging.load(Ordering::Relaxed),
            // Home is a one-shot: there is no state for it to sit lit up in.
            NavButton::Home => false,
            NavButton::Grid => grid_on,
        };
        bg.0 = if active {
            rgb(accent())
        } else if matches!(interaction, Interaction::Hovered | Interaction::Pressed) {
            rgb(hover_bg())
        } else {
            resting()
        };
    }
}

/// Pan/Zoom press-and-drag → accumulate into the camera-consumed atomics;
/// Home/Grid press → the one-shot they stand for.
fn nav_input(
    mouse: Res<ButtonInput<MouseButton>>,
    mut motion: MessageReader<MouseMotion>,
    nav: Res<NavOverlayState>,
    mut latch: ResMut<NavDragLatch>,
    mut settings: Option<ResMut<ViewportSettings>>,
    buttons: Query<(&NavButton, &Interaction)>,
) {
    if mouse.just_pressed(MouseButton::Left) {
        let pressed = buttons
            .iter()
            .find(|(_, i)| **i == Interaction::Pressed)
            .map(|(kind, _)| *kind);
        match pressed {
            Some(kind @ (NavButton::Pan | NavButton::Zoom)) => {
                latch.0 = Some(kind);
                nav.pan_dragging
                    .store(kind == NavButton::Pan, Ordering::Relaxed);
                nav.zoom_dragging
                    .store(kind == NavButton::Zoom, Ordering::Relaxed);
            }
            // Raise the flag rather than move the camera here: the orbit state
            // lives in renzora_camera, which this crate deliberately doesn't
            // link. The controller consumes it next frame.
            Some(NavButton::Home) => {
                if let Some(s) = settings.as_mut() {
                    s.pending_camera_home = true;
                }
            }
            Some(NavButton::Grid) => {
                if let Some(s) = settings.as_mut() {
                    let on = s.show_grid;
                    s.show_grid = !on;
                }
            }
            None => {}
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
            // Negated: mouse Y grows downward, and the camera reads a positive
            // delta as "come closer". Dragging **up** zooms in, which is the way
            // round every other zoom in the editor works.
            nav.zoom_delta_y
                .fetch_add((-delta.y * 1000.0) as i32, Ordering::Relaxed);
        }
        // Home and Grid fire on press and never latch, so there is no drag of
        // theirs to accumulate.
        NavButton::Home | NavButton::Grid => {}
    }
}

