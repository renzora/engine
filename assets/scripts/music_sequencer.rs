//! A generative step sequencer: a tempo clock walks per-track step masks and
//! fires one-shot sounds, building a groove without a music engine. The hat
//! pattern mutates once a bar (a probabilistic fill), and the hat itself only
//! plays with some chance, so the loop never repeats exactly.
//!
//! The audio is local; gate it on the network role if peers should not each
//! play their own.
//!
//! Setup: attach to a manager entity and provide the sounds named in `TRACKS`.

use bevy::prelude::*;
use renzora::script_bridge::ScriptAction;
use renzora::ScriptCtx;

const BPM: f32 = 120.0;
/// Steps per bar.
const STEPS: u32 = 16;
/// Seconds per step: sixteenth notes.
const INTERVAL: f32 = 60.0 / BPM / 4.0;
/// Each track's sound, starting 8-step mask, and chance a set step plays.
const TRACKS: [(&str, [bool; 8], f32); 3] = [
    ("sfx/kick.ogg", [true, false, false, false, true, false, false, false], 1.0),
    ("sfx/snare.ogg", [false, false, true, false, false, false, true, false], 1.0),
    ("sfx/hat.ogg", [true; 8], 0.8),
];
/// The track whose pattern mutates each bar.
const MUTATING: usize = 2;

#[derive(Component)]
struct Sequencer {
    masks: [[bool; 8]; 3],
    step: u32,
    timer: f32,
    seed: u64,
}

fn update(ctx: &mut ScriptCtx) {
    let dt = ctx.delta();
    if !ctx.has::<Sequencer>() {
        let seed = ctx.entity().to_bits() ^ ((ctx.elapsed().to_bits() as u64) << 32);
        ctx.insert(Sequencer { masks: TRACKS.map(|t| t.1), step: 0, timer: INTERVAL, seed });
    }
    let Some(mut seq) = ctx.get_mut::<Sequencer>() else { return };
    seq.timer -= dt;
    if seq.timer > 0.0 {
        return;
    }
    seq.timer += INTERVAL;

    let step = seq.step;
    let mut hits = Vec::new();
    for (i, (sound, _, chance)) in TRACKS.iter().enumerate() {
        let on = seq.masks[i][step as usize % 8];
        if on && rand01(&mut seq.seed) <= *chance {
            hits.push(*sound);
        }
    }
    if step % STEPS == 0 {
        let flip = (rand01(&mut seq.seed) * 8.0) as usize % 8;
        seq.masks[MUTATING][flip] = !seq.masks[MUTATING][flip];
    }
    seq.step = (step + 1) % STEPS;
    drop(seq);

    let me = ctx.entity();
    for sound in hits {
        ScriptAction::new("play_sound", me)
            .arg("path", sound)
            .arg("volume", 1.0)
            .arg("looping", false)
            .arg("bus", "Sfx")
            .send(ctx.world());
    }
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

renzora::script!(update);
