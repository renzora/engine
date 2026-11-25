//! A single director that advances a 24-hour clock and drives the sun angle,
//! fog and music together so the whole scene stays in phase. Everything is
//! derived from one normalized time of day (0..1), so there is one source of
//! truth to tune.
//!
//! Fog is written into the scene's `WorldEnvironment` (exponential falloff),
//! which the fog plugin then applies to the cameras; the Lua `set_fog` call
//! did nothing. The sun angles are also sent the right way round now: the Lua
//! original passed elevation as azimuth and vice versa.
//!
//! Setup: attach to an empty director entity. The scene has a directional sun
//! and a World Environment.

use std::f32::consts::{FRAC_PI_2, TAU};

use bevy::prelude::*;
use renzora::script_bridge::ScriptAction;
use renzora::{ScriptCtx, WorldEnvironment};

/// Real seconds for a full day.
const DAY_LENGTH: f32 = 120.0;
/// Starting time of day; 0.25 is sunrise.
const START_TIME: f32 = 0.25;
/// `FogSection::mode` for an exponential falloff.
const FOG_EXPONENTIAL: u32 = 1;

#[derive(Component)]
struct DayClock {
    time_of_day: f32,
    phase: &'static str,
}

fn update(ctx: &mut ScriptCtx) {
    let dt = ctx.delta();
    if !ctx.has::<DayClock>() {
        ctx.insert(DayClock { time_of_day: START_TIME, phase: "" });
    }
    let Some(mut clock) = ctx.get_mut::<DayClock>() else { return };
    clock.time_of_day = (clock.time_of_day + dt / DAY_LENGTH).rem_euclid(1.0);
    let t = clock.time_of_day;

    // A sine that peaks at noon (t = 0.5) and is below the horizon at night.
    let elevation = (t * TAU - FRAC_PI_2).sin() * 80.0;
    let azimuth = t * 360.0;
    let daylight = ((elevation + 10.0) / 90.0).clamp(0.0, 1.0);
    let phase = if daylight > 0.6 {
        "day"
    } else if daylight > 0.15 {
        "dusk"
    } else {
        "night"
    };
    // The music bed swaps on a phase change only, not every frame.
    let phase_changed = phase != clock.phase;
    clock.phase = phase;
    drop(clock);

    let me = ctx.entity();
    ScriptAction::new("set_sun_angles", me)
        .arg("azimuth", azimuth)
        .arg("elevation", elevation)
        .send(ctx.world());

    // Thicker and darker at night.
    let density = lerp(0.02, 0.006, daylight);
    let color = [lerp(0.05, 0.6, daylight), lerp(0.06, 0.7, daylight), lerp(0.1, 0.85, daylight)];
    let world = ctx.world();
    let mut envs = world.query::<&mut WorldEnvironment>();
    for mut env in envs.iter_mut(world) {
        env.fog.enabled = true;
        env.fog.mode = FOG_EXPONENTIAL;
        env.fog.color = color;
        env.fog.density = density;
    }

    if phase_changed {
        ScriptAction::new("play_music", me)
            .arg("path", format!("ambient_{phase}"))
            .arg("volume", 1.0)
            .arg("fade_in", 0.0)
            .arg("bus", "Music")
            .send(ctx.world());
    }
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

renzora::script!(update);
