// Procedural Clouds Shader — Ultra-realistic two-layer sky with spherical projection
// Layer 1: Cumulus (lower, dense, billowy, back-lit) projected via curved shell
// Layer 2: Cirrus (higher, stretched, wispy) for upper-atmosphere streaks
// Compositing uses over-operator so cumulus sits in front of cirrus naturally.

#import bevy_pbr::forward_io::VertexOutput
#import bevy_pbr::mesh_view_bindings::globals

@group(3) @binding(0) var<uniform> params_a: vec4<f32>;  // coverage, density, scale, speed
@group(3) @binding(1) var<uniform> params_b: vec4<f32>;  // wind_dir_x, wind_dir_y, altitude, sun_elevation
@group(3) @binding(2) var<uniform> cloud_color: vec4<f32>;
@group(3) @binding(3) var<uniform> shadow_color: vec4<f32>;
@group(3) @binding(4) var<uniform> params_c: vec4<f32>;  // sun_dir.xyz, absorption
@group(3) @binding(5) var<uniform> params_d: vec4<f32>;  // silver_intensity, silver_spread, powder_strength, ambient_brightness
@group(3) @binding(6) var<uniform> horizon_color: vec4<f32>; // rgb = haze color, a = atmosphere_strength

const PI: f32 = 3.14159265359;

// Virtual atmosphere geometry — scene units, NOT physical km.
// Larger EARTH_R relative to cloud heights gives stronger horizon compression.
const EARTH_R: f32 = 260.0;
const CUMULUS_H: f32 = 2.0;
const CIRRUS_H: f32 = 5.5;

// ── Gradient noise (quintic interpolation) ──

fn hash_grad(p: vec2<f32>) -> vec2<f32> {
    let k = vec2<f32>(
        dot(p, vec2<f32>(127.1, 311.7)),
        dot(p, vec2<f32>(269.5, 183.3)),
    );
    return fract(sin(k) * 43758.5453) * 2.0 - 1.0;
}

fn grad_noise(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * f * (f * (f * 6.0 - 15.0) + 10.0);

    let g00 = dot(hash_grad(i + vec2<f32>(0.0, 0.0)), f - vec2<f32>(0.0, 0.0));
    let g10 = dot(hash_grad(i + vec2<f32>(1.0, 0.0)), f - vec2<f32>(1.0, 0.0));
    let g01 = dot(hash_grad(i + vec2<f32>(0.0, 1.0)), f - vec2<f32>(0.0, 1.0));
    let g11 = dot(hash_grad(i + vec2<f32>(1.0, 1.0)), f - vec2<f32>(1.0, 1.0));

    return mix(mix(g00, g10, u.x), mix(g01, g11, u.x), u.y) * 0.5 + 0.5;
}

fn rot2(a: f32) -> mat2x2<f32> {
    let c = cos(a);
    let s = sin(a);
    return mat2x2<f32>(c, s, -s, c);
}

// ── Normalized FBM (always in [0,1]) ──

fn fbm6(p_in: vec2<f32>) -> f32 {
    var p = p_in;
    var value = 0.0;
    var amplitude = 0.5;
    var total = 0.0;
    let r = rot2(0.8);
    for (var i = 0; i < 6; i = i + 1) {
        value += amplitude * grad_noise(p);
        total += amplitude;
        p = r * p * 2.0 + vec2<f32>(100.0);
        amplitude *= 0.5;
    }
    return value / total;
}

// Ridged FBM — produces cumulus billow shapes
fn fbm_ridged(p_in: vec2<f32>) -> f32 {
    var p = p_in;
    var value = 0.0;
    var amplitude = 0.5;
    var total = 0.0;
    let r = rot2(1.2);
    for (var i = 0; i < 4; i = i + 1) {
        let n = grad_noise(p);
        value += amplitude * (1.0 - abs(n * 2.0 - 1.0));
        total += amplitude;
        p = r * p * 2.0 + vec2<f32>(37.0);
        amplitude *= 0.5;
    }
    return value / total;
}

fn fbm3(p_in: vec2<f32>) -> f32 {
    var p = p_in;
    var value = 0.0;
    var amplitude = 0.5;
    let r = rot2(0.6);
    for (var i = 0; i < 3; i = i + 1) {
        value += amplitude * grad_noise(p);
        p = r * p * 2.0 + vec2<f32>(50.0);
        amplitude *= 0.5;
    }
    return value;
}

// ── Spherical-shell projection ──
// Intersect view ray (from virtual ground at (0, EARTH_R, 0)) with cloud shell
// of radius EARTH_R + h. Returns distance along ray — large near horizon, small near zenith.
fn shell_t(dir: vec3<f32>, h: f32) -> f32 {
    let y = max(dir.y, 0.0);
    let r = EARTH_R + h;
    let b = EARTH_R * y;
    let disc = b * b + r * r - EARTH_R * EARTH_R;
    return -b + sqrt(max(disc, 0.0));
}

