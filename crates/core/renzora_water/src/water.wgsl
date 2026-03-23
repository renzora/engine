#import bevy_pbr::mesh_functions
#import bevy_pbr::view_transformations::position_world_to_clip
#import bevy_pbr::forward_io::VertexOutput
#import bevy_pbr::mesh_view_bindings::view
#import bevy_pbr::pbr_functions
#import bevy_pbr::pbr_types::{PbrInput, pbr_input_new}

// ── Uniform buffer ─────────────────────────────────────────────────────────

struct WaterUniforms {
    time: f32,
    _pad0: f32,
    _pad1: f32,
    _pad2: f32,

    wave_0: vec4<f32>,
    wave_0_amp: vec4<f32>,
    wave_1: vec4<f32>,
    wave_1_amp: vec4<f32>,
    wave_2: vec4<f32>,
    wave_2_amp: vec4<f32>,
    wave_3: vec4<f32>,
    wave_3_amp: vec4<f32>,
    wave_4: vec4<f32>,
    wave_4_amp: vec4<f32>,
    wave_5: vec4<f32>,
    wave_5_amp: vec4<f32>,

    wave_count: u32,
    _wpad0: f32,
    _wpad1: f32,
    _wpad2: f32,

    deep_color: vec4<f32>,
    shallow_color: vec4<f32>,
    foam_color: vec4<f32>,
    sun_direction: vec4<f32>,

    foam_threshold: f32,
    absorption: f32,
    roughness: f32,
    subsurface_strength: f32,

    // Object interactions: 8 slots, each vec4(x, z, radius, intensity)
    obj_0: vec4<f32>,
    obj_1: vec4<f32>,
    obj_2: vec4<f32>,
    obj_3: vec4<f32>,
    obj_4: vec4<f32>,
    obj_5: vec4<f32>,
    obj_6: vec4<f32>,
    obj_7: vec4<f32>,
    obj_count: u32,
    _opad0: f32,
    _opad1: f32,
    _opad2: f32,
}

@group(3) @binding(0) var<uniform> water: WaterUniforms;

const PI: f32 = 3.14159265359;
const GRAVITY: f32 = 9.81;

// ── Gerstner wave ──────────────────────────────────────────────────────────

// Returns (displacement.xyz, tangent contribution, binormal contribution)
// for analytical normal calculation.
fn gerstner_wave(
    pos: vec2<f32>,
    params: vec4<f32>,   // (dir.x, dir.y, steepness, wavelength)
    amplitude: f32,
    time: f32,
    wave_count_f: f32,
) -> vec3<f32> {
    let dir = normalize(params.xy);
    let steepness = params.z;
    let wavelength = params.w;

    if wavelength < 0.01 || amplitude < 0.001 {
        return vec3<f32>(0.0);
    }

    let w = 2.0 * PI / wavelength;
    let phi = sqrt(GRAVITY / w) * w; // deep water dispersion
    let d = dot(dir, pos) * w + time * phi;
    let Q = steepness / (w * amplitude * wave_count_f + 0.001);

    return vec3<f32>(
        Q * amplitude * dir.x * cos(d),
        amplitude * sin(d),
        Q * amplitude * dir.y * cos(d),
    );
}

// Compute analytical normal from Gerstner wave derivatives.
// For each wave: tangent += (-d.x² * WA * sin(f), d.x * WA * cos(f), -d.x*d.y * WA * sin(f))
// binormal analog. Final normal = cross(binormal, tangent).
fn gerstner_normal(
    pos: vec2<f32>,
    params: vec4<f32>,
    amplitude: f32,
    time: f32,
    wave_count_f: f32,
) -> vec3<f32> {
    let dir = normalize(params.xy);
    let steepness = params.z;
    let wavelength = params.w;

    if wavelength < 0.01 || amplitude < 0.001 {
        return vec3<f32>(0.0);
    }

    let w = 2.0 * PI / wavelength;
    let phi = sqrt(GRAVITY / w) * w;
    let d = dot(dir, pos) * w + time * phi;
    let Q = steepness / (w * amplitude * wave_count_f + 0.001);
    let WA = w * amplitude;
    let s = sin(d);
    let c = cos(d);

    // Returns (nx_contribution, ny_contribution, nz_contribution)
    return vec3<f32>(
        dir.x * WA * c,
        Q * WA * s,
        dir.y * WA * c,
    );
}

