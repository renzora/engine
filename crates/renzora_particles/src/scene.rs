//! Collide Scene: particles hitting the scene's own physics colliders.
//!
//! This crate links no physics engine. The scene is asked through the contract
//! crate's `renzora::scene_query`, which the physics crate answers when it is
//! in the build; without it there is no `SceneQueries` resource and the node
//! does nothing, exactly as if the scene were empty.
//!
//! A query needs the world (the collider tree is read by a system parameter),
//! so it cannot happen inside a step. [`collide_with_scene`] is an exclusive
//! system that runs right after the frame's steps: it gathers every CPU
//! emitter's rays (each particle's motion since the scene last saw it, see
//! [`crate::sim::scene`]), asks the scene once for the whole batch, and hands
//! each effect its answers. The cost is bounded by
//! [`MAX_SCENE_RAYS_PER_FRAME`]; past it, particles take turns, and a particle
//! that waited is swept over its whole path when its turn comes, so waiting
//! never lets it through a wall.
//!
//! What this gives up, against the analytic colliders: a hit lands at the end
//! of the frame rather than of the step (a particle that would have bounced
//! twice in one frame bounces once), a seek replays without the scene, and a
//! GPU emitter cannot use it at all (`gpu::select` keeps it on the CPU and says
//! so in the Running On row).

use bevy::prelude::*;
use renzora::scene_query::{SceneQueries, SceneQueryBatch};

use crate::asset::Dimension;
use crate::sim::scene::SceneProbe;
use crate::systems::ParticleRuntime;

/// The most scene casts the particle system makes in one frame, across every
/// effect. A cast is a walk of the collider tree; a few thousand a frame is a
/// small part of a physics frame, and a sparks burst rarely needs more.
pub const MAX_SCENE_RAYS_PER_FRAME: usize = 4096;

/// Reused between frames, so a steady state allocates nothing.
#[derive(Default)]
pub struct SceneScratch {
    batch: SceneQueryBatch,
    probes: Vec<SceneProbe>,
    /// Which effect owns which run of `probes`.
    owners: Vec<(Entity, usize, usize)>,
    effects: Vec<Entity>,
    /// Which effect goes first this frame, so when the budget runs out it is
    /// not always the same effects that wait.
    turn: usize,
}

/// See the module docs.
pub fn collide_with_scene(world: &mut World, mut scratch: Local<SceneScratch>) {
    let Some(service) = world.get_resource::<SceneQueries>().cloned() else { return };
    let s = &mut *scratch;
    s.batch.clear();
    s.probes.clear();
    s.owners.clear();
    s.effects.clear();

    let mut runtimes = world.query::<(Entity, &GlobalTransform, &mut ParticleRuntime)>();
    s.effects.extend(runtimes.iter_mut(world).filter(|(_, _, r)| r.sim.wants_scene()).map(|(e, _, _)| e));
    if s.effects.is_empty() {
        return;
    }
    s.turn = s.turn.wrapping_add(1);
    let first = s.turn % s.effects.len();
    s.effects.rotate_left(first);

    let mut budget = MAX_SCENE_RAYS_PER_FRAME;
    for &entity in &s.effects {
        if budget == 0 {
            break;
        }
        let Ok((_, global, mut runtime)) = runtimes.get_mut(world, entity) else { continue };
        let start = s.probes.len();
        let planar = runtime.dimension == Dimension::TwoD;
        runtime.sim.scene_rays(&global.affine(), planar, &mut budget, &mut s.batch.rays, &mut s.probes);
        if s.probes.len() > start {
            s.owners.push((entity, start, s.probes.len()));
        }
    }
    if s.batch.rays.is_empty() {
        return;
    }

    service.cast(world, &mut s.batch);

    for &(entity, a, b) in &s.owners {
        // The transform the rays were built with: nothing moves the entity
        // between the gather and here.
        let Ok((_, global, mut runtime)) = runtimes.get_mut(world, entity) else { continue };
        let transform = global.affine();
        runtime.sim.apply_scene_hits(&transform, &s.probes[a..b], &s.batch.hits[a..b]);
    }
}
