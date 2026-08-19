//! The state machine: one system, one pass per character, per frame.
//!
//! Order inside the pass is fixed and matters:
//!
//! 1. **Probe** the surroundings from where the character *is*.
//! 2. **Decide** — run the state machine, which may switch state, start a
//!    traversal, or just set a velocity.
//! 3. **Move** — either warp (states that own the exact position: traversals,
//!    hangs, ladders, swings) or collide-and-slide (states that are subject to
//!    gravity and geometry: ground, air, wall runs).
//!
//! Steps 2 and 3 are separate because most of the interesting states do not
//! move by integrating a velocity at all. A mantle follows a curve, a hang is
//! pinned to a lip, a swing is pinned to a sphere around its anchor. Trying to
//! express those as forces produces a controller that is always *nearly* right
//! and never exactly right, which in a traversal system reads as the character
//! clipping the ledge they were supposed to catch.
//!
//! Everything runs on `Transform`, not on avian's `Position`, matching what
//! `renzora_physics`'s own `kinematic_slide` does — the character is kinematic
//! and the solver is not asked to move it.

use avian3d::prelude::*;
use bevy::prelude::*;
use renzora_physics::backend::avian_character::shape_cast_slide;

use crate::probe::{probe, ProbeWorld};
use crate::read_state::ParkourReadState;
use crate::state::{
    HangGrip, ParkourInput, ParkourMotion, ParkourState, SwingLink, Traversal, TraversalExit,
};
use crate::{
    ParkourController, ParkourEvent, ParkourEventKind, ParkourLadder, ParkourSweep,
    ParkourSwingAnchor,
};

/// How far above the ground the capsule is parked. Small enough that no
/// grounded test can miss it, large enough that a horizontal sweep never
/// starts flush against the floor.
const GROUND_SKIN: f32 = 0.01;

/// A swing anchor that passed the reach and line-of-sight tests.
struct AnchorCandidate {
    entity: Entity,
    point: Vec3,
    /// The anchor's authored rope length; `0` means "however far away it is".
    rope: f32,
}

