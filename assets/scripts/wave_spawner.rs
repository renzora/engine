//! A timed wave director. A repeating countdown announces each wave, and the
//! enemy count ramps per wave so difficulty climbs. Spawns scatter inside a
//! box so they do not stack on one point, and the shared name prefix keeps
//! cleanup to one sweep.
//!
//! Setup: place at the arena centre. Enemies are red cubes named
//! "wave_enemy"; swap the spawn for a prefab if you have one.

use bevy::prelude::*;
use renzora::core::{MeshColor, MeshPrimitive};
use renzora::script_bridge::ScriptAction;
use renzora::{ScriptCtx, ScriptHook};

/// Seconds between waves.
const WAVE_INTERVAL: f32 = 8.0;
/// Enemies in the first wave.
const BASE_COUNT: u32 = 3;
/// Extra enemies added by each later wave.
const PER_WAVE: u32 = 2;
/// Half the side of the square enemies scatter in.
const HALF_EXTENT: f32 = 6.0;
const PREFIX: &str = "wave_enemy";

#[derive(Component)]
struct Waves {
    wave: u32,
    countdown: f32,
    /// Scatter randomness; there is no rand crate, so a small LCG.
    rng: u64,
}

fn update(ctx: &mut ScriptCtx) {
    let dt = ctx.delta();
    let Some(mut waves) = ctx.get_mut::<Waves>() else { return };
    waves.countdown -= dt;
    if waves.countdown > 0.0 {
        return;
    }
    waves.countdown += WAVE_INTERVAL;
    drop(waves);
    spawn_wave(ctx);
}

fn hooks(ctx: &mut ScriptCtx, hook: &ScriptHook) {
    if let ScriptHook::Ready = hook {
        let seed = ctx.entity().to_bits() ^ u64::from(ctx.elapsed().to_bits());
        ctx.insert(Waves { wave: 0, countdown: WAVE_INTERVAL, rng: seed });
        // Kick off at once rather than waiting a full interval.
        spawn_wave(ctx);
    }
}

fn spawn_wave(ctx: &mut ScriptCtx) {
    let Some(origin) = ctx.get::<Transform>().map(|t| t.translation) else { return };
    let Some(mut waves) = ctx.get_mut::<Waves>() else { return };
    waves.wave += 1;
    let wave = waves.wave;
    let count = BASE_COUNT + (wave - 1) * PER_WAVE;
    let spots: Vec<Vec3> = (0..count)
        .map(|_| {
            let x = (next_f32(&mut waves.rng) * 2.0 - 1.0) * HALF_EXTENT;
            let z = (next_f32(&mut waves.rng) * 2.0 - 1.0) * HALF_EXTENT;
            origin + Vec3::new(x, 0.5, z)
        })
        .collect();
    drop(waves);

    let me = ctx.entity();
    let world = ctx.world();
    for pos in spots {
        world.spawn((
            Name::new(PREFIX),
            Transform::from_translation(pos),
            Visibility::default(),
            MeshPrimitive("cube".into()),
            MeshColor(Color::srgb(0.8, 0.2, 0.2)),
        ));
    }
    ScriptAction::new("play_sound", me)
        .arg("path", "wave_start")
        .arg("volume", 1.0)
        .arg("looping", false)
        .arg("bus", "Sfx")
        .send(world);
    renzora::clog_info!("Script", "wave {wave} -> {count} enemies");
}

/// A uniform value in 0..1 from a 64-bit LCG.
fn next_f32(state: &mut u64) -> f32 {
    *state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
    (*state >> 40) as f32 / (1u64 << 24) as f32
}

renzora::script!(update, hooks = hooks);
