//! Fires footstep sounds as the character moves, spaced by distance travelled
//! so the cadence tracks how fast it is going. Steps are silenced while
//! airborne by reading the physics ground state.
//!
//! This script only decides *when* a step happens; the sounds live on an
//! Audio Player component, and each step fires it, which picks a random clip
//! from its pool with per-shot pitch and volume jitter.
//!
//! "Moving" is gated on a *smoothed* speed, not raw per-frame distance: the
//! smoothing rides over the physics-vs-render frame-rate mismatch (no
//! stutter), and `MIN_SPEED` ignores the tiny drift the physics solver
//! produces while the character settles after stopping (no phantom steps).
//! `MIN_INTERVAL` caps the rate so clips never pile up.
//!
//! Setup: attach to the character entity, and give that same entity an Audio
//! Player component with a Clip Pool of footstep sounds. With
//! `REQUIRE_GROUNDED` the character also needs a physics body.

use bevy::prelude::*;
use renzora::core::reflection::get_reflected_field;
use renzora::script_bridge::{PropertyValue, ScriptAction};
use renzora::ScriptCtx;

/// World units travelled between footsteps; smaller is a faster cadence.
const STEP_DISTANCE: f32 = 1.42;
/// Minimum seconds between footsteps; caps the fastest cadence (sprinting).
const MIN_INTERVAL: f32 = 0.38;
/// No steps below this speed (units per second); also rejects settle drift.
const MIN_SPEED: f32 = 1.0;
/// Only step while grounded.
const REQUIRE_GROUNDED: bool = true;

#[derive(Component)]
struct Footsteps {
    /// Last horizontal position, X and Z.
    last: Vec2,
    /// Smoothed horizontal speed.
    speed: f32,
    /// Distance since the last step.
    accum: f32,
    /// Seconds since the last step.
    since: f32,
    was_moving: bool,
}

fn update(ctx: &mut ScriptCtx) {
    let dt = ctx.delta();
    let Some(pos) = ctx.get::<Transform>().map(|t| t.translation.xz()) else { return };
    if !ctx.has::<Footsteps>() {
        ctx.insert(Footsteps { last: pos, speed: 0.0, accum: 0.0, since: 0.0, was_moving: false });
    }
    if dt <= 0.0 {
        return;
    }

    let me = ctx.entity();
    let grounded = !REQUIRE_GROUNDED
        || matches!(
            get_reflected_field(ctx.world(), me, "PhysicsReadState", "grounded"),
            Some(PropertyValue::Bool(true))
        );

    let Some(mut state) = ctx.get_mut::<Footsteps>() else { return };
    // Vertical motion is ignored so gravity or ground-stick never counts as
    // walking.
    let dist = pos.distance(state.last);
    state.last = pos;
    // About a 0.1 s time constant: rides over frames where physics did not
    // tick, and decays to roughly zero quickly once the character stops.
    state.speed += (dist / dt - state.speed) * (dt * 10.0).min(1.0);
    state.since += dt;

    let moving = grounded && state.speed >= MIN_SPEED;
    let mut step = false;
    if moving {
        if !state.was_moving {
            // Just started: step right away so the first one does not lag.
            if state.since >= MIN_INTERVAL {
                step = true;
                state.since = 0.0;
            }
            state.accum = 0.0;
        } else {
            state.accum += dist;
            if state.accum >= STEP_DISTANCE && state.since >= MIN_INTERVAL {
                step = true;
                state.accum = 0.0;
                state.since = 0.0;
            }
        }
    } else {
        state.accum = 0.0;
    }
    state.was_moving = moving;
    drop(state);

    if step {
        ScriptAction::new("play_audio_player", me).send(ctx.world());
    }
}

renzora::script!(update);
