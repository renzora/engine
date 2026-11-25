//! A realistic day/night cycle. It drives the scene's `Sun` (azimuth,
//! elevation, colour, illuminance) from a solar-position model, so the sun
//! rises in the east, arcs across the sky at an angle set by latitude and the
//! day of the year, and sets in the west. Colour warms toward orange near the
//! horizon and cools to white overhead; illuminance tracks how high it is.
//!
//! The engine does the rest: below the horizon it fades the sun to black and
//! disables its shadows, the procedural sky reddens at the horizon and darkens
//! at night, and the night stars fade in. So this script only moves and tints
//! the sun.
//!
//! A full 24 hours takes `DAY_LENGTH` real seconds. Set `PAUSED` to freeze the
//! clock at `START_HOUR` for lighting setup.
//!
//! Setup: attach to any entity; the `Sun` is on the entity named
//! "world_environment" (`SUN_NAME`).

use bevy::prelude::*;
use renzora::{ScriptCtx, Sun};

/// The entity holding the `Sun` component.
const SUN_NAME: &str = "world_environment";

/// Real seconds for one full 24 hour cycle.
const DAY_LENGTH: f32 = 120.0;
/// Time of day to start at, 0 to 24.
const START_HOUR: f32 = 8.0;
const PAUSED: bool = false;

/// Observer latitude in degrees, -90 to 90.
const LATITUDE: f32 = 40.0;
/// Day of the year to start on, 1 to 365 (172 is about the June solstice).
const START_DAY_OF_YEAR: f32 = 172.0;
/// Advance the day each cycle, for a slow seasonal drift.
const ADVANCE_DAYS: bool = true;

/// Midday sun illuminance in lux, scaled down as the sun lowers.
const PEAK_ILLUMINANCE: f32 = 75000.0;

/// Below this elevation, in degrees, the colour warms toward the horizon tint.
const WARM_BELOW: f32 = 18.0;
const NOON_COLOR: Vec3 = Vec3::new(1.0, 0.96, 0.90);
const DAWN_COLOR: Vec3 = Vec3::new(1.0, 0.52, 0.28);

/// Distance fog on the 3D cameras that closes in when the sun is low.
const FOG: bool = false;
const FOG_START: f32 = 8.0;
const FOG_FAR_DAY: f32 = 320.0;
const FOG_FAR_NIGHT: f32 = 60.0;

#[derive(Component)]
struct Clock {
    /// Hours, 0 to 24.
    time: f32,
    day_of_year: f32,
}

fn update(ctx: &mut ScriptCtx) {
    let dt = ctx.delta();
    if !ctx.has::<Clock>() {
        ctx.insert(Clock { time: START_HOUR, day_of_year: START_DAY_OF_YEAR });
    }
    let Some(mut clock) = ctx.get_mut::<Clock>() else { return };
    if !PAUSED && DAY_LENGTH > 0.0 {
        clock.time += 24.0 / DAY_LENGTH * dt;
        while clock.time >= 24.0 {
            clock.time -= 24.0;
            if ADVANCE_DAYS {
                clock.day_of_year += 1.0;
                if clock.day_of_year > 365.0 {
                    clock.day_of_year -= 365.0;
                }
            }
        }
    }
    let (time, day_of_year) = (clock.time, clock.day_of_year);
    drop(clock);

    // Solar declination: the sun's tilt for this day of the year.
    let decl = 23.45 * (360.0f32 / 365.0 * (day_of_year - 81.0)).to_radians().sin();
    // Hour angle: 15 degrees per hour away from solar noon (negative is morning).
    let hour_angle = 15.0 * (time - 12.0);

    let lat_r = LATITUDE.to_radians();
    let decl_r = decl.to_radians();
    let ha_r = hour_angle.to_radians();

    let sin_elev = (lat_r.sin() * decl_r.sin() + lat_r.cos() * decl_r.cos() * ha_r.cos()).clamp(-1.0, 1.0);
    let elev = sin_elev.asin().to_degrees();

    // Azimuth clockwise from north (N 0, E 90, S 180, W 270), the `Sun`
    // component's convention.
    let denom = elev.to_radians().cos() * lat_r.cos();
    let az = if denom.abs() < 1e-4 {
        180.0
    } else {
        let cos_az = (decl_r.sin() - lat_r.sin() * sin_elev) / denom;
        let az = cos_az.clamp(-1.0, 1.0).acos().to_degrees();
        if hour_angle > 0.0 {
            360.0 - az
        } else {
            az
        }
    };

    // 0 at or below the horizon, 1 once the sun is above `WARM_BELOW`.
    let warm = (elev / WARM_BELOW.max(0.001)).clamp(0.0, 1.0);
    let color = DAWN_COLOR.lerp(NOON_COLOR, warm);
    let illuminance = PEAK_ILLUMINANCE * sin_elev.clamp(0.0, 1.0);

    if let Some(sun_entity) = find_named(ctx.world(), SUN_NAME) {
        if let Some(mut sun) = ctx.get_mut_on::<Sun>(sun_entity) {
            sun.azimuth = az;
            sun.elevation = elev;
            sun.color = color;
            sun.illuminance = illuminance;
        }
    }

    if FOG {
        let far = FOG_FAR_NIGHT + (FOG_FAR_DAY - FOG_FAR_NIGHT) * (elev / 10.0).clamp(0.0, 1.0);
        set_fog(ctx.world(), FOG_START, far);
    }
}

/// Linear distance fog on every 3D camera, keeping any colour already set.
fn set_fog(world: &mut World, start: f32, end: f32) {
    let mut cameras = world.query_filtered::<Entity, With<Camera3d>>();
    let cameras: Vec<Entity> = cameras.iter(world).collect();
    for camera in cameras {
        let falloff = FogFalloff::Linear { start, end };
        if let Some(mut fog) = world.get_mut::<DistanceFog>(camera) {
            fog.falloff = falloff;
        } else {
            world.entity_mut(camera).insert(DistanceFog { falloff, ..default() });
        }
    }
}

/// The entity with this name (its snake_case id), if there is one.
fn find_named(world: &mut World, name: &str) -> Option<Entity> {
    let mut names = world.query::<(Entity, &Name)>();
    names.iter(world).find(|(_, n)| n.as_str() == name).map(|(e, _)| e)
}

renzora::script!(update);