fn shell_uv(dir: vec3<f32>, h: f32) -> vec2<f32> {
    let t = shell_t(dir, h);
    return vec2<f32>(dir.x, dir.z) * t;
}

// ── Layer output ──

struct Layer {
    color: vec3<f32>,
    alpha: f32,
}

// ── Cumulus: dense billowy front-layer clouds with Beer/powder/silver lighting ──

fn sample_cumulus(
    dir: vec3<f32>,
    time: f32,
    coverage: f32,
    density: f32,
    scale: f32,
    speed: f32,
    wind_dir: vec2<f32>,
    lit_col: vec3<f32>,
    shad_col: vec3<f32>,
    sun_dir: vec3<f32>,
    absorption: f32,
    silver_i: f32,
    silver_s: f32,
    powder: f32,
    ambient: f32,
) -> Layer {
    var out: Layer;
    out.color = vec3<f32>(0.0);
    out.alpha = 0.0;

    let uv = shell_uv(dir, CUMULUS_H) * scale * 0.22;
    let animated = uv + wind_dir * time * speed;

    // Double-stage domain warp for organic billowing shapes
    let warp1 = vec2<f32>(
        fbm3(animated),
        fbm3(animated + vec2<f32>(5.2, 1.3)),
    );
    let w1 = animated + warp1 * 0.45;
    let warp2 = vec2<f32>(
        fbm3(w1 * 2.0 + vec2<f32>(12.0, 3.0)),
        fbm3(w1 * 2.0 + vec2<f32>(-7.0, 9.0)),
    );
    let warped = w1 + warp2 * 0.18;

    // Shape composition: smooth base + ridged billows
    let base = fbm6(warped);
    let billows = fbm_ridged(warped * 2.0);
    let shape = base * 0.55 + billows * 0.45;

    // Remap by coverage
    let edge_low = 1.0 - coverage;
    let edge_high = edge_low + 0.22;
    let cloud_shape = smoothstep(edge_low, edge_high, shape);

    if cloud_shape < 0.001 {
        return out;
    }

    // High-frequency erosion for detailed edges
    let detail = fbm3(warped * 5.5 + vec2<f32>(77.0, 33.0));
    let eroded = saturate(cloud_shape - detail * 0.18);

    if eroded < 0.001 {
        return out;
    }

    // ── Lighting ──

    // View-dependent optical depth: looking along horizon travels through more cloud
    let view_slant = 1.0 / clamp(dir.y + 0.15, 0.15, 1.0);
    let optical_depth = eroded * density * absorption * view_slant;

    // Beer-Lambert transmission
    let beer = exp(-optical_depth);
    let powder_term = 1.0 - exp(-optical_depth * 2.0);
    let beer_powder = mix(beer, beer * powder_term, powder);

    // Vertical gradient: cloud bottom darker than top (simulates self-shadowing)
    let vgrad = smoothstep(0.0, 0.55, dir.y);
    let self_shadow = mix(0.28, 1.0, vgrad);

    let light = beer_powder * self_shadow;
    var lit = mix(shad_col, lit_col, light);

    // Silver lining: forward-scatter glow at cloud edges toward sun
    let sun_dot = dot(dir, sun_dir);
    let silver = pow(saturate(sun_dot), 1.0 / max(silver_s, 0.01)) * silver_i;
    let edge_mask = 1.0 - smoothstep(0.0, 0.5, eroded);
    lit += lit_col * 1.5 * silver * edge_mask;

    // Secondary rim: broader glow on entire lit side of cloud facing sun
    let broad_glow = pow(saturate(sun_dot), 2.0) * silver_i * 0.25;
    lit += lit_col * broad_glow * light;

    // Ambient floor
    lit = max(lit, shad_col * ambient + lit_col * ambient * 0.4);

    out.color = lit;
    out.alpha = saturate(eroded * density);
    return out;
}

// ── Cirrus: wispy, stretched high-altitude streaks ──

