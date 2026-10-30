//! The orbit pivot marker: a small ring at the editor camera's orbit pivot,
//! shown while you orbit, pan or zoom, and for a moment after F, End, A or
//! Home moves it.
//!
//! Orbit, pan and zoom all scale with how far away the pivot is, and that
//! distance used to be invisible: a pivot left a kilometre off (F on a large
//! terrain, a scripted view) made every gesture lurch with nothing on screen to
//! say why. The ring shows where the pivot is, and the viewport labels it with
//! the distance (`renzora_viewport::pivot_label`, which reads
//! [`PivotMarkerState`]).
//!
//! **Transient on purpose.** The pivot sits in the middle of whatever you are
//! working on, so a marker that stayed up would always be in the way. Flying
//! and mouse-look (right mouse) carry the pivot along with the camera, so they
//! do not show it either: it would only trail the camera around.
//!
//! **On top, but dimmed when hidden.** It draws over the scene so a pivot
//! inside a mesh still shows, and dims when a surface stands between it and
//! the camera, which is how a pivot left underground reads as one.

use bevy::prelude::*;
use renzora::core::viewport_types::{PivotMarkerState, ViewportSettings, ViewportState, ViewportView};
use renzora::core::EditorCamera;

use super::{CameraDragState, CameraVelocityState, OrbitCameraState, PivotLock, SurfaceAhead};

/// Seconds the marker stays at full strength after the pivot last moved.
const HOLD: f32 = 0.8;
/// Seconds it then takes to fade out.
const FADE: f32 = 0.4;
/// Ring radius on screen, in logical pixels.
const RING_PX: f32 = 7.0;
/// The centre dot, in logical pixels.
const DOT_PX: f32 = 2.0;
/// How strong the marker stays while a surface hides the pivot.
const OCCLUDED: f32 = 0.35;

/// The marker's lines: on the overlay layer and drawn over the scene, like
/// the editor's other overlays.
#[derive(Default, Reflect, GizmoConfigGroup)]
#[reflect(Default)]
pub struct PivotGizmoGroup;

/// When the pivot last moved, and where the orbit was then.
#[derive(Resource, Default)]
pub(super) struct PivotActivity {
    pose: Option<(Vec3, f32, f32, f32)>,
    moved_at: Option<f32>,
}

pub(super) fn install(app: &mut App) {
    app.init_resource::<PivotActivity>()
        .init_resource::<PivotMarkerState>()
        .insert_gizmo_config(
            PivotGizmoGroup,
            GizmoConfig {
                depth_bias: -1.0,
                line: GizmoLineConfig { width: 2.0, ..default() },
                render_layers: bevy::camera::visibility::RenderLayers::layer(1),
                ..default()
            },
        );
}

/// Note when the pivot moves under orbit, pan, zoom or a jump (F, End, A,
/// Home, the View menu), and keep the marker up through a held orbit or pan.
pub(super) fn track_pivot(
    time: Res<Time>,
    orbit: Res<OrbitCameraState>,
    drag: Res<CameraDragState>,
    velocity: Res<CameraVelocityState>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut activity: ResMut<PivotActivity>,
) {
    let pose = (orbit.focus, orbit.distance, orbit.yaw, orbit.pitch);
    let moved = activity.pose.is_some_and(|last| last != pose);
    activity.pose = Some(pose);
    let flying = mouse.pressed(MouseButton::Right) || velocity.velocity != Vec3::ZERO;
    if !flying && (moved || drag.dragging) {
        activity.moved_at = Some(time.elapsed_secs());
    }
}

/// How strong the marker is `since` seconds after the pivot last moved.
fn strength(since: f32) -> f32 {
    if since <= HOLD {
        1.0
    } else {
        (1.0 - (since - HOLD) / FADE).clamp(0.0, 1.0)
    }
}

/// Draw the ring and publish where it is for the viewport's distance label.
#[allow(clippy::too_many_arguments)]
pub(super) fn draw_pivot(
    time: Res<Time>,
    orbit: Res<OrbitCameraState>,
    lock: Res<PivotLock>,
    activity: Res<PivotActivity>,
    settings: Option<Res<ViewportSettings>>,
    viewport: Option<Res<ViewportState>>,
    play_mode: Option<Res<renzora::core::PlayModeState>>,
    camera: Query<(&GlobalTransform, &Projection), With<EditorCamera>>,
    mut marker: ResMut<PivotMarkerState>,
    mut surface: SurfaceAhead,
    mut gizmos: Gizmos<PivotGizmoGroup>,
) {
    let wanted = settings.as_ref().is_none_or(|s| s.show_pivot && s.viewport_view == ViewportView::Three)
        && !play_mode.as_ref().is_some_and(|p| p.is_in_play_mode());
    let strength = match activity.moved_at {
        Some(at) if wanted => strength(time.elapsed_secs() - at),
        _ => 0.0,
    };
    let Some(((cam, projection), screen_h)) = camera
        .single()
        .ok()
        .zip(viewport.as_ref().map(|v| v.screen_size.y).filter(|h| *h > 0.0))
        .filter(|_| strength > 0.0)
    else {
        if marker.opacity != 0.0 {
            marker.opacity = 0.0;
        }
        return;
    };

    let eye = cam.translation();
    let to_pivot = orbit.focus - eye;
    let distance = to_pivot.length();
    // A surface nearer than the pivot along the line to it hides it.
    let hidden = Dir3::new(to_pivot)
        .ok()
        .and_then(|dir| surface.distance(Ray3d::new(eye, dir)))
        .is_some_and(|hit| hit < distance * 0.995);
    let opacity = strength * if hidden { OCCLUDED } else { 1.0 };

    // The world size of one logical pixel at the pivot, so the ring keeps one
    // size on screen however far away the pivot is.
    let per_px = match projection {
        Projection::Perspective(p) => 2.0 * to_pivot.dot(*cam.forward()).max(0.01) * (p.fov * 0.5).tan() / screen_h,
        Projection::Orthographic(o) => o.area.height() / screen_h,
        _ => return,
    };

    let tint = if lock.0 { Color::srgb(1.0, 0.72, 0.2) } else { Color::WHITE };
    let color = tint.with_alpha(opacity);
    let shadow = Color::BLACK.with_alpha(opacity * 0.5);
    let facing = Isometry3d::new(orbit.focus, cam.rotation());
    // A dark halo under the ring keeps it readable on snow and sky.
    gizmos.circle(facing, (RING_PX + 1.5) * per_px, shadow).resolution(32);
    gizmos.circle(facing, RING_PX * per_px, color).resolution(32);
    // Two tight rings read as a filled dot at this size.
    gizmos.circle(facing, DOT_PX * per_px, color).resolution(12);
    gizmos.circle(facing, DOT_PX * 0.5 * per_px, color).resolution(8);

    *marker = PivotMarkerState { position: orbit.focus, distance, locked: lock.0, opacity };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_marker_holds_then_fades_out() {
        assert_eq!(strength(0.0), 1.0);
        assert_eq!(strength(HOLD), 1.0);
        let midway = strength(HOLD + FADE * 0.5);
        assert!(midway > 0.4 && midway < 0.6, "{midway}");
        assert_eq!(strength(HOLD + FADE), 0.0);
        assert_eq!(strength(HOLD + FADE + 10.0), 0.0);
    }
}
