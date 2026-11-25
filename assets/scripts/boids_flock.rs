//! Reynolds boids: spawn `COUNT` cubes and steer them with cohesion,
//! alignment and separation every frame. Flocking needs per-agent state the
//! engine does not track, so positions and velocities live in the script's
//! state and are written back onto the spawned entities. The O(n^2) neighbour
//! scan is fine for a small flock and keeps it dependency free.
//!
//! The three rules alone never bring a flock back: once it picks a heading it
//! keeps it, and the Lua original was off screen within a minute. A weak pull
//! toward the manager keeps it circling there.
//!
//! Setup: attach to an empty manager entity; the flock spawns and circles
//! around it. The boids are named "boid_<n>".
//! Visual only: each peer runs its own, unsynced.

use bevy::prelude::*;
use renzora::core::{MeshColor, MeshPrimitive};
use renzora::{ScriptCtx, ScriptHook};

const COUNT: usize = 24;
/// Neighbours closer than this influence a boid.
const RADIUS: f32 = 4.0;
const MAX_SPEED: f32 = 3.0;
const COHESION: f32 = 0.5;
const ALIGNMENT: f32 = 0.6;
const SEPARATION: f32 = 1.4;
/// Flight height.
const HEIGHT: f32 = 2.0;
/// How hard each boid is steered back toward the manager.
const HOME_PULL: f32 = 0.15;

struct Boid {
    entity: Entity,
    pos: Vec3,
    vel: Vec3,
}

#[derive(Component)]
struct Flock(Vec<Boid>);

fn update(ctx: &mut ScriptCtx) {
    // A long frame would fling the flock apart.
    let dt = ctx.delta().clamp(0.0, 0.05);
    let home = ctx.get::<GlobalTransform>().map_or(Vec3::ZERO, |t| t.translation());
    let Some(mut flock) = ctx.get_mut::<Flock>() else { return };
    let boids = &mut flock.0;
    for i in 0..boids.len() {
        let me = boids[i].pos;
        let (mut coh, mut ali, mut sep, mut n) = (Vec3::ZERO, Vec3::ZERO, Vec3::ZERO, 0.0);
        for (j, other) in boids.iter().enumerate() {
            if i == j {
                continue;
            }
            let offset = other.pos - me;
            let d = offset.length();
            if d < RADIUS && d > 0.0 {
                coh += other.pos;
                ali += other.vel;
                sep -= offset / d;
                n += 1.0;
            }
        }
        let boid = &mut boids[i];
        let mut steer = (home - me) * HOME_PULL;
        if n > 0.0 {
            steer += (coh / n - me) * COHESION + ali / n * ALIGNMENT + sep * SEPARATION;
        }
        // Steering stays on the XZ plane.
        boid.vel += Vec3::new(steer.x, 0.0, steer.z) * dt;
        boid.vel = boid.vel.clamp_length_max(MAX_SPEED);
        boid.pos += boid.vel * dt;
    }
    let placed: Vec<(Entity, Vec3)> = boids.iter().map(|b| (b.entity, b.pos)).collect();
    drop(flock);

    for (entity, pos) in placed {
        if let Some(mut transform) = ctx.get_mut_on::<Transform>(entity) {
            transform.translation.x = pos.x;
            transform.translation.z = pos.z;
        }
    }
}

fn hooks(ctx: &mut ScriptCtx, hook: &ScriptHook) {
    if let ScriptHook::Ready = hook {
        let mut rng = ctx.entity().to_bits() ^ u64::from(ctx.elapsed().to_bits());
        let home = ctx.get::<GlobalTransform>().map_or(Vec3::ZERO, |t| t.translation());
        let world = ctx.world();
        let boids = (0..COUNT)
            .map(|i| {
                let x = home.x + next_f32(&mut rng) * 10.0 - 5.0;
                let z = home.z + next_f32(&mut rng) * 10.0 - 5.0;
                let vel = Vec3::new(next_f32(&mut rng) - 0.5, 0.0, next_f32(&mut rng) - 0.5);
                let pos = Vec3::new(x, HEIGHT, z);
                let entity = world
                    .spawn((
                        Name::new(format!("boid_{}", i + 1)),
                        Transform::from_translation(pos),
                        Visibility::default(),
                        MeshPrimitive("cube".into()),
                        MeshColor(Color::srgb(0.3, 0.8, 0.5)),
                    ))
                    .id();
                Boid { entity, pos, vel }
            })
            .collect();
        ctx.insert(Flock(boids));
    }
}

/// A uniform value in 0..1 from a 64-bit LCG; there is no rand crate.
fn next_f32(state: &mut u64) -> f32 {
    *state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
    (*state >> 40) as f32 / (1u64 << 24) as f32
}

renzora::script!(update, hooks = hooks);