fn sample_cirrus(
    dir: vec3<f32>,
    time: f32,
    coverage: f32,
    density: f32,
    scale: f32,
    speed: f32,
    wind_dir: vec2<f32>,
    lit_col: vec3<f32>,
    sun_dir: vec3<f32>,
    silver_i: f32,
    silver_s: f32,
    ambient: f32,
) -> Layer {
    var out: Layer;
    out.color = vec3<f32>(0.0);
    out.alpha = 0.0;

    // Use cirrus-altitude shell for natural perspective
    let uv = shell_uv(dir, CIRRUS_H) * scale * 0.16;
    // Cirrus drifts slower; independent wind axis rotation gives cross-layer parallax
    let cirrus_wind = rot2(0.6) * wind_dir;
    let animated = uv + cirrus_wind * time * speed * 0.55;

    // Stretch along primary axis for horizontal streaks
    let stretch = mat2x2<f32>(1.0, 0.0, 0.0, 0.28);
    let stretched = stretch * animated;

    let warp = vec2<f32>(
        fbm3(stretched),
        fbm3(stretched + vec2<f32>(19.0, 7.0)),
    );
    let warped = stretched + warp * 0.3;

    let shape = fbm6(warped);

    // Cirrus has lower coverage threshold — always a bit thinner
    let cirrus_cov = coverage * 0.55 + 0.1;
    let edge_low = 1.0 - cirrus_cov;
    let edge_high = edge_low + 0.3;
    let cloud_shape = smoothstep(edge_low, edge_high, shape);

    if cloud_shape < 0.001 {
        return out;
    }

    let detail = fbm3(warped * 7.0 + vec2<f32>(131.0, 29.0));
    let eroded = saturate(cloud_shape - detail * 0.22);

    if eroded < 0.001 {
        return out;
    }

    // Cirrus is high above the sun-facing clouds — mostly bright and translucent
    var lit = lit_col * (ambient * 0.6 + 0.55);
    let sun_dot = dot(dir, sun_dir);
    let silver = pow(saturate(sun_dot), 1.0 / max(silver_s * 1.4, 0.01)) * silver_i;
    lit += lit_col * silver * 0.6;

    out.color = lit;
    // Cirrus is inherently thin — cap alpha contribution
    out.alpha = saturate(eroded * density * 0.55);
    return out;
}

// ── Sun-tinted horizon glow (added to haze so the sky near the sun warms up) ──

fn sky_sun_glow(dir: vec3<f32>, sun_dir: vec3<f32>, base_haze: vec3<f32>) -> vec3<f32> {
    let sun_dot = saturate(dot(dir, sun_dir));
    let halo = pow(sun_dot, 6.0) * 0.30 + pow(sun_dot, 48.0) * 0.55;
    return base_haze + vec3<f32>(1.0, 0.93, 0.82) * halo;
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    // Unpack uniforms
    let coverage = params_a.x;
    let density = params_a.y;
    let scale = params_a.z;
    let speed = params_a.w;
    let wind_dir = vec2<f32>(params_b.x, params_b.y);
    let altitude = params_b.z;
    let sun_elevation = params_b.w;
    let sun_dir = normalize(vec3<f32>(params_c.x, params_c.y, params_c.z));
    let absorption = params_c.w;
    let silver_i = params_d.x;
    let silver_s = params_d.y;
    let powder = params_d.z;
    let ambient = params_d.w;
    let haze_base = horizon_color.rgb;
    let atmo_strength = horizon_color.a;

    let dir = normalize(in.world_position.xyz);

    // Below-horizon discard
    if dir.y < 0.005 {
        discard;
    }

    // Altitude masking — fade clouds near horizon base and zenith top
    let alt_fade = smoothstep(altitude * 0.25, altitude * 0.25 + 0.22, dir.y);
    let alt_top = smoothstep(0.99, 0.88, dir.y);
    let alt_mask = alt_fade * alt_top;

    if alt_mask < 0.001 {
        discard;
    }

    let time = globals.time;

    // Sample both layers
    let cumulus = sample_cumulus(
        dir, time, coverage, density, scale, speed, wind_dir,
        cloud_color.rgb, shadow_color.rgb, sun_dir, absorption,
        silver_i, silver_s, powder, ambient,
    );
    let cirrus = sample_cirrus(
        dir, time, coverage, density, scale, speed, wind_dir,
        cloud_color.rgb, sun_dir, silver_i, silver_s, ambient,
    );

    if cumulus.alpha + cirrus.alpha < 0.002 {
        discard;
    }

    // Over-composite: cirrus is behind (higher/farther), cumulus in front
    var pre_color = cirrus.color * cirrus.alpha;
    var pre_alpha = cirrus.alpha;
    pre_color = cumulus.color * cumulus.alpha + pre_color * (1.0 - cumulus.alpha);
    pre_alpha = cumulus.alpha + pre_alpha * (1.0 - cumulus.alpha);

    // De-premultiply so atmospheric mix works on straight color
    var color = pre_color;
    if pre_alpha > 0.001 {
        color = pre_color / pre_alpha;
    }

    // Atmospheric perspective — haze near horizon, warmed near sun
    let sky_haze = sky_sun_glow(dir, sun_dir, haze_base);
    let horizon_t = 1.0 - pow(max(dir.y, 0.0), 0.35);
    let atmo = saturate(horizon_t * atmo_strength);
    color = mix(color, sky_haze, atmo);

    // Day/night fade
    let day_factor = smoothstep(-0.05, 0.15, sun_elevation);

    let final_alpha = pre_alpha * alt_mask * day_factor;

    if final_alpha < 0.001 {
        discard;
    }

    return vec4<f32>(color, final_alpha);
}
