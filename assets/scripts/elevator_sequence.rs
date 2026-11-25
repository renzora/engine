//! A multi-stop elevator. It dwells at each floor for a hold time, then moves
//! Y to the next stop. Modelling it as an explicit phase (moving or waiting)
//! plus a stop index keeps the motion deterministic and loopable.
//!
//! Setup: attach to the platform; list the floor heights in `STOPS`.

use bevy::prelude::*;
use renzora::script_bridge::ScriptAction;
use renzora::ScriptCtx;

/// Floor heights, bottom first. The platform loops back to the first.
const STOPS: [f32; 3] = [0.0, 4.0, 8.0];
/// Travel speed in units per second.
const SPEED: f32 = 2.0;
/// Seconds spent at each floor.
const HOLD: f32 = 2.5;

/// This elevator's progress, kept on the entity between frames.
#[derive(Component)]
struct Elevator {
    next: usize,
    moving: bool,
    /// Seconds left at the current floor while waiting.
    dwell: f32,
}

fn update(ctx: &mut ScriptCtx) {
    let dt = ctx.delta();
    if !ctx.has::<Elevator>() {
        ctx.insert(Elevator { next: 1, moving: false, dwell: HOLD });
    }
    let Some(state) = ctx.get::<Elevator>() else { return };
    let (next, moving, dwell) = (state.next, state.moving, state.dwell);

    if !moving {
        let dwell = dwell - dt;
        let starting = dwell <= 0.0;
        if let Some(mut state) = ctx.get_mut::<Elevator>() {
            state.dwell = dwell;
            state.moving = starting;
        }
        if starting {
            play_sound(ctx, "elevator_start");
        }
        return;
    }

    let target = STOPS[next];
    let step = SPEED * dt;
    let Some(mut transform) = ctx.get_mut::<Transform>() else { return };
    let y = transform.translation.y;
    if (target - y).abs() > step {
        transform.translation.y = y + step * (target - y).signum();
        return;
    }
    transform.translation.y = target;
    if let Some(mut state) = ctx.get_mut::<Elevator>() {
        state.moving = false;
        state.dwell = HOLD;
        state.next = (next + 1) % STOPS.len();
    }
    play_sound(ctx, "elevator_stop");
}

fn play_sound(ctx: &mut ScriptCtx, path: &str) {
    let me = ctx.entity();
    ScriptAction::new("play_sound", me)
        .arg("path", path)
        .arg("volume", 1.0)
        .arg("looping", false)
        .arg("bus", "Sfx")
        .send(ctx.world());
}

renzora::script!(update);
