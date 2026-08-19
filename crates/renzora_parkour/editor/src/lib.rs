//! Editor diagnostic gizmo for the parkour controller.
//!
//! The controller does not move the collider you authored — it sweeps a capsule
//! of its own, built from `ParkourController`'s `radius` / `height` /
//! `foot_offset`, and that capsule is what decides where the character can go.
//! Nothing in the viewport showed it, so a character whose capsule did not match
//! its model — floating, sunk through the floor, too fat to fit a gap — looked
//! exactly like one that did. This draws it.
//!
//! It also draws what the probe saw: whether the character is grounded, the
//! ledge in front and how the controller classified it, the walls beside it, and
//! the arc of a traversal while one is playing. Those come from the values the
//! controller recorded on `ParkourMotion` last frame rather than from fresh
//! casts, so what you see is what the state machine actually decided from — a
//! second set of rays would answer slightly differently and be worse than
//! useless for diagnosis.
//!
//! Visibility rides the existing **Gizmos → Physics** dropdown
//! ([`CollisionGizmoVisibility`]), so it appears and hides with collider
//! wireframes instead of adding a switch of its own.

use bevy::prelude::*;

use renzora::core::viewport_types::{CollisionGizmoVisibility, ViewportSettings};
use renzora_editor_framework::EditorSelection;
use renzora_gizmo::collider_gizmo::draw_capsule;
use renzora_gizmo::OverlayGizmoGroup;
use renzora_parkour::state::{ParkourMotion, ParkourState};
use renzora_parkour::ParkourController;

/// Standing on walkable ground.
const COLOR_GROUNDED: Color = Color::srgb(0.30, 0.85, 0.40);
/// In the air — the state where a wrong `foot_offset` shows up as a character
/// that never lands.
const COLOR_AIRBORNE: Color = Color::srgb(1.0, 0.75, 0.20);
/// Playing an authored move, where gravity and collision are switched off.
const COLOR_TRAVERSAL: Color = Color::srgb(0.75, 0.45, 1.0);
/// Holding onto something: a ledge, a ladder, a rope.
const COLOR_HELD: Color = Color::srgb(0.30, 0.80, 1.0);
/// Riding a wall.
const COLOR_WALL: Color = Color::srgb(1.0, 0.40, 0.70);

/// A ledge the controller would vault.
const COLOR_VAULT: Color = Color::srgb(0.40, 1.0, 0.50);
/// A ledge it would mantle onto.
const COLOR_MANTLE: Color = Color::srgb(1.0, 0.70, 0.25);
/// A ledge too high to reach from the ground, but grabbable in mid-air.
const COLOR_GRAB: Color = Color::srgb(0.40, 0.85, 1.0);
/// A ledge the controller found but will not act on.
const COLOR_INERT: Color = Color::srgb(0.55, 0.55, 0.60);

