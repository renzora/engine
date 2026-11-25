//! Idle, walk and run locomotion by crossfade, without a state-machine asset.
//! The clip is picked from input-magnitude thresholds but only crossfaded on
//! a change, because restarting it every frame would freeze it on frame 0.
//! Playback speed then follows the input, a cheap foot-sync.
//!
//! Setup: attach to a character with "idle", "walk" and "run" clips.

use bevy::prelude::*;
use renzora::script_bridge::ScriptAction;
use renzora::{ScriptCtx, ScriptInput};

const WALK_THRESHOLD: f32 = 0.15;
const RUN_THRESHOLD: f32 = 0.75;
/// Crossfade seconds.
const FADE: f32 = 0.25;

#[derive(Component)]
struct Locomotion {
    current: &'static str,
}

fn update(ctx: &mut ScriptCtx) {
    let me = ctx.entity();
    if !ctx.has::<Locomotion>() {
        ctx.insert(Locomotion { current: "" });
    }
    let mag = ctx
        .get_resource::<ScriptInput>()
        .map(|input| input.get_movement_vector().length().min(1.0))
        .unwrap_or(0.0);
    let (want, speed) = if mag < WALK_THRESHOLD {
        ("idle", None)
    } else if mag < RUN_THRESHOLD {
        ("walk", Some(0.6 + mag * 0.8))
    } else {
        ("run", Some(0.9 + mag * 0.6))
    };

    let Some(mut loco) = ctx.get_mut::<Locomotion>() else { return };
    let changed = loco.current != want;
    loco.current = want;
    drop(loco);

    let world = ctx.world();
    if changed {
        ScriptAction::new("crossfade_animation", me)
            .arg("name", want)
            .arg("duration", FADE)
            .arg("looping", true)
            .send(world);
    }
    if let Some(speed) = speed {
        ScriptAction::new("set_animation_speed", me).arg("speed", speed).send(world);
    }
}

renzora::script!(update);
