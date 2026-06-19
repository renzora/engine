// Procedural Clouds Shader — realistic two-layer sky with spherical projection.
//
// Layer 1: Cumulus (lower, dense, billowy) lit with a light-march toward the sun.
// Layer 2: Cirrus (higher, stretched, wispy) for upper-atmosphere streaks.
//
// The cumulus lighting follows the physically-based volumetric-cloud model
// (Schneider/Hillaire-style scattering), adapted to our 2.5D dome:
//   * A secondary "light march" steps the density field TOWARD the sun and
//     accumulates optical depth → Beer-Lambert transmittance. This — not a
//     view-elevation gradient — is what gives clouds bright sun-facing sides and
//     dark shadow sides (the single biggest realism cue).
//   * A dual-lobe Henyey-Greenstein phase function provides directional
//     forward-scatter (the silver lining) plus a softer back lobe.
//   * A 3-octave multi-scatter approximation (Wrenninge) keeps shadowed cloud
//     interiors lit instead of pure black.
//   * Shadowed regions fall back to the (bluish) Shadow Color, which is what
//     lights real cloud undersides: scattered blue skylight, not darkness.
// Compositing uses the over-operator so cumulus sits in front of cirrus.

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

// Sun light-march: how many shadow samples and how far each steps through the
// noise field. Kept small (this is a dome, not a full volume) but enough to
// resolve a cloud's own thickness against the sun.
const LIGHT_STEPS: i32 = 5;
const LIGHT_STEP: f32 = 0.34;

// Multi-scatter octaves. Each successive octave scatters less, extincts less,
// and has a more isotropic phase — the cheap stand-in for light that bounced
// several times inside the cloud before reaching the eye.
const MS_OCTAVES: i32 = 3;
const MS_SCATTER: f32 = 0.55; // contribution falloff per octave
const MS_EXTINCT: f32 = 0.55; // extinction falloff per octave (deeper light reach)
const MS_PHASE: f32 = 0.5;    // phase eccentricity falloff per octave

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

