//! A swarm that follows a moving attractor. The attractor traces a 3D
//! Lissajous path, and every member eases toward it each frame (frame-rate
//! independent exponential easing, so there is no overshoot), with a
//! per-member jitter so the cloud breathes instead of collapsing to a point.
//!
//! The motion is a function of elapsed time, so peers see the same swarm once
//! it has settled; only the starting scatter is random.
//!
//! Setup: attach to a manager entity. `SWARM` sets the member count; members
//! are spawned as `swarm_<n>`.

use bevy::ecs::system::SystemState;
use bevy::gizmos::config::GizmoConfigStore;
use bevy::prelude::*;
use renzora::core::{MeshColor, MeshPrimitive};
use renzora::{ScriptCtx, ScriptHook};

const SWARM: usize = 40;
const MEMBER_SIZE: f32 = 0.25;
/// How quickly members close on the attractor.
const PULL: f32 = 2.5;

#[derive(Component)]
struct Swarm {
    members: Vec<Entity>,
}

fn update(ctx: &mut ScriptCtx) {
    let t = ctx.elapsed();
    let k = (ctx.delta() * PULL).clamp(0.0, 1.0);
    let attractor = Vec3::new((t * 0.7).sin() * 5.0, 3.0 + (t * 1.3).sin() * 2.0, (t * 0.9).cos() * 5.0);
    let Some(swarm) = ctx.get::<Swarm>() else { return };
    let members = swarm.members.clone();
    for (i, member) in members.into_iter().enumerate() {
        let i = (i + 1) as f32;
        let goal = attractor + Vec3::new((t + i).sin() * 0.6, i.sin() * 0.6, (t * 1.1 + i).cos() * 0.6);
        if let Some(mut tr) = ctx.get_mut_on::<Transform>(member) {
            tr.translation = tr.translation.lerp(goal, k);
        }
    }
    draw_line(ctx.world(), attractor, attractor + Vec3::Y * 0.5);
}

fn hooks(ctx: &mut ScriptCtx, hook: &ScriptHook) {
    if !matches!(hook, ScriptHook::Ready) {
        return;
    }
    let mut seed = ctx.entity().to_bits() ^ ((ctx.elapsed().to_bits() as u64) << 32);
    let world = ctx.world();
    let members = (1..=SWARM)
        .map(|i| {
            let x = rand01(&mut seed) * 8.0 - 4.0;
            let y = rand01(&mut seed) * 8.0 - 4.0;
            let z = rand01(&mut seed) * 8.0 - 4.0;
            // The Lua version scaled the manager instead of the members.
            world
                .spawn((
                    Name::new(format!("swarm_{i}")),
                    Transform::from_xyz(x, y + 3.0, z).with_scale(Vec3::splat(MEMBER_SIZE)),
                    Visibility::default(),
                    MeshPrimitive("sphere".into()),
                    MeshColor(Color::srgb(0.9, 0.5, 1.0)),
                ))
                .id()
        })
        .collect();
    ctx.insert(Swarm { members });
}

/// A one-frame debug line marking the attractor.
fn draw_line(world: &mut World, a: Vec3, b: Vec3) {
    // Gizmos are absent in a build without the gizmo plugin; drawing is optional.
    if !world.contains_resource::<GizmoConfigStore>() {
        return;
    }
    let mut state = SystemState::<Gizmos>::new(world);
    if let Ok(mut gizmos) = state.get_mut(world) {
        gizmos.line(a, b, Color::WHITE);
    }
    state.apply(world);
}

/// A uniform value in 0..1 from a SplitMix64 step; scripts have no rand crate.
fn rand01(seed: &mut u64) -> f32 {
    *seed = seed.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *seed;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^= z >> 31;
    (z >> 40) as f32 / (1u64 << 24) as f32
}

renzora::script!(update, hooks = hooks);
