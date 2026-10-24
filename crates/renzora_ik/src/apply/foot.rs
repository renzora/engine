//! Foot IK against the physics world.
//!
//! Three systems because a cast needs the whole `World` read-only (that is the
//! [`PhysicsBackends::cast`] contract, so a backend can read its own state out
//! of it) while gathering and applying need mutable queries. Gathering writes
//! the rays into [`FootRays`], one exclusive system casts them, and applying
//! reads the hits back. Going through `renzora::physics` rather than a physics
//! crate means the feet stand on whatever the installed backend simulates, and
//! with no backend at all they stand on the component's fallback plane, so a
//! scene with no physics still gets planted feet on flat ground.

use crate::components::FootIk;
use crate::names::NameLookup;
use crate::pose::{track, world_of, Pose, PoseCache};
use crate::solve::{approach, foot, two_bone};
use bevy::prelude::*;
use renzora::physics::{Cast, CastBatch, CastHit, CollisionShapeData, PhysicsBackends, PhysicsDim};

/// How often, in frames, a character's own colliders are re-collected for the
/// rays to ignore. They change only when bodies are generated (a ragdoll
/// building its bones a few frames after load), so every frame would be waste.
const EXCLUDE_REFRESH_FRAMES: u32 = 60;

/// Per-character state the solver keeps between frames.
#[derive(Component, Default)]
pub struct FootIkState {
    /// Resolved left foot, right foot, hips, with the name each came from so a
    /// rename in the inspector re-resolves.
    bones: [Option<(String, Entity)>; 3],
    /// Smoothed ground offset and normal per foot.
    offsets: [f32; 2],
    normals: [Vec3; 2],
    exclude: Vec<Entity>,
    exclude_age: u32,
}

/// This frame's foot rays and their answers.
#[derive(Resource, Default)]
pub struct FootRays {
    batch: CastBatch,
    /// Which character and which foot (0 left, 1 right) each cast is for.
    owners: Vec<(Entity, usize)>,
    /// Whether a 3D backend answered at all. Without one the fallback plane is
    /// the ground; with one, a miss means there is no ground under that foot.
    have_backend: bool,
}

impl FootRays {
    fn hit(&self, owner: Entity, foot: usize) -> Option<CastHit> {
        let i = self.owners.iter().position(|o| *o == (owner, foot))?;
        self.batch.hits.get(i).copied().flatten()
    }
}

pub fn ensure_foot_state(mut commands: Commands, missing: Query<Entity, (With<FootIk>, Without<FootIkState>)>) {
    for entity in &missing {
        commands.entity(entity).try_insert(FootIkState { normals: [Vec3::Y; 2], ..default() });
    }
}

fn resolve(
    slot: &mut Option<(String, Entity)>,
    name: &str,
    owner: Entity,
    lookup: &NameLookup,
    names: &Query<&Name>,
    parents: &Query<&ChildOf>,
) -> Option<Entity> {
    let fresh = match slot {
        Some((n, e)) if n == name && names.get(*e).is_ok_and(|current| current.as_str() == name) => return Some(*e),
        _ => lookup.find_under(name, owner, names, parents),
    };
    *slot = fresh.map(|e| (name.to_string(), e));
    fresh
}

/// Collect the character's own colliders: everything under the `FootIk`
/// entity and its ancestors (the controller capsule usually sits on one).
/// Not the ancestors' other children: a character parented under a level
/// root would otherwise ignore the level.
fn collect_exclusions(
    owner: Entity,
    children: &Query<&Children>,
    parents: &Query<&ChildOf>,
    shapes: &Query<(), With<CollisionShapeData>>,
) -> Vec<Entity> {
    let mut out = Vec::new();
    let mut stack = vec![owner];
    while let Some(e) = stack.pop() {
        if shapes.get(e).is_ok() {
            out.push(e);
        }
        if let Ok(kids) = children.get(e) {
            stack.extend(kids.iter());
        }
    }
    let mut e = owner;
    while let Ok(p) = parents.get(e) {
        e = p.parent();
        if shapes.get(e).is_ok() {
            out.push(e);
        }
    }
    out
}

#[allow(clippy::too_many_arguments)]
pub fn gather_foot_rays(
    mut rays: ResMut<FootRays>,
    mut feet: Query<(Entity, &FootIk, &mut FootIkState)>,
    transforms: Query<&Transform>,
    parents: Query<&ChildOf>,
    children: Query<&Children>,
    shapes: Query<(), With<CollisionShapeData>>,
    lookup: Res<NameLookup>,
    names: Query<&Name>,
) {
    rays.batch.casts.clear();
    rays.batch.hits.clear();
    rays.owners.clear();
    let read = |e: Entity| transforms.get(e).ok().copied();
    for (owner, ik, mut state) in &mut feet {
        if ik.weight <= 0.0 {
            continue;
        }
        if state.exclude_age == 0 {
            state.exclude = collect_exclusions(owner, &children, &parents, &shapes);
        }
        state.exclude_age = (state.exclude_age + 1) % EXCLUDE_REFRESH_FRAMES;
        let state = &mut *state;
        for (i, name) in [&ik.left_foot, &ik.right_foot].into_iter().enumerate() {
            let Some(bone) = resolve(&mut state.bones[i], name, owner, &lookup, &names, &parents) else {
                continue;
            };
            let Some(ankle) = world_of(bone, &read, &parents) else {
                continue;
            };
            let height = ik.ray_height.max(0.0);
            let cast = Cast::ray(
                PhysicsDim::D3,
                ankle.translation + Vec3::Y * height,
                Vec3::NEG_Y,
                height + ik.ray_depth.max(0.0),
            )
            .excluding(state.exclude.iter().copied());
            rays.batch.casts.push(cast);
            rays.owners.push((owner, i));
        }
    }
}

