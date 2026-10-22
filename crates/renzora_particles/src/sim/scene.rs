//! The simulation's half of a Collide Scene node: each particle's motion
//! since the scene last saw it, as a ray, and a hit, as a bounce or a death.
//!
//! The scene's colliders can only be asked with the world in hand, which a
//! step does not have, so this runs after the frame's steps rather than inside
//! them (the runtime half is `crate::scene`). Each particle remembers where it
//! was when it was last checked ([`ScenePoint::from`]); a ray from there to
//! where it is now covers every step of the frame at once, and a particle the
//! per-frame cap left out is swept over its whole path next time instead of
//! skipping a frame of it. The response is the analytic colliders' own
//! ([`collide::respond`]): the same bounce, Coulomb friction and resting rule,
//! with the bounce, friction and radius the particle's last step evaluated.
//!
//! Everything here is in world space at the boundary: a `Local` emitter's
//! particles go out through the entity's transform and come back through its
//! inverse, so a torch's sparks hit the floor under the torch wherever it is
//! carried.

use bevy::math::Affine3A;
use renzora::scene_query::{SceneHit, SceneRay};

use super::{collide, sanitize3, EffectSim, ScenePoint};
use crate::asset::SimulationSpace;
use crate::ir::{ColliderIr, ColliderShape, CompiledEmitter};

/// How far off the surface a particle is put back, in world units. Exactly on
/// it, the next frame's ray would start touching the collider and a resting
/// particle would hit at distance zero every frame.
pub const SKIN: f32 = 1e-3;

/// Shorter moves than this cast nothing: a particle resting on the ground or
/// paused in the editor costs no query.
const MIN_TRAVEL: f32 = 1e-5;

/// The emitter's Collide Scene node. Only the first counts: one scene is one
/// set of surfaces, and two nodes would only disagree about the bounce.
pub fn first_scene_collider(compiled: &CompiledEmitter) -> Option<&ColliderIr> {
    compiled.colliders.iter().find(|c| c.shape == ColliderShape::Scene)
}

/// Which particle a ray in the batch belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SceneProbe {
    /// Slot in [`EffectSim::emitters`].
    pub emitter: usize,
    pub particle: usize,
}

/// A `Local` emitter's particles are in the entity's space; a `World` one's
/// already in the world's.
fn to_world(space: SimulationSpace, transform: &Affine3A) -> Affine3A {
    match space {
        SimulationSpace::Local => *transform,
        SimulationSpace::World => Affine3A::IDENTITY,
    }
}

impl EffectSim {
    /// Whether any emitter running on the CPU collides with the scene.
    pub fn wants_scene(&self) -> bool {
        self.emitters.iter().any(|e| e.gpu.is_none() && e.particles.scene_tracking)
    }

    /// Queue a ray for each particle that has moved since the scene last saw
    /// it, at most `budget` of them (which this spends), carrying on where the
    /// last call stopped so every particle gets its turn. `planar` asks the
    /// 2D world: a 2D effect hits the 2D level.
    pub fn scene_rays(
        &mut self,
        transform: &Affine3A,
        planar: bool,
        budget: &mut usize,
        rays: &mut Vec<SceneRay>,
        probes: &mut Vec<SceneProbe>,
    ) {
        for (slot, e) in self.emitters.iter_mut().enumerate() {
            if e.gpu.is_some() || !e.particles.scene_tracking || *budget == 0 {
                continue;
            }
            let n = e.particles.len();
            if n == 0 {
                continue;
            }
            let out = to_world(e.space, transform);
            // A radius in the entity's units is that many of its scaled
            // metres in the world.
            let scale = match e.space {
                SimulationSpace::Local => {
                    let m = out.matrix3;
                    m.x_axis.length().max(m.y_axis.length()).max(m.z_axis.length())
                }
                SimulationSpace::World => 1.0,
            };
            let start = e.scene_cursor % n;
            let mut scanned = 0;
            while scanned < n && *budget > 0 {
                let i = (start + scanned) % n;
                scanned += 1;
                let s = e.particles.scene[i];
                let a = out.transform_point3(s.from);
                let b = out.transform_point3(e.particles.position[i]);
                let travel = b - a;
                let len = travel.length();
                if !len.is_finite() || len < MIN_TRAVEL {
                    continue;
                }
                rays.push(SceneRay {
                    origin: a,
                    direction: travel / len,
                    max_distance: len,
                    radius: (s.radius * scale).max(0.0),
                    planar,
                });
                probes.push(SceneProbe { emitter: slot, particle: i });
                *budget -= 1;
            }
            e.scene_cursor = (start + scanned) % n;
        }
    }

