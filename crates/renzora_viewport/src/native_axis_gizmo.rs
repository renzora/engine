//! Native (bevy_ui) axis-orientation gizmo (top-right of each viewport) —
//! replaces the egui `render_axis_gizmo`.
//!
//! Six "tip" nodes (X/Y/Z balls + their negative rings) are re-projected from
//! [`CameraOrbitSnapshot`] every frame using the same yaw/pitch math the egui
//! version used, and repositioned/faded/z-ordered by depth. Clicking a tip snaps
//! the camera to that view (`ViewportSettings::pending_view_angle`); dragging the
//! gizmo background orbits (accumulates into [`NavOverlayState`]'s orbit deltas,
//! which the camera system already consumes). The cluster is an [`OverlaySurface`]
//! so the camera won't free-orbit while the cursor is over it.

use std::f32::consts::{FRAC_PI_2, PI};
use std::sync::atomic::Ordering;

use bevy::input::mouse::MouseMotion;
use bevy::prelude::*;
use bevy::ui::RelativeCursorPosition;

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

#[derive(Component)]
struct AxisGizmoRoot;

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
        (gizmo_input, gizmo_layout).run_if(in_state(SplashState::Editor)),
    );
}

fn rgba((r, g, b): (u8, u8, u8), a: f32) -> Color {
    Color::srgba(r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, a)
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

    // (dir, color, label, target_yaw, target_pitch, positive)
    let axes: [(Vec3, (u8, u8, u8), &str, f32, f32, bool); 6] = [
        (Vec3::X, (237, 76, 92), "X", FRAC_PI_2, 0.0, true),
        (Vec3::Y, (139, 201, 63), "Y", 0.0, FRAC_PI_2, true),
        (Vec3::Z, (68, 138, 255), "Z", 0.0, 0.0, true),
        (Vec3::NEG_X, (150, 50, 60), "", -FRAC_PI_2, 0.0, false),
        (Vec3::NEG_Y, (80, 120, 40), "", 0.0, -FRAC_PI_2, false),
        (Vec3::NEG_Z, (40, 80, 150), "", PI, 0.0, false),
    ];

    for (dir, color, label, yaw, pitch, positive) in axes {
        let d = if positive { 18.0 } else { 12.0 };
        let tip = commands
            .spawn((
                Node {
                    position_type: PositionType::Absolute,
                    width: Val::Px(d),
                    height: Val::Px(d),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    border_radius: BorderRadius::all(Val::Px(d / 2.0)),
                    border: if positive {
                        UiRect::ZERO
                    } else {
                        UiRect::all(Val::Px(1.5))
                    },
                    ..default()
                },
                BackgroundColor(if positive { rgb(color) } else { Color::NONE }),
                BorderColor::all(rgb(color)),
                ZIndex(0),
                Interaction::default(),
                AxisTip {
                    dir,
                    yaw,
                    pitch,
                    positive,
                    color,
                },
                Name::new("axis-tip"),
            ))
            .id();
        if positive && !label.is_empty() {
            let t = commands
                .spawn((
                    Text::new(label),
                    ui_font(&fonts.ui, 10.0),
                    TextColor(Color::WHITE),
                ))
                .id();
            commands.entity(tip).add_child(t);
        }
        commands.entity(root).add_child(tip);
    }
    root
}

/// Reposition / fade / z-order the tips from the camera orbit each frame, and
/// hide the gizmo when the setting is off.
fn gizmo_layout(
    orbit: Option<Res<CameraOrbitSnapshot>>,
    settings: Option<Res<ViewportSettings>>,
    mut roots: Query<&mut Node, (With<AxisGizmoRoot>, Without<AxisTip>)>,
    mut tips: Query<(
        &AxisTip,
        &mut Node,
        &mut BackgroundColor,
        &mut BorderColor,
        &mut ZIndex,
    )>,
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
    let (cos_yaw, sin_yaw) = (orbit.yaw.cos(), orbit.yaw.sin());
    let (cos_pitch, sin_pitch) = (orbit.pitch.cos(), orbit.pitch.sin());

    for (tip, mut node, mut bg, mut border, mut z) in &mut tips {
        let dir = tip.dir;
        // Rotate by yaw then pitch (same as egui).
        let r = Vec3::new(
            dir.x * cos_yaw + dir.z * sin_yaw,
            dir.y,
            -dir.x * sin_yaw + dir.z * cos_yaw,
        );
        let v = Vec3::new(
            r.x,
            r.y * cos_pitch + r.z * sin_pitch,
            -r.y * sin_pitch + r.z * cos_pitch,
        );
        let off = Vec2::new(v.x * AXIS_LEN, -v.y * AXIS_LEN);
        let d = if tip.positive { 18.0 } else { 12.0 };
        node.left = Val::Px(CENTRE + off.x - d / 2.0);
        node.top = Val::Px(CENTRE + off.y - d / 2.0);

        // Fade axes pointing away; z-order back-to-front.
        let alpha = if v.z < -0.1 { 0.4 } else { 1.0 };
        if tip.positive {
            bg.0 = rgba(tip.color, alpha);
        } else {
            *border = BorderColor::all(rgba(tip.color, alpha));
        }
        *z = ZIndex((v.z * 100.0) as i32);
    }
}

/// Tip-click → snap view; background-drag → orbit.
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
        // A tip directly under the cursor snaps the camera to that view.
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
