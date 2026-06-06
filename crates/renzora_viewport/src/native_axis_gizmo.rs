//! Native (bevy_ui) axis-orientation gizmo (top-right of each viewport) —
//! replaces the egui `render_axis_gizmo`.
//!
//! A circular **backplate** (drag it to orbit the camera) sits behind six "tip"
//! balls (filled X/Y/Z + their negatives) connected to the centre by **axis
//! lines**. Tips/lines are re-projected from [`CameraOrbitSnapshot`] every frame
//! (same yaw/pitch math the egui version used), faded + ZIndex-ordered by depth.
//! Clicking a tip snaps the camera to that view
//! (`ViewportSettings::pending_view_angle`); dragging the backplate orbits
//! (accumulates into [`NavOverlayState`]'s orbit deltas, which the camera system
//! already consumes). The cluster is an [`OverlaySurface`] so the camera won't
//! free-orbit while the cursor is over it.

use std::f32::consts::{FRAC_PI_2, PI};
use std::sync::atomic::Ordering;

use bevy::input::mouse::MouseMotion;
use bevy::math::Rot2;
use bevy::prelude::*;
use bevy::ui::{RelativeCursorPosition, UiTransform};

use renzora::core::viewport_types::{
    CameraOrbitSnapshot, NavOverlayState, ViewAngleCommand, ViewportSettings,
};
use renzora_editor::SplashState;
use renzora_ember::font::{ui_font, EmberFonts};
use renzora_ember::theme::rgb;
use renzora_ember::widgets::OverlaySurface;

use crate::{AXIS_GIZMO_MARGIN, AXIS_GIZMO_SIZE};

/// Half-length of the projected axes (matches egui: SIZE/2 - 12).
const AXIS_LEN: f32 = AXIS_GIZMO_SIZE / 2.0 - 12.0;
/// Container-local centre.
const CENTRE: f32 = AXIS_GIZMO_SIZE / 2.0;
const POS_D: f32 = 18.0;
const NEG_D: f32 = 12.0;

#[derive(Component)]
struct AxisGizmoRoot;

#[derive(Component)]
struct AxisBackplate;

#[derive(Component)]
struct AxisLine {
    dir: Vec3,
    color: (u8, u8, u8),
}

#[derive(Component)]
struct AxisTip {
    dir: Vec3,
    yaw: f32,
    pitch: f32,
    positive: bool,
    color: (u8, u8, u8),
}

/// Whether the gizmo is currently latched for an orbit-drag.
#[derive(Resource, Default)]
struct GizmoOrbitLatch(bool);

pub(crate) fn register(app: &mut App) {
    app.init_resource::<GizmoOrbitLatch>();
    app.add_systems(
        Update,
        (gizmo_input, gizmo_layout, gizmo_backplate).run_if(in_state(SplashState::Editor)),
    );
}

fn rgba((r, g, b): (u8, u8, u8), a: f32) -> Color {
    Color::srgba(r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, a)
}

/// Project an axis direction to a (screen-space offset, depth) pair.
fn project(dir: Vec3, cy: f32, sy: f32, cp: f32, sp: f32) -> (Vec2, f32) {
    let r = Vec3::new(dir.x * cy + dir.z * sy, dir.y, -dir.x * sy + dir.z * cy);
    let v = Vec3::new(r.x, r.y * cp + r.z * sp, -r.y * sp + r.z * cp);
    (Vec2::new(v.x * AXIS_LEN, -v.y * AXIS_LEN), v.z)
}

