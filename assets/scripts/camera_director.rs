//! A cinematic camera director: cut through named camera presets on a
//! timeline, like a cutscene sequencer. A cutscene is just a scheduled list of
//! (preset, dwell) pairs; presets exist but nothing sequences them, so the
//! director owns the shot index and a dwell timer, advancing when it runs out
//! and looping. Space skips the current shot.
//!
//! The view is local and not networked.
//!
//! Setup: attach to the camera, and capture presets in its Camera Presets
//! section named as in `SHOTS`.

use bevy::prelude::*;
use renzora::core::CameraPresets;
use renzora::{ScriptCtx, ScriptHook, ScriptInput};

/// Each shot's preset name and how long it holds, in seconds.
const SHOTS: [(&str, f32); 4] = [
    ("establishing", 4.0),
    ("closeup", 2.5),
    ("over_shoulder", 3.0),
    ("wide_orbit", 5.0),
];

#[derive(Component)]
struct Director {
    shot: usize,
    dwell: f32,
}

fn update(ctx: &mut ScriptCtx) {
    let dt = ctx.delta();
    let skip = ctx.get_resource::<ScriptInput>().is_some_and(|i| i.is_key_just_pressed(KeyCode::Space));
    let Some(mut d) = ctx.get_mut::<Director>() else { return };
    d.dwell -= dt;
    if d.dwell > 0.0 && !skip {
        return;
    }
    let next = (d.shot + 1) % SHOTS.len();
    drop(d);
    cut_to(ctx, next);
}

fn hooks(ctx: &mut ScriptCtx, hook: &ScriptHook) {
    if matches!(hook, ScriptHook::Ready) {
        ctx.insert(Director { shot: 0, dwell: 0.0 });
        cut_to(ctx, 0);
    }
}

fn cut_to(ctx: &mut ScriptCtx, shot: usize) {
    let (preset, dwell) = SHOTS[shot];
    if let Some(mut d) = ctx.get_mut::<Director>() {
        d.shot = shot;
        d.dwell = dwell;
    }
    let Some(pose) = ctx.get::<CameraPresets>().and_then(|p| p.get(preset)).map(|p| p.to_transform()) else {
        renzora::clog_warn!("Script", "[director] no camera preset named {}", preset);
        return;
    };
    if let Some(mut t) = ctx.get_mut::<Transform>() {
        t.translation = pose.translation;
        t.rotation = pose.rotation;
    }
    renzora::clog_info!("Script", "[director] shot {} -> {}", shot + 1, preset);
}

renzora::script!(update, hooks = hooks);