/// Draw the capsule and, once the simulation is running, the probes.
///
/// `ParkourMotion` only exists while the controller has run at least once, so
/// in edit mode this draws the capsule alone — which is the half that matters
/// when the complaint is that the character sits wrong.
pub fn draw_parkour_gizmos(
    mut gizmos: Gizmos<OverlayGizmoGroup>,
    selection: Res<EditorSelection>,
    settings: Option<Res<ViewportSettings>>,
    characters: Query<(
        Entity,
        &ParkourController,
        &GlobalTransform,
        Option<&ParkourMotion>,
    )>,
) {
    let visibility = settings
        .map(|s| s.collision_gizmo_visibility)
        .unwrap_or_default();
    if visibility == CollisionGizmoVisibility::Off {
        return;
    }
    let selected_only = visibility == CollisionGizmoVisibility::SelectedOnly;

    for (entity, controller, gt, motion) in &characters {
        if selected_only && !selection.is_selected(entity) {
            continue;
        }

        let state = motion.map(|m| m.state).unwrap_or_default();
        let color = match state {
            ParkourState::Grounded => COLOR_GROUNDED,
            ParkourState::Airborne => COLOR_AIRBORNE,
            ParkourState::Vaulting | ParkourState::Mantling => COLOR_TRAVERSAL,
            ParkourState::Hanging | ParkourState::ClimbingLadder | ParkourState::Swinging => {
                COLOR_HELD
            }
            ParkourState::WallRunning => COLOR_WALL,
        };

        // Deliberately upright and unscaled, matching the sweep: the controller
        // casts an axis-aligned capsule with an identity rotation, so drawing
        // the entity's own rotation here would show a shape it never uses — and
        // would hide exactly the bug where something else is tipping the
        // character over.
        let origin = gt.translation();
        let foot = origin - Vec3::Y * controller.foot_offset;
        let radius = controller.radius;
        let half_height = (controller.height * 0.5 - radius).max(0.01);
        let center = foot + Vec3::Y * (controller.height * 0.5);
        draw_capsule(&mut gizmos, center, Quat::IDENTITY, radius, half_height, color);

        // Where the controller believes the soles are. `foot_offset` is the
        // single most commonly wrong field, and it is invisible without this:
        // set it too small and the cross floats above the floor while the
        // character sinks into it.
        let tick = radius * 0.6;
        gizmos.line(foot - Vec3::X * tick, foot + Vec3::X * tick, color);
        gizmos.line(foot - Vec3::Z * tick, foot + Vec3::Z * tick, color);

        // A plumb line from the entity origin down to the soles, so an origin
        // that is not where `foot_offset` claims is obvious at a glance.
        if controller.foot_offset.abs() > 0.001 {
            gizmos.line(origin, foot, color.with_alpha(0.4));
        }

        let Some(motion) = motion else {
            continue;
        };
        let probe = &motion.last_probe;

        // Ground contact and its normal. A normal leaning away from vertical on
        // flat floor means the probe is catching something it should not.
        if probe.grounded {
            gizmos.line(foot, foot + probe.ground_normal * 0.5, COLOR_GROUNDED);
            gizmos.circle(
                Isometry3d::new(
                    foot + Vec3::Y * 0.01,
                    Quat::from_rotation_x(std::f32::consts::FRAC_PI_2),
                ),
                radius,
                COLOR_GROUNDED,
            );
        }

        // The direction everything ahead is probed along, at the height the
        // low obstacle ray is actually cast from.
        let knee = foot + Vec3::Y * (controller.step_height + 0.05);
        let reach = controller.radius + controller.forward_reach;
        gizmos.line(
            knee,
            knee + motion.last_forward * reach,
            Color::srgb(0.8, 0.8, 0.85),
        );

        // The ledge, coloured by what the controller would do with it. This is
        // the answer to "why did it vault instead of climbing".
        if let Some(ledge) = probe.ledge {
            let ledge_color = if ledge.thin && ledge.height <= controller.vault_max_height {
                COLOR_VAULT
            } else if ledge.clear && ledge.height <= controller.mantle_max_height {
                COLOR_MANTLE
            } else if ledge.height >= controller.height * 0.55
                && ledge.height <= controller.height * 1.15
            {
                COLOR_GRAB
            } else {
                COLOR_INERT
            };
            let lip = ledge.top;
            gizmos.line(lip - Vec3::X * 0.15, lip + Vec3::X * 0.15, ledge_color);
            gizmos.line(lip - Vec3::Z * 0.15, lip + Vec3::Z * 0.15, ledge_color);
            // Its height above the feet, which is the number the whole
            // vault/mantle/grab decision turns on.
            gizmos.line(Vec3::new(lip.x, foot.y, lip.z), lip, ledge_color);
            // Outward face normal, short.
            gizmos.line(lip, lip + ledge.face_normal * 0.25, ledge_color);
            if ledge.thin {
                // Where a vault would put the character down.
                gizmos.line(
                    ledge.landing - Vec3::X * 0.12,
                    ledge.landing + Vec3::X * 0.12,
                    COLOR_VAULT,
                );
                gizmos.line(
                    ledge.landing - Vec3::Z * 0.12,
                    ledge.landing + Vec3::Z * 0.12,
                    COLOR_VAULT,
                );
            }
        }

        // Walls in reach, drawn from chest height where they are sensed.
        let chest = foot + Vec3::Y * (controller.height * 0.6);
        for wall in [probe.wall_left, probe.wall_right, probe.wall_front]
            .into_iter()
            .flatten()
        {
            gizmos.line(chest, chest - wall.normal * wall.distance, COLOR_WALL);
        }

        // The arc a traversal is following, sampled along its curve. Seeing the
        // path is the only way to tell a mantle that is aiming short from one
        // that is aiming through the wall.
        if let Some(t) = motion.traversal {
            const STEPS: usize = 16;
            let mut previous = t.start;
            for i in 1..=STEPS {
                let f = i as f32 / STEPS as f32;
                let inv = 1.0 - f;
                let point = t.start * (inv * inv) + t.apex * (2.0 * inv * f) + t.end * (f * f);
                gizmos.line(previous, point, COLOR_TRAVERSAL);
                previous = point;
            }
            gizmos.line(t.end - Vec3::X * 0.15, t.end + Vec3::X * 0.15, COLOR_TRAVERSAL);
            gizmos.line(t.end - Vec3::Z * 0.15, t.end + Vec3::Z * 0.15, COLOR_TRAVERSAL);
        }
    }
}

#[derive(Default)]
pub struct ParkourEditorPlugin;

impl Plugin for ParkourEditorPlugin {
    fn build(&self, app: &mut App) {
        info!("[editor] ParkourEditorPlugin (traversal diagnostic gizmos)");
        app.add_systems(Update, draw_parkour_gizmos);
    }
}

renzora::add!(ParkourEditorPlugin, Editor);
