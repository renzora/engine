//! A tower-defense wave director: a state machine spawning escalating enemy
//! waves. Pacing needs a timer-driven FSM (intermission, spawning, clearing)
//! that the engine does not orchestrate, so the director counts the enemies it
//! spawned against the kills reported to it and only advances when the field
//! is clear, raising enemy count and spawn rate each round.
//!
//! Spawning is server-authoritative: a pure client never starts a wave.
//!
//! Setup: attach to a manager near the path origin. A UI text named
//! `td_status` shows the wave state. Enemies are spawned as `enemy_<n>`; each
//! one's own script reports its death with an `enemy_down` RPC.

use bevy::prelude::*;
use renzora::core::{MeshColor, MeshPrimitive};
use renzora::network as net;
use renzora::script_bridge::ScriptAction;
use renzora::{ScriptCtx, ScriptHook};

/// Seconds between a cleared wave and the next.
const BREAK: f32 = 5.0;

#[derive(PartialEq, Clone, Copy)]
enum Phase {
    Intermission,
    Spawning,
    Clearing,
}

#[derive(Component)]
struct Director {
    phase: Phase,
    wave: i32,
    to_spawn: i32,
    alive: i32,
    spawn_id: u32,
    /// Seconds left in the intermission, while in it.
    break_left: Option<f32>,
    /// Seconds to the next spawn, and the spawn interval, while spawning.
    spawn_left: f32,
    spawn_every: f32,
    seed: u64,
}

fn update(ctx: &mut ScriptCtx) {
    let dt = ctx.delta();
    let Some(mut d) = ctx.get_mut::<Director>() else { return };

    if let Some(left) = d.break_left.as_mut() {
        *left -= dt;
        if *left <= 0.0 {
            d.break_left = None;
            drop(d);
            start_wave(ctx);
            return;
        }
    }

    let mut spawn_x = None;
    if d.phase == Phase::Spawning {
        d.spawn_left -= dt;
        if d.spawn_left <= 0.0 {
            d.spawn_left += d.spawn_every;
            if d.to_spawn > 0 {
                d.spawn_id += 1;
                d.to_spawn -= 1;
                d.alive += 1;
                spawn_x = Some((d.spawn_id, rand01(&mut d.seed) * 4.0 - 2.0));
            } else {
                d.phase = Phase::Clearing;
            }
        }
    }
    let cleared = d.phase == Phase::Clearing && d.alive <= 0;
    drop(d);

    if let Some((id, x)) = spawn_x {
        ctx.world().spawn((
            Name::new(format!("enemy_{id}")),
            Transform::from_xyz(x, 1.0, -10.0),
            Visibility::default(),
            MeshPrimitive("capsule".into()),
            MeshColor(Color::srgb(0.9, 0.3, 0.3)),
        ));
    }
    if cleared {
        begin_intermission(ctx);
    }
}

fn hooks(ctx: &mut ScriptCtx, hook: &ScriptHook) {
    match hook {
        ScriptHook::Ready => {
            let seed = ctx.entity().to_bits() ^ ((ctx.elapsed().to_bits() as u64) << 32);
            let world = ctx.world();
            let pure_client = net::is_client(world) && !net::is_server(world);
            if pure_client {
                return;
            }
            ctx.insert(Director {
                phase: Phase::Intermission,
                wave: 0,
                to_spawn: 0,
                alive: 0,
                spawn_id: 0,
                break_left: None,
                spawn_left: 0.0,
                spawn_every: 1.0,
                seed,
            });
            start_wave(ctx);
        }
        ScriptHook::Rpc { name: "enemy_down", .. } => {
            if let Some(mut d) = ctx.get_mut::<Director>() {
                d.alive = (d.alive - 1).max(0);
            }
        }
        _ => {}
    }
}

fn start_wave(ctx: &mut ScriptCtx) {
    let Some(mut d) = ctx.get_mut::<Director>() else { return };
    d.wave += 1;
    d.to_spawn = 4 + d.wave * 2;
    d.phase = Phase::Spawning;
    d.spawn_every = (1.2 - d.wave as f32 * 0.05).max(0.3);
    d.spawn_left = d.spawn_every;
    let text = format!("Wave {} incoming", d.wave);
    drop(d);
    status(ctx, text);
}

fn begin_intermission(ctx: &mut ScriptCtx) {
    let Some(mut d) = ctx.get_mut::<Director>() else { return };
    d.phase = Phase::Intermission;
    d.break_left = Some(BREAK);
    let text = format!("Wave {} cleared!", d.wave);
    drop(d);
    status(ctx, text);
}

fn status(ctx: &mut ScriptCtx, text: String) {
    let me = ctx.entity();
    ScriptAction::new("ui_set_text", me).arg("name", "td_status").arg("text", text).send(ctx.world());
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
