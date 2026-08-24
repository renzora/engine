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
use renzora_editor_framework::SplashState;
use renzora_ember::font::{ui_font, EmberFonts};
use renzora_ember::theme::rgb;
use renzora_ember::widgets::OverlaySurface;

use crate::{AXIS_GIZMO_MARGIN, AXIS_GIZMO_SIZE};

/// Half-length of the projected axes (matches egui: SIZE/2 - 12). The
/// `AXIS_GIZMO_SIZE` constant sets the *base* size at slider value 5; the
/// Display dropdown's "Gizmo Size" slider scales this and every other axis
/// dimension (`CENTRE`, `POS_D`, `NEG_D`, line thickness) uniformly via
/// [`gizmo_size_scale`].
const AXIS_LEN: f32 = AXIS_GIZMO_SIZE / 2.0 - 12.0;
/// Container-local centre.
const CENTRE: f32 = AXIS_GIZMO_SIZE / 2.0;
const POS_D: f32 = 18.0;
const NEG_D: f32 = 12.0;

/// Curve from slider value `s` (0..5) → multiplier on the base sizes above.
/// 5.0 = the base (1.0×); 0.0 = 20% of base, the smallest that still reads
/// as a usable click target. Linear so the slider feels even across its range.
fn gizmo_size_scale(slider: f32) -> f32 {
    let s = slider.clamp(0.0, 5.0);
    // 0.20 at slider=0; 1.0 at slider=5. Linear in between.
    0.20 + (s / 5.0) * 0.80
}

#[derive(Component)]
struct AxisGizmoRoot;

/// Which viewport slot this axis-gizmo cluster belongs to. Every viewport panel
/// builds its own cluster; this tags it so `gizmo_layout` projects it from that
/// slot's *own* camera orbit rather than every cluster mirroring the focused
/// camera. Propagated onto the tips + lines too so their per-frame projection
/// reads the right slot without walking the hierarchy.
#[derive(Component, Clone, Copy)]
struct AxisGizmoSlot(usize);

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

