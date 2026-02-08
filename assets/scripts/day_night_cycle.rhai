// Day/Night Cycle
//
// Drop this script onto any entity to simulate a day/night cycle.
// Controls sun position, color, energy, sky colors, ambient light, and exposure.
//
// Configure cycle_duration and start_hour in the Inspector.

fn props() {
    #{
        cycle_duration: #{ type: "float", value: 120.0, hint: "Full day cycle in seconds" },
        start_hour: #{ type: "float", value: 12.0, hint: "Starting hour (0=midnight, 12=noon)" },
    }
}

fn on_update() {
    // Time of day: 0.0 = midnight, 0.25 = 6am, 0.5 = noon, 0.75 = 6pm
    let time_of_day = (start_hour / 24.0 + elapsed / cycle_duration) % 1.0;

    // --- Sun position ---
    let sun_elevation = sin((time_of_day - 0.25) * tau()) * 90.0;
    let sun_azimuth = time_of_day * 360.0;
    set_sun_angles(sun_azimuth, sun_elevation);

    // --- Sun energy ---
    let sun_factor = max(sin(deg_to_rad(max(sun_elevation, 0.0))), 0.0);
    set_sun_energy(sun_factor);

    // --- Sun color ---
    let warmth = 1.0 - smoothstep(0.0, 30.0, sun_elevation);
    set_sun_color(1.0, lerp(0.95, 0.55, warmth), lerp(0.85, 0.25, warmth));

    // --- Sky colors ---
    let day_t = smoothstep(-5.0, 15.0, sun_elevation);
    let sunset_t = smoothstep(-5.0, 5.0, sun_elevation) * (1.0 - smoothstep(5.0, 20.0, sun_elevation));

    set_sky_top_color(
        lerp(0.01, 0.15, day_t),
        lerp(0.01, 0.35, day_t),
        lerp(0.06, 0.65, day_t),
    );

    set_sky_horizon_color(
        clamp(lerp(0.04, 0.55, day_t) + sunset_t * 0.40, 0.0, 1.0),
        clamp(lerp(0.04, 0.70, day_t) + sunset_t * 0.10, 0.0, 1.0),
        clamp(lerp(0.06, 0.85, day_t) - sunset_t * 0.25, 0.0, 1.0),
    );

    // --- Ambient light ---
    set_ambient_brightness(lerp(40.0, 400.0, day_t));
    set_ambient_color(
        lerp(0.3, 1.0, day_t),
        lerp(0.3, 1.0, day_t),
        lerp(0.5, 1.0, day_t),
    );

    // --- Exposure ---
    set_ev100(lerp(6.0, 9.7, day_t));
}