pub fn cast_foot_rays(world: &mut World) {
    world.resource_scope(|world, mut rays: Mut<FootRays>| {
        let have_backend = world
            .get_resource::<PhysicsBackends>()
            .is_some_and(|b| b.selected(PhysicsDim::D3).is_some());
        rays.have_backend = have_backend;
        if have_backend {
            PhysicsBackends::cast(world, &mut rays.batch);
        } else {
            let n = rays.batch.casts.len();
            rays.batch.hits.clear();
            rays.batch.hits.resize(n, None);
        }
    });
}

#[allow(clippy::too_many_arguments)]
pub fn apply_foot_ik(
    mut commands: Commands,
    time: Res<Time>,
    rays: Res<FootRays>,
    mut feet: Query<(Entity, &FootIk, &mut FootIkState)>,
    mut transforms: Query<&mut Transform>,
    parents: Query<&ChildOf>,
    lookup: Res<NameLookup>,
    names: Query<&Name>,
    cached: Query<(), With<PoseCache>>,
) {
    let dt = time.delta_secs();
    let mut pose = Pose { transforms: &mut transforms, parents: &parents };
    for (owner, ik, mut state) in &mut feet {
        let w = ik.weight.clamp(0.0, 1.0);
        if w <= 0.0 {
            continue;
        }
        let Some(root) = pose.world(owner) else {
            continue;
        };
        let floor = root.translation.y;
        let k = approach(ik.smoothing, dt);

        // Each foot's ground offset, before the hips move anything.
        let mut legs: [Option<(Entity, Entity, Entity, Transform)>; 2] = [None, None];
        for (i, leg) in legs.iter_mut().enumerate() {
            let Some((_, ankle)) = state.bones[i].clone() else {
                continue;
            };
            let Some(knee) = pose.parent(ankle) else {
                continue;
            };
            let Some(thigh) = pose.parent(knee) else {
                continue;
            };
            let Some(ankle_world) = pose.world(ankle) else {
                continue;
            };
            let (ground, normal) = match rays.hit(owner, i) {
                Some(hit) => (Some(hit.point.y), hit.normal),
                None if !rays.have_backend => (Some(ik.ground_height), Vec3::Y),
                None => (None, Vec3::Y),
            };
            // No ground under a foot: ease it back to the animation.
            let target = ground.map_or(0.0, |g| foot::ground_offset(g, floor));
            state.offsets[i] += (target - state.offsets[i]) * k;
            state.normals[i] = state.normals[i].lerp(normal, k).try_normalize().unwrap_or(Vec3::Y);
            *leg = Some((thigh, knee, ankle, ankle_world));
        }
        if legs.iter().all(Option::is_none) {
            continue;
        }

        let present: Vec<f32> = (0..2).filter(|i| legs[*i].is_some()).map(|i| state.offsets[i]).collect();
        let drop = foot::pelvis_drop(&present, ik.max_hip_drop) * w;
        let hips_name = ik.hips.clone();
        if let Some(hips) = resolve(&mut state.bones[2], &hips_name, owner, &lookup, &names, &parents) {
            if let Some(hips_world) = pose.world(hips) {
                if let Some(local) = pose.local(hips) {
                    track(&mut commands, &cached, hips, local);
                }
                pose.set_world_translation(hips, hips_world.translation + Vec3::Y * drop);
            }
        }

        for (i, leg) in legs.iter().enumerate() {
            let Some((thigh, knee, ankle, animated)) = *leg else {
                continue;
            };
            for bone in [thigh, knee, ankle] {
                if let Some(local) = pose.local(bone) {
                    track(&mut commands, &cached, bone, local);
                }
            }
            let (Some(wt), Some(wk), Some(wa)) = (pose.world(thigh), pose.world(knee), pose.world(ankle)) else {
                continue;
            };
            let target = foot::plant(animated.translation, Vec3::Y, state.offsets[i] * w);
            // Keep the knee pointing where the animation points it; only a
            // dead-straight leg falls back to the character's forward.
            let bend = wk.translation - (wt.translation + wa.translation) * 0.5;
            let forward = root.rotation * Vec3::Z;
            let pole = wk.translation + bend.try_normalize().unwrap_or(forward);
            let (upper, lower) = two_bone::solve(wt.translation, wk.translation, wa.translation, target, Some(pole));
            pose.set_world_rotation(thigh, upper * wt.rotation);
            pose.set_world_rotation(knee, lower * wk.rotation);
            let foot_rotation = if ik.align_to_ground {
                let aligned = foot::align(animated.rotation, Vec3::Y, state.normals[i], ik.max_slope.to_radians());
                animated.rotation.slerp(aligned, w)
            } else {
                animated.rotation
            };
            pose.set_world_rotation(ankle, foot_rotation);
        }
    }
}