/// Build the gizmo cluster on a viewport content node's top-right. Returns the
/// root. `slot` is the viewport slot this panel shows, so the cluster projects
/// from that slot's own camera orbit.
pub(crate) fn build(commands: &mut Commands, fonts: &EmberFonts, slot: usize) -> Entity {
    let root = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                right: Val::Px(AXIS_GIZMO_MARGIN),
                // Dropped below the in-viewport toolbar strip on the top edge.
                // No toolbar offset: the bar is above the scene now, not over it.
                top: Val::Px(AXIS_GIZMO_MARGIN),
                width: Val::Px(AXIS_GIZMO_SIZE),
                height: Val::Px(AXIS_GIZMO_SIZE),
                ..default()
            },
            RelativeCursorPosition::default(),
            OverlaySurface,
            Interaction::default(),
            AxisGizmoRoot,
            AxisGizmoSlot(slot),
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
                AxisGizmoSlot(slot),
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
                AxisGizmoSlot(slot),
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
    viewports: Option<Res<renzora::core::viewport_types::Viewports>>,
    settings: Option<Res<ViewportSettings>>,
    play_mode: Option<Res<renzora::core::PlayModeState>>,
    mut roots: Query<&mut Node, (With<AxisGizmoRoot>, Without<AxisTip>, Without<AxisLine>)>,
    mut tips: Query<
        (&AxisTip, &AxisGizmoSlot, &mut Node, &mut BackgroundColor, &mut ZIndex),
        Without<AxisLine>,
    >,
    mut lines: Query<
        (
            &AxisLine,
            &AxisGizmoSlot,
            &mut Node,
            &mut UiTransform,
            &mut BackgroundColor,
            &mut ZIndex,
        ),
        Without<AxisTip>,
    >,
) {
    // Hidden during play mode for a clean game view, and in 2D view (the axis
    // orientation gizmo is a 3D-orbit control).
    let playing = play_mode.as_ref().is_some_and(|p| p.is_in_play_mode());
    let gizmo_size = settings.as_ref().map(|s| s.gizmo_size).unwrap_or(5.0);
    let scale = gizmo_size_scale(gizmo_size);
    // Pre-scale the per-frame constants so the per-tip math stays simple.
    let axis_len = AXIS_LEN * scale;
    let centre = CENTRE * scale;
    let pos_d = POS_D * scale;
    let neg_d = NEG_D * scale;
    let line_thickness = 2.5 * scale;
    let root_size = AXIS_GIZMO_SIZE * scale;
    let show = settings
        .map(|s| {
            s.show_axis_gizmo
                && s.viewport_view != renzora::core::viewport_types::ViewportView::Two
        })
        .unwrap_or(true)
        && !playing;
    // Keep the cluster container's size in step with the scaled dimensions so
    // the cluster doesn't overflow its own bounds (and the backplate stays a
    // circle, not a clipped square).
    for mut node in &mut roots {
        let want_disp = if show { Display::Flex } else { Display::None };
        if node.display != want_disp {
            node.display = want_disp;
        }
        if node.width != Val::Px(root_size) {
            node.width = Val::Px(root_size);
        }
        if node.height != Val::Px(root_size) {
            node.height = Val::Px(root_size);
        }
        if node.border_radius != BorderRadius::all(Val::Px(root_size / 2.0)) {
            node.border_radius = BorderRadius::all(Val::Px(root_size / 2.0));
        }
    }
    if !show {
        return;
    }

    // Each cluster reads its OWN slot's stored orbit angle (`Viewports.slots[i]`),
    // which `renzora_camera::mirror_focused_orbit_out` rewrites for the focused
    // slot every frame — so it's current for all slots. We deliberately do NOT
    // special-case the focused slot to read the live `CameraOrbitSnapshot`: on the
    // frame focus moves to a new viewport, the snapshot still holds the *previous*
    // view's angle (it's refreshed later in the schedule), so the newly focused
    // cube would flash the old angle for one frame. Reading slots[i] avoids that
    // race; the only cost is a 1-frame lag mid-orbit, which is invisible.
    let slot_orbit = |slot: usize| -> (f32, f32) {
        if let Some(s) = viewports.as_ref().and_then(|v| v.slots.get(slot)) {
            return (s.yaw, s.pitch);
        }
        orbit.as_deref().map(|o| (o.yaw, o.pitch)).unwrap_or((0.0, 0.0))
    };

    // Project is a free function that still uses the unscaled `AXIS_LEN`; pass
    // the scaled length in via a tiny inline project so we don't duplicate the
    // rotation math.
    let project_at = |dir: Vec3, yaw: f32, pitch: f32| -> (Vec2, f32) {
        let (cy, sy) = (yaw.cos(), yaw.sin());
        let (cp, sp) = (pitch.cos(), pitch.sin());
        let r = Vec3::new(dir.x * cy + dir.z * sy, dir.y, -dir.x * sy + dir.z * cy);
        let v = Vec3::new(r.x, r.y * cp + r.z * sp, -r.y * sp + r.z * cp);
        (Vec2::new(v.x * axis_len, -v.y * axis_len), v.z)
    };

    for (tip, slot, mut node, mut bg, mut z) in &mut tips {
        let (yaw, pitch) = slot_orbit(slot.0);
        let (off, depth) = project_at(tip.dir, yaw, pitch);
        let d = if tip.positive { pos_d } else { neg_d };
        node.left = Val::Px(centre + off.x - d / 2.0);
        node.top = Val::Px(centre + off.y - d / 2.0);
        node.width = Val::Px(d);
        node.height = Val::Px(d);
        node.border_radius = BorderRadius::all(Val::Px(d / 2.0));
        let alpha = if depth < -0.1 { 0.45 } else { 1.0 };
        bg.0 = rgba(tip.color, alpha);
        *z = ZIndex(100 + (depth * 10.0) as i32);
    }

    for (line, slot, mut node, mut tf, mut bg, mut z) in &mut lines {
        let (yaw, pitch) = slot_orbit(slot.0);
        let (off, depth) = project_at(line.dir, yaw, pitch);
        let len = off.length();
        node.width = Val::Px(len);
        node.height = Val::Px(line_thickness);
        // Centre the line on the midpoint, then rotate about its own centre so it
        // spans centre -> tip.
        node.left = Val::Px(centre + off.x / 2.0 - len / 2.0);
        node.top = Val::Px(centre + off.y / 2.0 - line_thickness / 2.0);
        *tf = UiTransform::from_rotation(Rot2::radians(off.y.atan2(off.x)));
        let alpha = if depth < -0.1 { 0.4 } else { 0.9 };
        bg.0 = rgba(line.color, alpha);
        *z = ZIndex(10 + (depth * 10.0) as i32);
    }
}

/// Brighten the backplate while orbiting (or leave it subtle), and keep its
/// size in step with the cluster's `gizmo_size` slider so the backplate stays
/// a circle filling the root rather than a clipped square.
fn gizmo_backplate(
    nav: Option<Res<NavOverlayState>>,
    settings: Option<Res<ViewportSettings>>,
    mut plates: Query<&mut Node, With<AxisBackplate>>,
    mut bgs: Query<&mut BackgroundColor, With<AxisBackplate>>,
) {
    let active = nav.is_some_and(|n| n.orbit_dragging.load(Ordering::Relaxed));
    let a = if active { 0.38 } else { 0.22 };
    let size = AXIS_GIZMO_SIZE
        * gizmo_size_scale(settings.as_ref().map(|s| s.gizmo_size).unwrap_or(5.0));
    for mut bg in &mut bgs {
        bg.0 = Color::srgba(0.0, 0.0, 0.0, a);
    }
    for mut node in &mut plates {
        if node.width != Val::Px(size) {
            node.width = Val::Px(size);
        }
        if node.height != Val::Px(size) {
            node.height = Val::Px(size);
        }
        let r = BorderRadius::all(Val::Px(size / 2.0));
        if node.border_radius != r {
            node.border_radius = r;
        }
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