/// Build the gizmo cluster on a viewport content node's top-right. Returns the root.
pub(crate) fn build(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let root = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                right: Val::Px(AXIS_GIZMO_MARGIN),
                top: Val::Px(AXIS_GIZMO_MARGIN),
                width: Val::Px(AXIS_GIZMO_SIZE),
                height: Val::Px(AXIS_GIZMO_SIZE),
                ..default()
            },
            RelativeCursorPosition::default(),
            OverlaySurface,
            Interaction::default(),
            AxisGizmoRoot,
            Name::new("axis-gizmo"),
        ))
        .id();

    // Backplate: a circle that fills the square — the drag-to-orbit target.
    let backplate = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                width: Val::Px(AXIS_GIZMO_SIZE),
                height: Val::Px(AXIS_GIZMO_SIZE),
                border_radius: BorderRadius::all(Val::Px(AXIS_GIZMO_SIZE / 2.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.22)),
            ZIndex(-100),
            bevy::picking::Pickable::IGNORE,
            AxisBackplate,
            Name::new("axis-gizmo-backplate"),
        ))
        .id();
    commands.entity(root).add_child(backplate);

    // (dir, color, label, target_yaw, target_pitch, positive)
    let axes: [(Vec3, (u8, u8, u8), &str, f32, f32, bool); 6] = [
        (Vec3::X, (237, 76, 92), "X", FRAC_PI_2, 0.0, true),
        (Vec3::Y, (139, 201, 63), "Y", 0.0, FRAC_PI_2, true),
        (Vec3::Z, (68, 138, 255), "Z", 0.0, 0.0, true),
        (Vec3::NEG_X, (150, 50, 60), "", -FRAC_PI_2, 0.0, false),
        (Vec3::NEG_Y, (80, 120, 40), "", 0.0, -FRAC_PI_2, false),
        (Vec3::NEG_Z, (40, 80, 150), "", PI, 0.0, false),
    ];

    // Lines first (under the tips), then tips.
    for (dir, color, _, _, _, positive) in axes {
        if positive {
            commands.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    width: Val::Px(0.0),
                    height: Val::Px(2.5),
                    border_radius: BorderRadius::all(Val::Px(1.25)),
                    ..default()
                },
                BackgroundColor(rgb(color)),
                UiTransform::default(),
                ZIndex(0),
                bevy::picking::Pickable::IGNORE,
                AxisLine { dir, color },
                ChildOf(root),
                Name::new("axis-line"),
            ));
        }
    }

    for (dir, color, label, yaw, pitch, positive) in axes {
        let d = if positive { POS_D } else { NEG_D };
        let tip = commands
            .spawn((
                Node {
                    position_type: PositionType::Absolute,
                    width: Val::Px(d),
                    height: Val::Px(d),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    border_radius: BorderRadius::all(Val::Px(d / 2.0)),
                    ..default()
                },
                BackgroundColor(rgb(color)),
                ZIndex(100),
                Interaction::default(),
                AxisTip {
                    dir,
                    yaw,
                    pitch,
                    positive,
                    color,
                },
                ChildOf(root),
                Name::new("axis-tip"),
            ))
            .id();
        if positive && !label.is_empty() {
            let t = commands
                .spawn((
                    Text::new(label),
                    ui_font(&fonts.ui, 10.0),
                    TextColor(Color::WHITE),
                    bevy::picking::Pickable::IGNORE,
                ))
                .id();
            commands.entity(tip).add_child(t);
        }
    }
    root
}