fn sum_displacement(pos: vec2<f32>, time: f32) -> vec3<f32> {
    let wc = f32(water.wave_count);
    var d = vec3<f32>(0.0);
    if water.wave_count > 0u { d += gerstner_wave(pos, water.wave_0, water.wave_0_amp.x, time, wc); }
    if water.wave_count > 1u { d += gerstner_wave(pos, water.wave_1, water.wave_1_amp.x, time, wc); }
    if water.wave_count > 2u { d += gerstner_wave(pos, water.wave_2, water.wave_2_amp.x, time, wc); }
    if water.wave_count > 3u { d += gerstner_wave(pos, water.wave_3, water.wave_3_amp.x, time, wc); }
    if water.wave_count > 4u { d += gerstner_wave(pos, water.wave_4, water.wave_4_amp.x, time, wc); }
    if water.wave_count > 5u { d += gerstner_wave(pos, water.wave_5, water.wave_5_amp.x, time, wc); }
    return d;
}

fn sum_normal(pos: vec2<f32>, time: f32) -> vec3<f32> {
    let wc = f32(water.wave_count);
    var n_contrib = vec3<f32>(0.0);
    if water.wave_count > 0u { n_contrib += gerstner_normal(pos, water.wave_0, water.wave_0_amp.x, time, wc); }
    if water.wave_count > 1u { n_contrib += gerstner_normal(pos, water.wave_1, water.wave_1_amp.x, time, wc); }
    if water.wave_count > 2u { n_contrib += gerstner_normal(pos, water.wave_2, water.wave_2_amp.x, time, wc); }
    if water.wave_count > 3u { n_contrib += gerstner_normal(pos, water.wave_3, water.wave_3_amp.x, time, wc); }
    if water.wave_count > 4u { n_contrib += gerstner_normal(pos, water.wave_4, water.wave_4_amp.x, time, wc); }
    if water.wave_count > 5u { n_contrib += gerstner_normal(pos, water.wave_5, water.wave_5_amp.x, time, wc); }

    // Reconstruct normal from summed derivatives
    return normalize(vec3<f32>(-n_contrib.x, 1.0 - n_contrib.y, -n_contrib.z));
}

// ── Vertex shader ──────────────────────────────────────────────────────────

@vertex
fn vertex(
    @builtin(instance_index) instance_index: u32,
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
) -> VertexOutput {
    var out: VertexOutput;

    let world_from_local = mesh_functions::get_world_from_local(instance_index);

    // Displace vertex with summed Gerstner waves
    var pos = position;
    let displacement = sum_displacement(pos.xz, water.time);
    pos += displacement;

    // Analytical normal from wave derivatives
    let wave_normal = sum_normal(position.xz, water.time);

    let world_position = mesh_functions::mesh_position_local_to_world(
        world_from_local,
        vec4<f32>(pos, 1.0)
    );

    out.position = position_world_to_clip(world_position.xyz);
    out.world_position = world_position;
    out.world_normal = mesh_functions::mesh_normal_local_to_world(wave_normal, instance_index);
    out.uv = uv;

    return out;
}

// ── Fragment helpers ────────────────────────────────────────────────────────

// Schlick fresnel
fn fresnel_schlick(cos_theta: f32, f0: f32) -> f32 {
    return f0 + (1.0 - f0) * pow(clamp(1.0 - cos_theta, 0.0, 1.0), 5.0);
}

// GGX normal distribution
fn ggx_distribution(n_dot_h: f32, roughness: f32) -> f32 {
    let a = roughness * roughness;
    let a2 = a * a;
    let d = n_dot_h * n_dot_h * (a2 - 1.0) + 1.0;
    return a2 / (PI * d * d + 0.0001);
}

// Beer's law light absorption
fn beer_absorption(depth: f32, coeff: f32) -> vec3<f32> {
    // Red absorbed fastest, green moderate, blue least
    return exp(vec3<f32>(-coeff * 3.0, -coeff * 1.0, -coeff * 0.4) * depth);
}