/// Advance every parkour character by one frame.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn drive_parkour(
    time: Res<Time>,
    spatial: SpatialQuery,
    world: ProbeWorld,
    anchors: Query<(Entity, &GlobalTransform, &ParkourSwingAnchor)>,
    ladder_cfg: Query<&ParkourLadder>,
    mut characters: Query<(
        Entity,
        &mut Transform,
        &ParkourController,
        &mut ParkourMotion,
        &mut ParkourInput,
        &mut ParkourReadState,
        &ParkourSweep,
    )>,
    mut commands: Commands,
    // Scratch for the per-character exclusion list, reused rather than
    // reallocated every frame.
    mut excluded: Local<Vec<Entity>>,
) {
    // A long frame (asset load, editor hitch) must not be integrated in one
    // step: at 200 ms a running character would move a metre and a half
    // between sweeps, straight through a wall.
    let dt = time.delta_secs().min(0.05);
    if dt <= 0.0 {
        return;
    }

    for (entity, mut transform, controller, mut motion, mut input, mut read, sweep) in
        &mut characters
    {
        // The character must not sweep against its own body — including any
        // collider hanging off a child, which is where an imported model
        // normally keeps it.
        world.subtree_into(entity, &mut excluded);
        let filter = SpatialQueryFilter::from_excluded_entities(excluded.iter().copied());
        let max_slope = controller.max_slope;
        let half = controller.height * 0.5;
        let foot = transform.translation - Vec3::Y * controller.foot_offset;

        // ── Input ────────────────────────────────────────────────────────
        // One-shots become short-lived buffers; the raw flags are cleared so a
        // press that lands after this system ran survives to the next frame
        // rather than firing twice.
        if input.jump_pressed {
            motion.jump_buffer = controller.jump_buffer_time;
            input.jump_pressed = false;
        }
        if input.action_pressed {
            motion.action_buffer = controller.jump_buffer_time;
            input.action_pressed = false;
        }
        let released = std::mem::take(&mut input.release_pressed);
        // `move_dir` is consumed, not latched: the script re-states its intent
        // every frame (same contract as `move_controller`), so letting go of
        // the stick stops the character instead of running forever.
        let move_dir = std::mem::take(&mut input.move_dir);
        let sprinting = input.sprint;

        let move_h = Vec3::new(move_dir.x, 0.0, move_dir.z);
        let move_len = move_h.length().min(1.0);
        let wish = move_h.normalize_or_zero();
        // Derived from the controller's own facing rather than read off the
        // transform: with a `facing_offset` the transform is deliberately
        // rotated away from the direction the character is logically facing,
        // and probing along the model's nose would then look backwards.
        let facing_dir = Vec3::new(-motion.facing.sin(), 0.0, -motion.facing.cos());
        let forward = if move_len > 0.05 { wish } else { facing_dir };

        motion.state_time += dt;
        motion.jump_buffer = (motion.jump_buffer - dt).max(0.0);
        motion.action_buffer = (motion.action_buffer - dt).max(0.0);
        motion.coyote = (motion.coyote - dt).max(0.0);
        motion.wall_cooldown = (motion.wall_cooldown - dt).max(0.0);
        motion.grab_cooldown = (motion.grab_cooldown - dt).max(0.0);

        let p = probe(&spatial, &world, controller, &sweep.0, foot, forward, &filter);
        motion.last_probe = p.clone();
        motion.last_forward = forward;
        let jump_speed = (2.0 * controller.gravity.abs() * controller.jump_height).sqrt();
        let target_speed = if sprinting {
            controller.run_speed
        } else {
            controller.walk_speed
        } * move_len;

        // Set by the states that place the character exactly; when it stays
        // `None` the move goes through collide-and-slide instead.
        let mut warp: Option<Vec3> = None;
        // Direction to turn toward this frame, if the state has an opinion.
        let mut face: Option<Vec3> = None;
        // Wall normal + measured distance a wall run wants held after the move.
        let mut wall_hold: Option<(Vec3, f32)> = None;
        let mut events: Vec<ParkourEventKind> = Vec::new();

        match motion.state {
            // ── On the ground ────────────────────────────────────────────
            ParkourState::Grounded => {
                // Touching down re-arms the wall run.
                motion.wall_used = false;
                let want = wish * target_speed;
                let flat = Vec3::new(motion.velocity.x, 0.0, motion.velocity.z);
                let moved = approach(flat, want, controller.acceleration * dt);
                motion.velocity.x = moved.x;
                motion.velocity.z = moved.z;
                // No vertical motion at all while grounded — the ground snap
                // after the slide is what keeps the soles on the surface.
                //
                // Pressing down with a constant bias instead (the obvious
                // alternative) breaks in two ways at once: the slide preserves
                // the *magnitude* of a clipped move, so the downward part is
                // converted into forward speed and the character walks ~12%
                // too fast; and the capsule sinks a skin-width per frame until
                // it is embedded far enough that the contact normal comes back
                // sideways, at which point the slide projects out all forward
                // motion and the character stops dead in open ground.
                motion.velocity.y = 0.0;
                if move_len > 0.05 {
                    face = Some(wish);
                }

                let want_traverse = controller.auto_traverse || motion.action_buffer > 0.0;
                if let Some(ledge) = p.ledge.filter(|_| want_traverse && move_len > 0.1) {
                    if ledge.thin && ledge.height <= controller.vault_max_height {
                        start_vault(&mut motion, controller, &transform, &ledge, forward);
                        motion.action_buffer = 0.0;
                        events.push(ParkourEventKind::VaultStart);
                    } else if ledge.height <= controller.mantle_max_height && ledge.clear {
                        start_mantle(
                            &mut motion,
                            controller,
                            &transform,
                            &ledge,
                            TraversalExit::Free,
                        );
                        motion.action_buffer = 0.0;
                        events.push(ParkourEventKind::MantleStart);
                    }
                }

                if motion.state == ParkourState::Grounded {
                    if let Some(ladder) = p.ladder {
                        let cfg = ladder_cfg.get(ladder).copied().unwrap_or_default();
                        let wants =
                            motion.action_buffer > 0.0 || (cfg.auto_attach && move_len > 0.1);
                        if wants {
                            mount_ladder(&mut motion, ladder, &p, foot);
                            motion.action_buffer = 0.0;
                            events.push(ParkourEventKind::LadderMount);
                        }
                    }
                }

                if motion.state == ParkourState::Grounded && motion.jump_buffer > 0.0 {
                    motion.velocity.y = jump_speed;
                    motion.jump_buffer = 0.0;
                    motion.coyote = 0.0;
                    motion.enter(ParkourState::Airborne);
                    events.push(ParkourEventKind::Jump);
                } else if motion.state == ParkourState::Grounded && !p.grounded {
                    motion.coyote = controller.coyote_time;
                    motion.enter(ParkourState::Airborne);
                }
            }

            // ── In the air ───────────────────────────────────────────────
            ParkourState::Airborne => {
                motion.velocity.y =
                    (motion.velocity.y + controller.gravity * dt).max(controller.terminal_velocity);
                let want = wish * target_speed;
                let flat = Vec3::new(motion.velocity.x, 0.0, motion.velocity.z);
                let moved = approach(
                    flat,
                    want,
                    controller.acceleration * controller.air_control * dt,
                );
                motion.velocity.x = moved.x;
                motion.velocity.z = moved.z;
                if move_len > 0.05 {
                    face = Some(wish);
                }

                // A wall jump beats every other use of the button: it is the
                // one the player pressed *at* a wall, and falling back to a
                // coyote jump there would silently do nothing.
                let wall = p
                    .wall_front
                    .or(p.wall_left)
                    .or(p.wall_right)
                    .map(|w| w.normal)
                    .filter(|_| controller.wall_run && motion.wall_cooldown <= 0.0);
                if let (Some(n), true) = (wall, motion.jump_buffer > 0.0 && !p.grounded) {
                    motion.velocity =
                        n * controller.wall_jump_away + Vec3::Y * controller.wall_jump_up;
                    motion.jump_buffer = 0.0;
                    motion.wall_cooldown = 0.25;
                    motion.state_time = 0.0;
                    face = Some(-n);
                    events.push(ParkourEventKind::WallJump);
                } else if motion.jump_buffer > 0.0 && motion.coyote > 0.0 {
                    motion.velocity.y = jump_speed;
                    motion.jump_buffer = 0.0;
                    motion.coyote = 0.0;
                    events.push(ParkourEventKind::Jump);
                }

                // Grabbing a swing anchor, then a ledge, then a wall run:
                // most specific opportunity first, so a bar hanging in front
                // of a wall is grabbed rather than run past.
                if controller.swing && motion.action_buffer > 0.0 && motion.grab_cooldown <= 0.0 {
                    let head = foot + Vec3::Y * (controller.height * 0.9);
                    if let Some(a) = nearest_anchor(&anchors, &spatial, &filter, head, forward) {
                        let rope = if a.rope > 0.0 {
                            a.rope
                        } else {
                            (head - a.point).length().max(0.5)
                        };
                        motion.swing = Some(SwingLink {
                            anchor: a.entity,
                            length: rope,
                        });
                        motion.action_buffer = 0.0;
                        motion.enter(ParkourState::Swinging);
                        events.push(ParkourEventKind::SwingGrab);
                    }
                }

                if motion.state == ParkourState::Airborne {
                    if let Some(ledge) = p.ledge {
                        let hand_low = controller.height * 0.55;
                        let hand_high = controller.height * 1.15;
                        let in_reach = ledge.height >= hand_low && ledge.height <= hand_high;
                        let grabbing = controller.ledge_grab
                            && motion.grab_cooldown <= 0.0
                            && motion.velocity.y < 1.0
                            && (move_len > 0.1 || motion.action_buffer > 0.0);
                        if in_reach && grabbing {
                            motion.hang = Some(HangGrip {
                                point: ledge.top,
                                wall_normal: ledge.face_normal,
                            });
                            motion.velocity = Vec3::ZERO;
                            motion.action_buffer = 0.0;
                            motion.enter(ParkourState::Hanging);
                            events.push(ParkourEventKind::Grab);
                        } else if motion.action_buffer > 0.0
                            && ledge.height <= controller.mantle_max_height
                            && ledge.clear
                        {
                            start_mantle(
                                &mut motion,
                                controller,
                                &transform,
                                &ledge,
                                TraversalExit::Free,
                            );
                            motion.action_buffer = 0.0;
                            events.push(ParkourEventKind::MantleStart);
                        }
                    }
                }

                if motion.state == ParkourState::Airborne {
                    if let Some(ladder) = p.ladder {
                        let cfg = ladder_cfg.get(ladder).copied().unwrap_or_default();
                        if motion.action_buffer > 0.0 || cfg.auto_attach {
                            mount_ladder(&mut motion, ladder, &p, foot);
                            motion.action_buffer = 0.0;
                            events.push(ParkourEventKind::LadderMount);
                        }
                    }
                }

                if motion.state == ParkourState::Airborne {
                    let side = p.wall_left.or(p.wall_right);
                    let fast_enough = Vec3::new(motion.velocity.x, 0.0, motion.velocity.z).length()
                        > controller.walk_speed * 0.8;
                    if controller.wall_run
                        && motion.wall_cooldown <= 0.0
                        && !p.grounded
                        && fast_enough
                        && move_len > 0.1
                    {
                        if let Some(w) = side {
                            // One run per wall per ground touch — but a wall
                            // facing a different way is a fresh opportunity, so
                            // a corridor can still be zig-zagged up.
                            let same_wall =
                                motion.wall_used && w.normal.dot(motion.wall_normal) > 0.7;
                            if !same_wall {
                                motion.wall_normal = w.normal;
                                motion.wall_used = true;
                                // Shed most of the jump's climb. Carrying it in
                                // makes the run gain ~8 m over its lifetime,
                                // which both reads wrong and takes the
                                // character clean off the top of any ordinary
                                // wall a second in.
                                motion.velocity.y = motion.velocity.y.min(1.5);
                                motion.enter(ParkourState::WallRunning);
                                events.push(ParkourEventKind::WallRunStart);
                            }
                        }
                    }
                }

                if motion.state == ParkourState::Airborne && p.grounded && motion.velocity.y <= 0.0 {
                    motion.velocity.y = 0.0;
                    motion.enter(ParkourState::Grounded);
                    events.push(ParkourEventKind::Land);
                }
            }

            // ── Vault / mantle ───────────────────────────────────────────
            ParkourState::Vaulting | ParkourState::Mantling => {
                if let Some(t) = motion.traversal.as_mut() {
                    t.elapsed += dt;
                    warp = Some(t.sample());
                    if t.finished() {
                        let (exit, exit_velocity) = (t.exit, t.exit_velocity);
                        let ending = motion.state;
                        motion.traversal = None;
                        motion.velocity = exit_velocity;
                        events.push(if ending == ParkourState::Vaulting {
                            ParkourEventKind::VaultEnd
                        } else {
                            ParkourEventKind::MantleEnd
                        });
                        match exit {
                            TraversalExit::Free => motion.enter(ParkourState::Airborne),
                            TraversalExit::Hang => motion.enter(ParkourState::Hanging),
                        }
                    }
                } else {
                    // Nothing to play (the traversal was cleared out from
                    // under us, e.g. by a script resetting the component).
                    motion.enter(ParkourState::Airborne);
                }
            }

            // ── Hanging from a lip ───────────────────────────────────────
            ParkourState::Hanging => {
                let Some(mut grip) = motion.hang else {
                    motion.enter(ParkourState::Airborne);
                    continue;
                };
                motion.velocity = Vec3::ZERO;
                face = Some(-grip.wall_normal);

                // Shimmy, but only onto lip that is actually there: the two
                // probes below are what stop the character sliding off the end
                // of a balcony into thin air, still in the hang pose.
                let tangent = grip.wall_normal.cross(Vec3::Y).normalize_or_zero();
                let lateral = move_dir.dot(tangent);
                if lateral.abs() > 0.2 && tangent != Vec3::ZERO {
                    let step = tangent * lateral.signum() * controller.hang_shimmy_speed * dt;
                    let probe_at = grip.point + step;
                    let lip_ok = spatial
                        .cast_ray(probe_at + Vec3::Y * 0.4, Dir3::NEG_Y, 0.8, true, &filter)
                        .is_some_and(|h| (probe_at.y + 0.4 - h.distance - grip.point.y).abs() < 0.25);
                    let wall_ok = Dir3::new(-grip.wall_normal).is_ok_and(|d| {
                        spatial
                            .cast_ray(probe_at + grip.wall_normal * 0.5 - Vec3::Y * 0.3, d, 0.9, true, &filter)
                            .is_some()
                    });
                    if lip_ok && wall_ok {
                        grip.point = probe_at;
                        motion.hang = Some(grip);
                        // A hang moves by warping, so this velocity drives
                        // nothing — it exists so the read-state mirror and the
                        // animation driver can tell a shimmy from a still hang.
                        motion.velocity = step / dt;
                    }
                }

                // Pinned pose: hands on the lip, body hanging below it and set
                // back from the wall by roughly the capsule radius.
                let hang_foot = grip.point - Vec3::Y * (controller.height * 0.95)
                    + grip.wall_normal * (controller.radius * 0.9);
                warp = Some(hang_foot + Vec3::Y * controller.foot_offset);

                let climb = motion.action_buffer > 0.0 || motion.jump_buffer > 0.0 || move_dir.y > 0.4;
                let drop = released || move_dir.y < -0.4;
                if climb {
                    let ledge = crate::probe::Ledge {
                        top: grip.point,
                        height: controller.height * 0.95,
                        face_normal: grip.wall_normal,
                        clear: true,
                        thin: false,
                        landing: grip.point,
                    };
                    start_mantle(
                        &mut motion,
                        controller,
                        &transform,
                        &ledge,
                        TraversalExit::Free,
                    );
                    motion.action_buffer = 0.0;
                    motion.jump_buffer = 0.0;
                    motion.hang = None;
                    events.push(ParkourEventKind::MantleStart);
                } else if drop {
                    motion.hang = None;
                    motion.grab_cooldown = 0.35;
                    motion.enter(ParkourState::Airborne);
                    events.push(ParkourEventKind::Release);
                }
            }

            // ── Ladder ───────────────────────────────────────────────────
            ParkourState::ClimbingLadder => {
                let (Some(ladder), Some(grip)) = (motion.ladder, motion.ladder_grip) else {
                    motion.ladder = None;
                    motion.ladder_grip = None;
                    motion.enter(ParkourState::Airborne);
                    continue;
                };
                let cfg = ladder_cfg.get(ladder).copied().unwrap_or_default();
                motion.velocity = Vec3::ZERO;
                face = Some(-grip.wall_normal);

                let up = move_dir.y.clamp(-1.0, 1.0);
                let climbed = transform.translation
                    + Vec3::Y * (up * controller.climb_speed * cfg.climb_speed_scale * dt);
                // XZ is held at the grip: a ladder is a rail, and letting the
                // stick push the character sideways off it mid-climb is the
                // single most common way ladder controllers feel broken.
                warp = Some(Vec3::new(grip.point.x, climbed.y, grip.point.z));

                let leaving = released || motion.jump_buffer > 0.0;
                let top_out = p.ledge.filter(|l| {
                    cfg.exit_at_top && up > 0.3 && l.clear && l.height <= controller.mantle_max_height
                });
                if let Some(ledge) = top_out {
                    start_mantle(
                        &mut motion,
                        controller,
                        &transform,
                        &ledge,
                        TraversalExit::Free,
                    );
                    motion.ladder = None;
                    motion.ladder_grip = None;
                    events.push(ParkourEventKind::LadderDismount);
                    events.push(ParkourEventKind::MantleStart);
                } else if leaving {
                    // Stepping off pushes away from the ladder; jumping off
                    // adds height on top of that.
                    let lift = if motion.jump_buffer > 0.0 {
                        jump_speed * 0.5
                    } else {
                        0.0
                    };
                    motion.velocity = grip.wall_normal * 2.5 + Vec3::Y * lift;
                    motion.jump_buffer = 0.0;
                    motion.ladder = None;
                    motion.ladder_grip = None;
                    motion.grab_cooldown = 0.3;
                    motion.enter(ParkourState::Airborne);
                    events.push(ParkourEventKind::LadderDismount);
                } else if p.grounded && up < -0.05 {
                    motion.ladder = None;
                    motion.ladder_grip = None;
                    motion.enter(ParkourState::Grounded);
                    events.push(ParkourEventKind::LadderDismount);
                }
            }

            // ── Wall run ─────────────────────────────────────────────────
            ParkourState::WallRunning => {
                // Confirm the wall is still beside us, and re-read its normal
                // so the run follows a curved wall instead of drifting off it.
                let still = [p.wall_left, p.wall_right]
                    .into_iter()
                    .flatten()
                    .find(|w| w.normal.dot(motion.wall_normal) > 0.6);
                let expired = motion.state_time > controller.wall_run_duration;
                if let (Some(w), false, false) = (still, expired, p.grounded) {
                    let n = w.normal;
                    motion.wall_normal = n;
                    let tangent = n.cross(Vec3::Y).normalize_or_zero();
                    let along = if motion.velocity.dot(tangent) >= 0.0 {
                        tangent
                    } else {
                        -tangent
                    };
                    let rise = motion.velocity.y + controller.wall_run_gravity * dt;
                    motion.velocity = along * controller.wall_run_speed + Vec3::Y * rise;
                    face = Some(along);
                    // Contact is held by correcting the *position* after the
                    // move, not by adding inward velocity. An inward push has
                    // nowhere to go: the slide stops it a skin-width short of
                    // the surface every frame, so the capsule creeps deeper
                    // into the wall until the contact normal comes back
                    // sideways. After that the wall jump, which is a velocity
                    // straight out of that wall, gets projected to nothing and
                    // the character just slides down it.
                    wall_hold = Some((n, w.distance));

                    if motion.jump_buffer > 0.0 {
                        motion.velocity =
                            n * controller.wall_jump_away + Vec3::Y * controller.wall_jump_up;
                        motion.jump_buffer = 0.0;
                        motion.wall_cooldown = 0.25;
                        wall_hold = None;
                        motion.enter(ParkourState::Airborne);
                        events.push(ParkourEventKind::WallJump);
                        events.push(ParkourEventKind::WallRunEnd);
                    }
                } else {
                    motion.wall_cooldown = 0.2;
                    motion.enter(if p.grounded {
                        ParkourState::Grounded
                    } else {
                        ParkourState::Airborne
                    });
                    events.push(ParkourEventKind::WallRunEnd);
                }
            }

            // ── Swing ────────────────────────────────────────────────────
            ParkourState::Swinging => {
                let Some(link) = motion.swing else {
                    motion.enter(ParkourState::Airborne);
                    continue;
                };
                let Ok((_, anchor_tf, anchor_cfg)) = anchors.get(link.anchor) else {
                    // The anchor was despawned mid-swing.
                    motion.swing = None;
                    motion.enter(ParkourState::Airborne);
                    continue;
                };
                let pivot = anchor_tf.translation();
                let hand_off = Vec3::Y * (controller.height * 0.9);
                let hand = transform.translation + hand_off;

                motion.velocity.y += controller.gravity * dt;
                // Rider input pumps the swing along its travel direction. It
                // is deliberately weak: a swing the player can drive like a
                // walk stops reading as a rope.
                if move_len > 0.05 {
                    motion.velocity += wish * controller.acceleration * 0.08 * dt * move_len;
                }

                let mut next = hand + motion.velocity * dt;
                let mut rel = next - pivot;
                let dist = rel.length();
                if dist > 1e-4 {
                    rel = rel / dist * link.length;
                    next = pivot + rel;
                    // Drop the radial component: a rope pulls, it does not
                    // push, and leaving it in makes the pendulum gain energy.
                    let radial = rel / link.length;
                    let along_rope = motion.velocity.dot(radial);
                    motion.velocity -= radial * along_rope;
                }
                let damp = (1.0 - anchor_cfg.damping * dt).clamp(0.0, 1.0);
                motion.velocity *= damp;

                // A swing is a warp, so it would happily pass through a wall.
                // Sweep the step and let go on contact rather than clipping.
                let step = next - hand;
                let hit = Dir3::new(step).ok().and_then(|dir| {
                    spatial.cast_shape(
                        &sweep.0,
                        transform.translation + Vec3::Y * half,
                        Quat::IDENTITY,
                        dir,
                        &ShapeCastConfig {
                            max_distance: step.length(),
                            ignore_origin_penetration: true,
                            ..Default::default()
                        },
                        &filter,
                    )
                });
                if hit.is_some() {
                    motion.swing = None;
                    motion.grab_cooldown = 0.4;
                    motion.enter(ParkourState::Airborne);
                    events.push(ParkourEventKind::SwingRelease);
                } else {
                    warp = Some(next - hand_off);
                    face = Some(motion.velocity.with_y(0.0));

                    if released || motion.jump_buffer > 0.0 {
                        let boost = if motion.jump_buffer > 0.0 {
                            controller.swing_release_boost + jump_speed * 0.5
                        } else {
                            controller.swing_release_boost
                        };
                        motion.velocity += Vec3::Y * boost;
                        motion.jump_buffer = 0.0;
                        motion.swing = None;
                        motion.grab_cooldown = 0.4;
                        motion.enter(ParkourState::Airborne);
                        events.push(ParkourEventKind::SwingRelease);
                    }
                }
            }
        }

        // ── Move ─────────────────────────────────────────────────────────
        let mut grounded = p.grounded;
        match warp {
            Some(pos) => transform.translation = pos,
            None => {
                let centre = transform.translation - Vec3::Y * controller.foot_offset
                    + Vec3::Y * half;
                let slide = shape_cast_slide(
                    &spatial,
                    &sweep.0,
                    centre,
                    Quat::IDENTITY,
                    motion.velocity * dt,
                    max_slope,
                    &filter,
                );
                transform.translation += slide.actual_delta;
                grounded = slide.grounded;

                // Re-read velocity from what actually happened. Running into a
                // wall must not keep accumulating speed into it, or the
                // character shoots sideways the moment the wall ends.
                let realized = slide.actual_delta / dt;
                motion.velocity.x = realized.x;
                motion.velocity.z = realized.z;
                if grounded && motion.velocity.y <= 0.0 {
                    motion.velocity.y = 0.0;
                } else {
                    motion.velocity.y = realized.y;
                }

                // Ground snap: put the soles exactly on the surface under the
                // capsule, up to `step_height` in either direction. This is
                // what makes the character follow stairs and slopes — down,
                // without leaving the surface on every crest and falling the
                // whole descent in small steps, and up, since collide-and-slide
                // on its own stops flat against a kerb rather than stepping
                // onto it.
                //
                // A ray from the capsule's centre line, not a shape cast: it
                // has to be able to move the character *up* onto a step, and it
                // has to give an exact surface height rather than a sweep
                // distance that is zero whenever the capsule is already
                // touching. Walking off an edge is then simply the ray missing,
                // which leaves `grounded` false and drops the character into
                // the air state next frame.
                if motion.state == ParkourState::Grounded {
                    let probe_top =
                        transform.translation - Vec3::Y * controller.foot_offset
                            + Vec3::Y * controller.step_height;
                    if let Some(h) = spatial.cast_ray(
                        probe_top,
                        Dir3::NEG_Y,
                        controller.step_height * 2.0,
                        true,
                        &filter,
                    ) {
                        if h.normal.angle_between(Vec3::Y) <= max_slope.to_radians() {
                            // Held a hair ABOVE the surface, not exactly on it.
                            // Resting precisely at the contact plane puts the
                            // capsule on a knife edge: the next horizontal
                            // sweep intermittently reports a zero-distance hit
                            // against the floor and eats part of the step, and
                            // the character walks at roughly three quarters of
                            // the speed it was asked for. The gap is far below
                            // the grounded probes, so nothing else notices.
                            transform.translation.y = probe_top.y - h.distance
                                + controller.foot_offset
                                + GROUND_SKIN;
                            grounded = true;
                        }
                    }
                }
            }
        }

        // Hold the wall run a constant distance off the surface. Clamped, so a
        // probe that suddenly reads a far-away wall (rounding a corner) cannot
        // teleport the character sideways.
        if let Some((n, distance)) = wall_hold {
            let want = controller.radius + 0.05;
            transform.translation -= n * (distance - want).clamp(-0.15, 0.15);
        }

        // ── Facing ───────────────────────────────────────────────────────
        if controller.face_movement {
            if let Some(dir) = face.map(|d| d.with_y(0.0)).filter(|d| d.length() > 1e-3) {
                motion.facing = lerp_angle(
                    motion.facing,
                    yaw_of(dir),
                    (controller.turn_speed * dt).clamp(0.0, 1.0),
                );
            }
            // The offset is applied here and nowhere else, so it stays purely
            // a matter of which way the mesh points.
            transform.rotation =
                Quat::from_rotation_y(motion.facing + controller.facing_offset.to_radians());
        }

        // ── Mirror + events ──────────────────────────────────────────────
        if read.state != motion.state.as_str() {
            read.state = motion.state.as_str().to_string();
        }
        read.grounded = grounded;
        read.velocity = motion.velocity;
        read.speed = motion.velocity.length();
        read.traversing = motion.state.is_traversal();
        read.hanging = motion.state == ParkourState::Hanging;
        read.climbing = motion.state == ParkourState::ClimbingLadder;
        read.wall_running = motion.state == ParkourState::WallRunning;
        read.swinging = motion.state == ParkourState::Swinging;
        read.can_vault = p
            .ledge
            .is_some_and(|l| l.thin && l.height <= controller.vault_max_height);
        read.can_mantle = p
            .ledge
            .is_some_and(|l| l.clear && l.height <= controller.mantle_max_height);
        read.can_grab = p.ledge.is_some_and(|l| {
            l.height >= controller.height * 0.55 && l.height <= controller.height * 1.15
        });
        read.near_ladder = p.ladder.is_some();
        read.ledge_height = p.ledge.map(|l| l.height).unwrap_or(0.0);
        // A one-frame pulse: scripts poll once per frame in `on_update`, so a
        // sticky value would fire the same footstep sound until the next event.
        read.event = events.first().map(|e| e.as_str().to_string()).unwrap_or_default();

        for kind in events {
            commands.trigger(ParkourEvent { entity, kind });
        }
    }
}