// Normalized 3-octave FBM — cheap density for the sun light-march, where we need
// the broad cloud mass between a point and the sun, not fine detail.
fn fbm3n(p_in: vec2<f32>) -> f32 {
    var p = p_in;
    var value = 0.0;
    var amplitude = 0.5;
    var total = 0.0;
    let r = rot2(0.7);
    for (var i = 0; i < 3; i = i + 1) {
        value += amplitude * grad_noise(p);
        total += amplitude;
        p = r * p * 2.0 + vec2<f32>(70.0);
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

// ── Henyey-Greenstein phase (includes the 1/4π normalization) ──

fn hg(cos_t: f32, g: f32) -> f32 {
    let g2 = g * g;
    let denom = 1.0 + g2 - 2.0 * g * cos_t;
    return (1.0 - g2) / (4.0 * PI * pow(max(denom, 1e-4), 1.5));
}

// Dual-lobe blend: a tight forward lobe (silver lining) + a softer wide lobe.
fn dual_hg(cos_t: f32, g0: f32, g1: f32, blend: f32) -> f32 {
    return mix(hg(cos_t, g0), hg(cos_t, g1), blend);
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

// Two-stage domain warp shared by the shape and the light-march, so both sample
// the same billowy field. Returns the warped coordinate in shape space.
fn warp_pos(animated: vec2<f32>) -> vec2<f32> {
    let warp1 = vec2<f32>(
        fbm3(animated),
        fbm3(animated + vec2<f32>(5.2, 1.3)),
    );
    let w1 = animated + warp1 * 0.45;
    let warp2 = vec2<f32>(
        fbm3(w1 * 2.0 + vec2<f32>(12.0, 3.0)),
        fbm3(w1 * 2.0 + vec2<f32>(-7.0, 9.0)),
    );
    return w1 + warp2 * 0.18;
}

// Full-detail cumulus density at a warped position, remapped by coverage and
// eroded at the edges. Returns [0,1].
fn cumulus_density(warped: vec2<f32>, coverage: f32) -> f32 {
    let base = fbm6(warped);
    let billows = fbm_ridged(warped * 2.0);
    let shape = base * 0.55 + billows * 0.45;

    let edge_low = 1.0 - coverage;
    let edge_high = edge_low + 0.22;
    let cloud_shape = smoothstep(edge_low, edge_high, shape);
    if cloud_shape < 0.001 {
        return 0.0;
    }

    let detail = fbm3(warped * 5.5 + vec2<f32>(77.0, 33.0));
    return saturate(cloud_shape - detail * 0.18);
}

// Cheap density used along the sun light-march — coarse mass only.
fn cumulus_density_cheap(warped: vec2<f32>, coverage: f32) -> f32 {
    let base = fbm3n(warped);
    let billows = fbm_ridged(warped * 2.0);
    let shape = base * 0.5 + billows * 0.5;
    let edge_low = 1.0 - coverage;
    let edge_high = edge_low + 0.25;
    return smoothstep(edge_low, edge_high, shape);
}

// ── Cumulus: dense billowy front-layer clouds with sun light-march + dual-lobe phase ──

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
    let warped = warp_pos(animated);

    let d0 = cumulus_density(warped, coverage);
    if d0 < 0.001 {
        return out;
    }

    // ── Sun light-march: accumulate optical depth from this point toward the sun.
    // Horizontal shadow direction = sun's projected azimuth; a low sun stretches
    // the shadow further through the field, just like long evening cloud shadows.
    let sun_xz = vec2<f32>(sun_dir.x, sun_dir.z);
    let sun_uv = normalize(sun_xz + vec2<f32>(1e-4, 0.0));
    let sun_reach = LIGHT_STEP / clamp(sun_dir.y * 0.5 + 0.5, 0.4, 1.0);

    var light_od = 0.0;
    for (var i = 1; i <= LIGHT_STEPS; i = i + 1) {
        let sp = warped + sun_uv * sun_reach * f32(i);
        light_od += cumulus_density_cheap(sp, coverage);
    }
    light_od *= sun_reach * density * absorption;

    // Self optical depth of THIS sample (controls how opaque it reads).
    let self_od = d0 * density * absorption;

    // ── Multi-scatter Beer-Lambert toward the sun.
    // Each octave reaches deeper (less extinction) and contributes less — the
    // classic stand-in for in-cloud multiple scattering so cores aren't black.
    let cos_t = dot(dir, sun_dir);
    var direct = 0.0;
    var atten = 1.0;
    var extinct = 1.0;
    var ecc = 1.0;
    for (var ms = 0; ms < MS_OCTAVES; ms = ms + 1) {
        let transmit = exp(-light_od * extinct);
        // Dual-lobe phase, eccentricity shrinking toward isotropic each octave.
        let phase = dual_hg(cos_t, 0.75 * ecc, -0.2 * ecc, 0.4);
        direct += atten * transmit * phase;
        atten *= MS_SCATTER;
        extinct *= MS_EXTINCT;
        ecc *= MS_PHASE;
    }
    // Phase carries a 1/4π factor; lift back to a perceptual scale.
    direct *= 4.0 * PI;

    // Powder: thin sunlit edges look darker because little has scattered yet.
    let powder_term = 1.0 - exp(-self_od * 2.0);
    let direct_lit = direct * mix(1.0, powder_term, powder);

    // ── Compose radiance.
    // Sunlit scattering uses the lit color; shadowed regions are filled by the
    // (bluish) shadow color standing in for scattered skylight + ambient — which
    // is exactly what lights real cloud undersides.
    let sky_fill = shad_col * (ambient + 0.15);
    var lit = lit_col * direct_lit + sky_fill;

    // Explicit silver lining: a sharp forward-scatter rim near the sun, strongest
    // on thin/edge regions, layered on top so the inspector knobs stay meaningful.
    let sun_facing = pow(saturate(cos_t), 1.0 / max(silver_s, 0.01));
    let edge_mask = 1.0 - smoothstep(0.0, 0.5, d0);
    lit += lit_col * silver_i * sun_facing * edge_mask;

    out.color = lit;
    out.alpha = saturate(d0 * density);
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
