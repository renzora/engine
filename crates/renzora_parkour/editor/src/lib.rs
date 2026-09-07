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
use renzora_parkour::{ParkourController, ParkourLadder, ParkourSwingAnchor};

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
/// A rope anchor close enough to catch.
const COLOR_ROPE: Color = Color::srgb(1.0, 0.85, 0.35);

/// How solid a highlighted face is drawn. Gizmos have no fill, so a "face" is a
/// hatched outline; too dense and it hides the geometry it is marking.
const HATCH_LINES: usize = 5;

/// Outline a rectangle in space and hatch it, so a *surface* reads as a surface
/// rather than as four lines that happen to meet.
///
/// `right` and `up` are half-extents along the face, so the patch is
/// `2·right × 2·up` centred on `centre`. Both are already oriented to the face
/// by the caller — the point of taking them rather than a normal is that the
/// caller knows which way is "along the wall" and this cannot.
fn draw_face(gizmos: &mut Gizmos<OverlayGizmoGroup>, centre: Vec3, right: Vec3, up: Vec3, color: Color) {
    let corners = [
        centre - right - up,
        centre + right - up,
        centre + right + up,
        centre - right + up,
    ];
    for i in 0..4 {
        gizmos.line(corners[i], corners[(i + 1) % 4], color);
    }
    // Hatching, at a lower alpha so the outline stays the dominant edge.
    let faint = color.with_alpha(0.35);
    for i in 1..HATCH_LINES {
        let f = i as f32 / HATCH_LINES as f32;
        let y = up * (f * 2.0 - 1.0);
        gizmos.line(centre - right + y, centre + right + y, faint);
    }
}

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
    // Swing anchors are found by proximity rather than by the forward probe,
    // so they never appear in `ParkourProbe` and have to be looked up here.
    anchors: Query<(&GlobalTransform, &ParkourSwingAnchor)>,
    // The ladder the probe found, resolved to something drawable.
    ladders: Query<&GlobalTransform, With<ParkourLadder>>,
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

            // The surface the action would actually use, highlighted.
            //
            // A cross at the lip says *where* the controller found something;
            // it does not say what you would be standing on or pulling up
            // onto, which is the question when a mantle aims at the wrong
            // shelf. `face_normal` points back at the character, so its
            // horizontal part is the outward direction of the face and the
            // cross product with up runs along it.
            let outward = Vec3::new(ledge.face_normal.x, 0.0, ledge.face_normal.z);
            if outward.length_squared() > 1e-4 {
                let outward = outward.normalize();
                let along = outward.cross(Vec3::Y).normalize() * (controller.radius * 2.0);
                if ledge.thin {
                    // A rail: mark the top edge you clear, a narrow strip.
                    draw_face(
                        &mut gizmos,
                        lip + Vec3::Y * 0.01,
                        along,
                        -outward * 0.12,
                        ledge_color,
                    );
                } else {
                    // A platform: mark the top face you land on, running back
                    // away from the character.
                    draw_face(
                        &mut gizmos,
                        lip + Vec3::Y * 0.01 - outward * (controller.radius * 1.2),
                        along,
                        -outward * (controller.radius * 1.2),
                        ledge_color,
                    );
                }
            }

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

        // Walls in reach, drawn from chest height where they are sensed, plus
        // the patch of wall a run would actually use.
        //
        // The line alone answers "is a wall sensed"; it does not answer "which
        // surface", which is what you need when a run refuses to start beside
        // something that looks like a perfectly good wall. The patch is drawn
        // at the sensed distance along the normal, spanning the capsule's
        // height and about a stride of its length.
        let chest = foot + Vec3::Y * (controller.height * 0.6);
        for (wall, runnable) in [
            (probe.wall_left, controller.wall_run),
            (probe.wall_right, controller.wall_run),
            // A wall ahead is never run along — it is what a wall *jump* comes
            // off — so it is marked as contact rather than as an opportunity.
            (probe.wall_front, false),
        ] {
            let Some(wall) = wall else { continue };
            gizmos.line(chest, chest - wall.normal * wall.distance, COLOR_WALL);

            let contact = chest - wall.normal * wall.distance;
            let along = wall.normal.cross(Vec3::Y);
            if along.length_squared() < 1e-4 {
                continue;
            }
            let color = if runnable { COLOR_WALL } else { COLOR_INERT };
            draw_face(
                &mut gizmos,
                // Lifted a hair off the surface, or the coplanar lines fight
                // the wall's own faces in the depth buffer.
                contact + wall.normal * 0.02,
                along.normalize() * (controller.height * 0.5),
                Vec3::Y * (controller.height * 0.4),
                color,
            );
        }

        // The ladder in front, marked along its full climbable height. The
        // probe only reports *that* one was found; without this there is no
        // way to see which object it resolved to, and a `ParkourLadder` on the
        // wrong ancestor looks identical to none at all.
        if let Some(entity) = probe.ladder {
            if let Ok(ladder) = ladders.get(entity) {
                let base = ladder.translation();
                let facing = (origin - base).with_y(0.0);
                if facing.length_squared() > 1e-4 {
                    let outward = facing.normalize();
                    draw_face(
                        &mut gizmos,
                        base + outward * 0.05,
                        outward.cross(Vec3::Y).normalize() * 0.4,
                        Vec3::Y * (controller.height * 0.75),
                        COLOR_HELD,
                    );
                }
            }
        }

        // Rope anchors within grabbing range, with the arc the character would
        // hang on. Drawn from the anchor's own `max_grab_distance` so a rope
        // that will not catch looks different from one that will.
        for (anchor_gt, anchor) in &anchors {
            let point = anchor_gt.translation();
            let reach = origin.distance(point);
            if reach > anchor.max_grab_distance {
                continue;
            }
            let color = if state == ParkourState::Swinging {
                COLOR_HELD
            } else {
                COLOR_ROPE
            };
            // The anchor itself.
            let tick = 0.2;
            gizmos.line(point - Vec3::X * tick, point + Vec3::X * tick, color);
            gizmos.line(point - Vec3::Y * tick, point + Vec3::Y * tick, color);
            gizmos.line(point - Vec3::Z * tick, point + Vec3::Z * tick, color);
            // The rope, and the circle it would swing through.
            let rope = if anchor.rope_length > 0.0 {
                anchor.rope_length
            } else {
                reach
            };
            gizmos.line(point, origin, color.with_alpha(0.5));
            gizmos.circle(
                Isometry3d::new(point, Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
                rope,
                color.with_alpha(0.35),
            );
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