// Simple hash for foam noise
fn hash_water(p: vec2<f32>) -> f32 {
    var p3 = fract(vec3<f32>(p.x, p.y, p.x) * 0.1031);
    p3 += dot(p3, p3.yzx + 33.33);
    return fract((p3.x + p3.y) * p3.z);
}

fn noise_water(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    return mix(
        mix(hash_water(i), hash_water(i + vec2<f32>(1.0, 0.0)), u.x),
        mix(hash_water(i + vec2<f32>(0.0, 1.0)), hash_water(i + vec2<f32>(1.0, 1.0)), u.x),
        u.y
    );
}

// ── Object interaction helpers ─────────────────────────────────────────────

fn get_obj(index: u32) -> vec4<f32> {
    switch index {
        case 0u: { return water.obj_0; }
        case 1u: { return water.obj_1; }
        case 2u: { return water.obj_2; }
        case 3u: { return water.obj_3; }
        case 4u: { return water.obj_4; }
        case 5u: { return water.obj_5; }
        case 6u: { return water.obj_6; }
        case 7u: { return water.obj_7; }
        default: { return vec4<f32>(0.0); }
    }
}

// Compute ripple + shadow + foam contribution from all active objects
fn object_effects(world_xz: vec2<f32>, time: f32) -> vec3<f32> {
    var ripple = 0.0;
    var shadow = 0.0;
    var foam = 0.0;

    for (var i = 0u; i < water.obj_count; i++) {
        let obj = get_obj(i);
        let obj_xz = obj.xy;
        let radius = obj.z;
        let intensity = obj.w;

        let to_obj = world_xz - obj_xz;
        let dist = length(to_obj);

        // ── Shadow — soft darkening under the object ──
        let shadow_radius = radius * 1.0;
        let s = 1.0 - smoothstep(0.0, shadow_radius, dist);
        let shadow_noise = noise_water(world_xz * 3.0 + vec2<f32>(time * 0.1)) * 0.3;
        shadow = max(shadow, s * intensity * (0.25 + shadow_noise));

        // ── Ripples — animated, outward-propagating, organic ──
        let falloff = smoothstep(radius * 6.0, radius * 0.3, dist);
        if falloff > 0.001 {
            // Animated noise distortion — shifts over time so pattern never freezes
            let noise_uv = to_obj * 1.5 + vec2<f32>(time * 0.8, time * 0.6);
            let angle_noise = (noise_water(noise_uv) - 0.5) * 0.5;
            let dist_warped = dist + angle_noise;

            // Waves propagate outward (-time means expanding from center)
            // Different speeds and frequencies prevent bullseye pattern
            let phase = f32(i) * 1.7; // per-object phase offset
            let w1 = sin(dist_warped * 10.0 - time * 6.0 + phase) * 0.45;
            let w2 = sin(dist_warped * 6.5 - time * 4.5 + phase + 2.0) * 0.3;
            let w3 = sin(dist_warped * 15.0 - time * 8.0 + phase + 4.5) * 0.15;

            // Time-varying amplitude — pulses of ripple energy
            let pulse = 0.7 + 0.3 * sin(time * 2.0 + phase);
            let wave_sum = (w1 + w2 + w3) * pulse;

            // Energy falls off with distance
            let energy = 1.0 / (1.0 + dist * dist * 0.2);

            // Animated break-up noise — constantly shifts
            let break_uv = world_xz * 6.0 + vec2<f32>(time * 1.2, -time * 0.8);
            let break_noise = noise_water(break_uv) * 0.7 + 0.3;
            let modulated = wave_sum * energy * falloff * break_noise;

            ripple += modulated * intensity * 0.2;
        }

        // ── Foam — patchy, noisy ring around waterline contact ──
        let foam_inner = smoothstep(radius * 0.15, radius * 0.5, dist);
        let foam_outer = smoothstep(radius * 1.5, radius * 0.7, dist);
        let foam_ring = foam_inner * foam_outer;
        if foam_ring > 0.001 {
            let fn1 = noise_water(world_xz * 6.0 + vec2<f32>(time * 0.4, time * 0.3));
            let fn2 = noise_water(world_xz * 12.0 + vec2<f32>(-time * 0.3, time * 0.5));
            let foam_pattern = fn1 * 0.6 + fn2 * 0.4;
            // Only show foam where noise is above threshold — creates patchy look
            let foam_val = foam_ring * smoothstep(0.3, 0.6, foam_pattern) * intensity;
            foam = max(foam, foam_val * 0.7);
        }
    }

    return vec3<f32>(clamp(ripple, -0.15, 0.15), shadow, foam);
}

