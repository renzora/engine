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
    let duration = vars["cycle_duration"];
    let start_hour = vars["start_hour"];

    // Time of day: 0.0 = midnight, 0.25 = 6am, 0.5 = noon, 0.75 = 6pm
    let time_of_day = (start_hour / 24.0 + elapsed / duration) % 1.0;

    // --- Sun position ---
    // Elevation: -90 at midnight, 0 at dawn/dusk, 90 at noon
    let sun_elevation = sin((time_of_day - 0.25) * tau()) * 90.0;
    // Azimuth: east (90) at dawn, south (180) at noon, west (270) at dusk
    let sun_azimuth = time_of_day * 360.0;

    _set_sun_angles = true;
    _sun_azimuth = sun_azimuth;
    _sun_elevation = sun_elevation;

    // --- Sun energy ---
    // Zero below horizon, ramps up with elevation
    let sun_factor = max(sin(deg_to_rad(max(sun_elevation, 0.0))), 0.0);
    _set_sun_energy = true;
    _sun_energy = sun_factor;

    // --- Sun color ---
    // Warm orange near horizon, white-ish when high
    let warmth = 1.0 - smoothstep(0.0, 30.0, sun_elevation);
    _set_sun_color = true;
    _sun_color_r = 1.0;
    _sun_color_g = lerp(0.95, 0.55, warmth);
    _sun_color_b = lerp(0.85, 0.25, warmth);

    // --- Sky colors ---
    // day_t: 0 at night, 1 during full day
    let day_t = smoothstep(-5.0, 15.0, sun_elevation);
    // sunset_t: peaks when sun is near the horizon
    let sunset_t = smoothstep(-5.0, 5.0, sun_elevation) * (1.0 - smoothstep(5.0, 20.0, sun_elevation));

    // Sky top: dark navy at night, bright blue during day
    _set_sky_top_color = true;
    _sky_top_r = lerp(0.01, 0.15, day_t);
    _sky_top_g = lerp(0.01, 0.35, day_t);
    _sky_top_b = lerp(0.06, 0.65, day_t);

    // Sky horizon: dark at night, light blue during day, orange tint at sunrise/sunset
    _set_sky_horizon_color = true;
    _sky_horizon_r = clamp(lerp(0.04, 0.55, day_t) + sunset_t * 0.40, 0.0, 1.0);
    _sky_horizon_g = clamp(lerp(0.04, 0.70, day_t) + sunset_t * 0.10, 0.0, 1.0);
    _sky_horizon_b = clamp(lerp(0.06, 0.85, day_t) - sunset_t * 0.25, 0.0, 1.0);

    // --- Ambient light ---
    _set_ambient_brightness = true;
    _ambient_brightness = lerp(40.0, 400.0, day_t);

    _set_ambient_color = true;
    _ambient_color_r = lerp(0.3, 1.0, day_t);
    _ambient_color_g = lerp(0.3, 1.0, day_t);
    _ambient_color_b = lerp(0.5, 1.0, day_t);

    // --- Exposure ---
    _set_ev100 = true;
    _ev100 = lerp(6.0, 9.7, day_t);
}
