//! A world orchestrator: one day/night clock drives the sun angle, the fog and
//! the music, and the canonical time is broadcast so every peer shares one sky.
//! Ambience is several coupled systems keyed off one clock, and centralising it
//! avoids drift. The server owns the time and sends it out; a small weather FSM
//! thickens the fog for storms and changes the music on transitions.
//!
//! Fog is written straight onto every 3D camera's `DistanceFog`, since the Lua
//! `set_fog` it replaces never did anything.
//!
//! Setup: attach to a single world manager entity. In multiplayer the server
//! broadcasts `world_time`; clients render it and never advance the clock.

use bevy::prelude::*;
use renzora::network::{self as net, Rpc, RpcArgs};
use renzora::script_bridge::ScriptAction;
use renzora::{ScriptCtx, ScriptHook};

/// Seconds per full day.
const DAY_LEN: f32 = 120.0;
/// Hour the clock starts at (0..24).
const START_HOUR: f32 = 6.0;
/// Seconds between weather rolls.
const WEATHER_EVERY: f32 = 20.0;
/// Chance a roll brings a storm.
const STORM_CHANCE: f32 = 0.3;
const SUN_AZIMUTH: f32 = 45.0;
const FOG_COLOR: Color = Color::srgb(0.5, 0.55, 0.65);

#[derive(Component)]
struct Sky {
    time_of_day: f32,
    storm: bool,
    weather_timer: f32,
    last_track: &'static str,
    /// The fog density last written, so cameras are only touched on change.
    last_density: f32,
    seed: u64,
}

fn update(ctx: &mut ScriptCtx) {
    let dt = ctx.delta();
    let world = ctx.world();
    if net::is_client(world) && !net::is_server(world) {
        return;
    }
    let connected = net::is_connected(world);
    let Some(mut sky) = ctx.get_mut::<Sky>() else { return };
    sky.time_of_day = (sky.time_of_day + (24.0 / DAY_LEN) * dt) % 24.0;
    sky.weather_timer += dt;
    let mut storm_began = false;
    if sky.weather_timer > WEATHER_EVERY {
        sky.weather_timer = 0.0;
        sky.storm = rand01(&mut sky.seed) < STORM_CHANCE;
        storm_began = sky.storm;
    }
    let (t, storm) = (sky.time_of_day, sky.storm);
    drop(sky);

    if storm_began {
        let me = ctx.entity();
        ScriptAction::new("screen_shake", me).arg("intensity", 0.3).arg("duration", 1.0).send(ctx.world());
    }
    apply_sky(ctx);
    // Sent every frame, so a lost packet is simply superseded by the next.
    if connected {
        Rpc::new("world_time")
            .arg("t", t)
            .arg("w", if storm { "storm" } else { "clear" })
            .unreliable()
            .send(ctx.world());
    }
}

fn hooks(ctx: &mut ScriptCtx, hook: &ScriptHook) {
    match hook {
        ScriptHook::Ready => {
            let seed = ctx.entity().to_bits() ^ ((ctx.elapsed().to_bits() as u64) << 32);
            ctx.insert(Sky {
                time_of_day: START_HOUR,
                storm: false,
                weather_timer: 0.0,
                last_track: "",
                last_density: -1.0,
                seed,
            });
            let world = ctx.world();
            if net::is_server(world) || !net::is_connected(world) {
                apply_sky(ctx);
            }
        }
        ScriptHook::Rpc { name: "world_time", args, .. } => {
            if !net::is_client(ctx.world()) {
                return;
            }
            let Some(t) = args.f32("t") else { return };
            let storm = args.str("w") == Some("storm");
            if let Some(mut sky) = ctx.get_mut::<Sky>() {
                sky.time_of_day = t;
                sky.storm = storm;
            }
            apply_sky(ctx);
        }
        _ => {}
    }
}

fn apply_sky(ctx: &mut ScriptCtx) {
    let me = ctx.entity();
    let Some(mut sky) = ctx.get_mut::<Sky>() else { return };
    let t = sky.time_of_day;
    let night = !(6.0..=19.0).contains(&t);
    let density = if night { 0.06 } else { 0.01 } + if sky.storm { 0.08 } else { 0.0 };
    let track = if sky.storm {
        "music/storm.ogg"
    } else if night {
        "music/night.ogg"
    } else {
        "music/day.ogg"
    };
    let new_track = track != sky.last_track;
    sky.last_track = track;
    let fog_changed = density != sky.last_density;
    sky.last_density = density;
    drop(sky);

    // Elevation runs -90 at midnight to +90 at noon. The Lua version passed it
    // as the azimuth; it is the elevation.
    let elevation = ((t / 24.0) * std::f32::consts::TAU - std::f32::consts::FRAC_PI_2).sin() * 90.0;
    ScriptAction::new("set_sun_angles", me)
        .arg("azimuth", SUN_AZIMUTH)
        .arg("elevation", elevation)
        .send(ctx.world());

    if fog_changed {
        set_fog(ctx.world(), density);
    }
    if new_track {
        ScriptAction::new("play_music", me)
            .arg("path", track)
            .arg("volume", 1.0)
            .arg("fade_in", 0.0)
            .arg("bus", "Music")
            .send(ctx.world());
    }
}

fn set_fog(world: &mut World, density: f32) {
    let mut cameras = world.query_filtered::<Entity, With<Camera3d>>();
    let cameras: Vec<Entity> = cameras.iter(world).collect();
    for camera in cameras {
        let fog = DistanceFog {
            color: FOG_COLOR,
            falloff: FogFalloff::Exponential { density },
            ..default()
        };
        world.entity_mut(camera).insert(fog);
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

renzora::script!(update, hooks = hooks);