/// Begin a vault: over the lip and down onto the far side.
fn start_vault(
    motion: &mut ParkourMotion,
    controller: &ParkourController,
    transform: &Transform,
    ledge: &crate::probe::Ledge,
    forward: Vec3,
) {
    let start = transform.translation;
    let end = ledge.landing + Vec3::Y * controller.foot_offset;
    motion.traversal = Some(Traversal {
        start,
        apex: arc_control(start, end, ledge.top.y + controller.foot_offset + 0.25),
        end,
        duration: controller.vault_duration,
        elapsed: 0.0,
        exit: TraversalExit::Free,
        // Keep going at the speed the vault was entered with, so a run doesn't
        // stop dead on the far side.
        exit_velocity: forward * controller.run_speed * 0.6,
    });
    motion.facing = yaw_of(forward);
    motion.enter(ParkourState::Vaulting);
}

/// Begin a mantle: up the face and onto the top.
fn start_mantle(
    motion: &mut ParkourMotion,
    controller: &ParkourController,
    transform: &Transform,
    ledge: &crate::probe::Ledge,
    exit: TraversalExit,
) {
    let start = transform.translation;
    // Far enough in from the lip that the capsule ends up standing on the
    // surface rather than balanced on its edge.
    let inward = -ledge.face_normal.with_y(0.0).normalize_or_zero();
    let end = ledge.top + inward * (controller.radius + 0.15) + Vec3::Y * controller.foot_offset;
    motion.traversal = Some(Traversal {
        start,
        apex: arc_control(start, end, end.y + 0.35),
        end,
        duration: controller.mantle_duration,
        elapsed: 0.0,
        exit,
        exit_velocity: Vec3::ZERO,
    });
    motion.facing = yaw_of(inward);
    motion.enter(ParkourState::Mantling);
}