/// Reposition / fade / z-order the tips + lines from the camera orbit each frame,
/// and hide the gizmo when the setting is off.
fn gizmo_layout(
    orbit: Option<Res<CameraOrbitSnapshot>>,
    settings: Option<Res<ViewportSettings>>,
    mut roots: Query<&mut Node, (With<AxisGizmoRoot>, Without<AxisTip>, Without<AxisLine>)>,
    mut tips: Query<
        (&AxisTip, &mut Node, &mut BackgroundColor, &mut ZIndex),
        Without<AxisLine>,
    >,
    mut lines: Query<
        (&AxisLine, &mut Node, &mut UiTransform, &mut BackgroundColor, &mut ZIndex),
        Without<AxisTip>,
    >,
) {
    let show = settings.map(|s| s.show_axis_gizmo).unwrap_or(true);
    for mut node in &mut roots {
        let want = if show { Display::Flex } else { Display::None };
        if node.display != want {
            node.display = want;
        }
    }
    if !show {
        return;
    }
    let Some(orbit) = orbit else { return };
    let (cy, sy) = (orbit.yaw.cos(), orbit.yaw.sin());
    let (cp, sp) = (orbit.pitch.cos(), orbit.pitch.sin());

    for (tip, mut node, mut bg, mut z) in &mut tips {
        let (off, depth) = project(tip.dir, cy, sy, cp, sp);
        let d = if tip.positive { POS_D } else { NEG_D };
        node.left = Val::Px(CENTRE + off.x - d / 2.0);
        node.top = Val::Px(CENTRE + off.y - d / 2.0);
        let alpha = if depth < -0.1 { 0.45 } else { 1.0 };
        bg.0 = rgba(tip.color, alpha);
        *z = ZIndex(100 + (depth * 10.0) as i32);
    }

    for (line, mut node, mut tf, mut bg, mut z) in &mut lines {
        let (off, depth) = project(line.dir, cy, sy, cp, sp);
        let len = off.length();
        node.width = Val::Px(len);
        // Centre the line on the midpoint, then rotate about its own centre so it
        // spans centre -> tip.
        node.left = Val::Px(CENTRE + off.x / 2.0 - len / 2.0);
        node.top = Val::Px(CENTRE + off.y / 2.0 - 1.25);
        *tf = UiTransform::from_rotation(Rot2::radians(off.y.atan2(off.x)));
        let alpha = if depth < -0.1 { 0.4 } else { 0.9 };
        bg.0 = rgba(line.color, alpha);
        *z = ZIndex(10 + (depth * 10.0) as i32);
    }
}

/// Brighten the backplate while orbiting (or leave it subtle).
fn gizmo_backplate(
    nav: Option<Res<NavOverlayState>>,
    mut plates: Query<&mut BackgroundColor, With<AxisBackplate>>,
) {
    let active = nav.is_some_and(|n| n.orbit_dragging.load(Ordering::Relaxed));
    let a = if active { 0.38 } else { 0.22 };
    for mut bg in &mut plates {
        bg.0 = Color::srgba(0.0, 0.0, 0.0, a);
    }
}

/// Tip-click → snap view; backplate-drag → orbit.
fn gizmo_input(
    mouse: Res<ButtonInput<MouseButton>>,
    mut motion: MessageReader<MouseMotion>,
    nav: Res<NavOverlayState>,
    mut latch: ResMut<GizmoOrbitLatch>,
    tips: Query<(&AxisTip, &Interaction)>,
    roots: Query<&Interaction, With<AxisGizmoRoot>>,
    settings: Option<ResMut<ViewportSettings>>,
) {
    if mouse.just_pressed(MouseButton::Left) {
        let snap = tips
            .iter()
            .find(|(_, i)| **i == Interaction::Pressed)
            .map(|(tip, _)| (tip.yaw, tip.pitch));
        if let Some((yaw, pitch)) = snap {
            if let Some(mut s) = settings {
                s.pending_view_angle = Some(ViewAngleCommand { yaw, pitch });
            }
        } else if roots
            .iter()
            .any(|i| matches!(i, Interaction::Hovered | Interaction::Pressed))
        {
            latch.0 = true;
            nav.orbit_dragging.store(true, Ordering::Relaxed);
        }
    }
    if mouse.just_released(MouseButton::Left) {
        latch.0 = false;
        nav.orbit_dragging.store(false, Ordering::Relaxed);
    }

    if !latch.0 {
        for _ in motion.read() {}
        return;
    }
    let mut delta = Vec2::ZERO;
    for ev in motion.read() {
        delta += ev.delta;
    }
    if delta != Vec2::ZERO {
        nav.orbit_delta_x
            .fetch_add((delta.x * 1000.0) as i32, Ordering::Relaxed);
        nav.orbit_delta_y
            .fetch_add((delta.y * 1000.0) as i32, Ordering::Relaxed);
    }
}