// ── Fragment shader ────────────────────────────────────────────────────────

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let N = normalize(in.world_normal);
    let V = pbr_functions::calculate_view(in.world_position, false);
    let N_dot_V = max(dot(N, V), 0.001);

    // ── Water body color ──
    let depth_factor = 1.0 - N_dot_V;
    var base_color = mix(water.shallow_color.rgb, water.deep_color.rgb, depth_factor);

    // Beer's law tint
    let optical_depth = depth_factor * 4.0 + 1.0;
    let absorbed = beer_absorption(optical_depth, water.absorption);
    base_color *= absorbed + 0.4;

    // Fresnel — blend in sky reflection
    let fresnel = fresnel_schlick(N_dot_V, 0.02);
    let reflect_dir = reflect(-V, N);
    let sky_grad = smoothstep(-0.1, 0.8, reflect_dir.y);
    let sky_color = mix(
        vec3<f32>(0.3, 0.4, 0.5),
        vec3<f32>(0.5, 0.7, 0.9),
        sky_grad
    );
    base_color = mix(base_color, sky_color, fresnel);

    // ── PBR lighting with shadows ──
    var pbr = pbr_input_new();
    pbr.material.base_color = vec4<f32>(base_color, 1.0);
    pbr.material.metallic = 0.0;
    pbr.material.perceptual_roughness = water.roughness;
    pbr.world_normal = N;
    pbr.world_position = in.world_position;
    pbr.N = N;
    pbr.V = V;

    var water_color = pbr_functions::apply_pbr_lighting(pbr).rgb;

    // ── Subsurface scattering (additive, post-PBR) ──
    let L = normalize(-water.sun_direction.xyz);
    let sss_dot = max(dot(V, -L), 0.0);
    let sss = water.subsurface_strength * pow(sss_dot, 4.0) * water.shallow_color.rgb * 0.4;
    water_color += sss;

    // ── Foam ──
    let crest_height = in.world_position.y;
    let foam_noise = noise_water(in.uv * 40.0 + vec2<f32>(water.time * 0.3, water.time * 0.2));
    let foam_crest = smoothstep(water.foam_threshold, water.foam_threshold + 0.3, crest_height + foam_noise * 0.1);

    let dissolve = noise_water(in.uv * 80.0 + vec2<f32>(water.time * 0.15, -water.time * 0.1));
    let foam_masked = foam_crest * smoothstep(0.2, 0.5, dissolve);
    let N_dot_L = max(dot(N, L), 0.0);
    let foam_lit = water.foam_color.rgb * (0.8 + N_dot_L * 0.2);
    water_color = mix(water_color, foam_lit, clamp(foam_masked * 0.85, 0.0, 1.0));

    // ── Micro sparkles ──
    let sp = noise_water(in.uv * 200.0 + vec2<f32>(water.time * 3.0, water.time * 2.5));
    let sparkle = pow(sp, 12.0) * 1.5;
    let sparkle_mask = smoothstep(0.3, 0.8, N.y);
    water_color += vec3<f32>(1.0, 0.97, 0.9) * sparkle * sparkle_mask;

    // ── Object interactions (ripples, shadows, foam) ──
    let world_xz = in.world_position.xz;
    let fx = object_effects(world_xz, water.time);
    let obj_ripple = fx.x;
    let obj_shadow = fx.y;
    let obj_foam = fx.z;

    // Ripples — signed value creates natural bright/dark disturbance
    water_color += water_color * obj_ripple;
    // Shadow — soft darkening under objects
    water_color *= 1.0 - obj_shadow;
    // Foam — patchy white around waterline
    water_color = mix(water_color, water.foam_color.rgb * (0.8 + N_dot_L * 0.2), obj_foam);

    // ── Atmospheric haze at glancing angles ──
    let haze = pow(1.0 - N_dot_V, 4.0) * 0.12;
    water_color = mix(water_color, sky_color, haze);

    return vec4<f32>(water_color, 1.0);
}
