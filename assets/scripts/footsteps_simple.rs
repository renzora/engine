//! Plays a footstep each time the entity has covered a stride's worth of
//! ground, so the step rate scales with speed instead of with frame rate.
//!
//! Setup: attach to a moving entity (pair it with a movement script) and point
//! `STEP_SOUND` at a real audio file.

use bevy::prelude::*;
use renzora::script_bridge::ScriptAction;
use renzora::ScriptCtx;

const STEP_SOUND: &str = "audio/footstep.ogg";
const STEP_VOLUME: f32 = 0.6;
/// Distance travelled per step.
const STRIDE_LENGTH: f32 = 1.5;

#[derive(Component)]
struct Footsteps {
    /// Position on the XZ plane last frame.
    last: Vec2,
    /// Distance walked since the last step.
    accum: f32,
}

fn update(ctx: &mut ScriptCtx) {
    let Some(pos) = ctx.get::<Transform>().map(|t| Vec2::new(t.translation.x, t.translation.z)) else {
        return;
    };
    if !ctx.has::<Footsteps>() {
        ctx.insert(Footsteps { last: pos, accum: 0.0 });
        return;
    }
    let Some(mut steps) = ctx.get_mut::<Footsteps>() else { return };
    steps.accum += pos.distance(steps.last);
    steps.last = pos;
    if steps.accum < STRIDE_LENGTH {
        return;
    }
    steps.accum = 0.0;
    drop(steps);

    let me = ctx.entity();
    ScriptAction::new("play_sound", me)
        .arg("path", STEP_SOUND)
        .arg("volume", STEP_VOLUME)
        .arg("looping", false)
        .arg("bus", "Sfx")
        .send(ctx.world());
}

renzora::script!(update);