/// Latch onto a ladder at the current height.
fn mount_ladder(
    motion: &mut ParkourMotion,
    ladder: Entity,
    p: &crate::probe::ParkourProbe,
    foot: Vec3,
) {
    // The face normal from the probe is the climbable side; hugging it keeps
    // the character on the front of the ladder rather than inside it.
    let normal = p
        .ledge
        .map(|l| l.face_normal)
        .or(p.wall_front.map(|w| w.normal))
        .unwrap_or(Vec3::Z);
    motion.ladder = Some(ladder);
    motion.ladder_grip = Some(HangGrip {
        point: foot,
        wall_normal: normal.with_y(0.0).normalize_or_zero(),
    });
    motion.velocity = Vec3::ZERO;
    motion.enter(ParkourState::ClimbingLadder);
}

/// Nearest swing anchor that is in reach, roughly ahead, and not behind a wall.
fn nearest_anchor(
    anchors: &Query<(Entity, &GlobalTransform, &ParkourSwingAnchor)>,
    spatial: &SpatialQuery,
    filter: &SpatialQueryFilter,
    head: Vec3,
    forward: Vec3,
) -> Option<AnchorCandidate> {
    let mut best: Option<(f32, AnchorCandidate)> = None;
    for (entity, gt, cfg) in anchors.iter() {
        let point = gt.translation();
        let to = point - head;
        let dist = to.length();
        if dist > cfg.max_grab_distance || dist < 1e-3 {
            continue;
        }
        // Anchors behind the character are ignored; ones above are not, since
        // a bar overhead is the normal case.
        if to.with_y(0.0).normalize_or_zero().dot(forward) < -0.2 {
            continue;
        }
        let Ok(dir) = Dir3::new(to) else { continue };
        if spatial
            .cast_ray(head, dir, (dist - 0.2).max(0.0), true, filter)
            .is_some()
        {
            continue;
        }
        if best.as_ref().is_none_or(|(d, _)| dist < *d) {
            best = Some((
                dist,
                AnchorCandidate {
                    entity,
                    point,
                    rope: cfg.rope_length,
                },
            ));
        }
    }
    best.map(|(_, a)| a)
}