    /// Apply the scene's answers to the rays [`EffectSim::scene_rays`]
    /// queued, `hits[k]` for `probes[k]`: put a particle that hit back on the
    /// surface and bounce it, or kill it on a Kill On Hit node; either way it
    /// now counts as seen where it is. Killed particles raise their death
    /// events, which the next step delivers.
    pub fn apply_scene_hits(&mut self, transform: &Affine3A, probes: &[SceneProbe], hits: &[Option<SceneHit>]) {
        let inverse = transform.inverse();
        let inverse = inverse.is_finite().then_some(inverse);
        let mut killed: Vec<(usize, usize)> = Vec::new();
        for (probe, hit) in probes.iter().zip(hits) {
            let Some(e) = self.emitters.get_mut(probe.emitter) else { continue };
            let i = probe.particle;
            if i >= e.particles.len() || i >= e.particles.scene.len() {
                continue;
            }
            let kill = first_scene_collider(&e.compiled).is_some_and(|c| c.kill);
            let Some(hit) = hit else {
                e.particles.scene[i].from = e.particles.position[i];
                continue;
            };
            let (out, back) = match e.space {
                SimulationSpace::World => (Affine3A::IDENTITY, Affine3A::IDENTITY),
                SimulationSpace::Local => match inverse {
                    Some(inv) => (*transform, inv),
                    // An entity scaled to nothing has no way back in.
                    None => continue,
                },
            };
            let s: ScenePoint = e.particles.scene[i];
            let origin = out.transform_point3(s.from);
            let direction = (out.transform_point3(e.particles.position[i]) - origin).normalize_or_zero();
            let normal = hit.normal.normalize_or_zero();
            let rest = origin + direction * hit.distance.max(0.0) + normal * SKIN;
            let position = sanitize3(back.transform_point3(rest));
            e.particles.position[i] = position;
            e.particles.scene[i].from = position;
            if kill {
                killed.push((probe.emitter, i));
                continue;
            }
            let mut v = out.transform_vector3(e.particles.velocity[i]);
            let accel_dt = out.transform_vector3(s.accel_dt);
            collide::respond(&mut v, normal, s.bounce, s.friction, accel_dt);
            e.particles.velocity[i] = sanitize3(back.transform_vector3(v));
        }
        // Highest index first per emitter, as the step removes its own
        // collision deaths: a swap-remove only moves a particle from past the
        // one removed.
        killed.sort_unstable_by(|a, b| b.cmp(a));
        killed.dedup();
        for (slot, i) in killed {
            let e = &mut self.emitters[slot];
            if i < e.particles.len() {
                e.raise_death(i);
                e.particles.swap_remove(i);
            }
        }
    }

    /// Every particle counts as seen where it is now. After a seek, which
    /// replays without a world to ask: the scene's next look starts from the
    /// replayed positions, not from the births the replay swept past.
    pub fn scene_seen_all(&mut self) {
        for e in &mut self.emitters {
            let p = &mut e.particles;
            for (s, &at) in p.scene.iter_mut().zip(&p.position) {
                s.from = at;
            }
        }
    }
}

/// A test service: an infinite ground plane at `y = height` facing up, which
/// is all most scene collision tests need and what a level's floor is.
#[cfg(test)]
pub(crate) fn ground_hits(rays: &[SceneRay], height: f32) -> Vec<Option<SceneHit>> {
    use bevy::math::Vec3;
    rays.iter()
        .map(|r| {
            let above = r.origin.y - r.radius - height;
            if r.direction.y >= 0.0 || above < 0.0 {
                return None;
            }
            let distance = above / -r.direction.y;
            (distance <= r.max_distance).then(|| SceneHit {
                distance,
                point: r.origin + r.direction * distance - Vec3::Y * r.radius,
                normal: Vec3::Y,
                entity: bevy::ecs::entity::Entity::PLACEHOLDER,
            })
        })
        .collect()
}