/// Bézier control point that makes the curve pass through `peak_y` at its
/// midpoint. The control point is not on the curve, so it has to be lifted
/// twice as far as the height actually wanted.
fn arc_control(start: Vec3, end: Vec3, peak_y: f32) -> Vec3 {
    let mid = (start + end) * 0.5;
    Vec3::new(mid.x, 2.0 * peak_y - 0.5 * (start.y + end.y), mid.z)
}

/// Move `cur` toward `target` by at most `max_delta`.
fn approach(cur: Vec3, target: Vec3, max_delta: f32) -> Vec3 {
    let delta = target - cur;
    let len = delta.length();
    if len <= max_delta || len < 1e-6 {
        target
    } else {
        cur + delta / len * max_delta
    }
}

/// Yaw that points a Bevy-forward (`-Z`) entity along `dir`.
pub(crate) fn yaw_of(dir: Vec3) -> f32 {
    (-dir.x).atan2(-dir.z)
}

/// Interpolate between two yaws the short way around.
fn lerp_angle(from: f32, to: f32, t: f32) -> f32 {
    let mut diff = (to - from) % std::f32::consts::TAU;
    if diff > std::f32::consts::PI {
        diff -= std::f32::consts::TAU;
    } else if diff < -std::f32::consts::PI {
        diff += std::f32::consts::TAU;
    }
    from + diff * t
}
