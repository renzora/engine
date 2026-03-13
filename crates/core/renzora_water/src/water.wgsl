#import bevy_pbr::mesh_functions
#import bevy_pbr::view_transformations::position_world_to_clip
#import bevy_pbr::forward_io::VertexOutput
#import bevy_pbr::mesh_view_bindings::view

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

// Compute ripple + shadow contribution from all active objects
fn object_effects(world_xz: vec2<f32>, time: f32) -> vec3<f32> {
    // Returns (ripple_brightness, shadow_darkness, foam_amount)
    var ripple = 0.0;
    var shadow = 0.0;
    var foam = 0.0;

    for (var i = 0u; i < water.obj_count; i++) {
        let obj = get_obj(i);
        let obj_xz = obj.xy;
        let radius = obj.z;
        let intensity = obj.w;

        let dist = distance(world_xz, obj_xz);

        // Shadow — darkening directly under the object
        let shadow_radius = radius * 0.8;
        let s = 1.0 - smoothstep(0.0, shadow_radius, dist);
        shadow = max(shadow, s * intensity * 0.35);

        // Ripples — concentric rings expanding outward from object
        let ripple_zone = smoothstep(radius * 3.0, radius * 0.5, dist);
        let ripple_wave = sin(dist * 8.0 - time * 4.0) * 0.5 + 0.5;
        let ripple_decay = 1.0 / (1.0 + dist * 0.5);
        ripple = max(ripple, ripple_wave * ripple_zone * ripple_decay * intensity * 0.3);

        // Foam ring around object at waterline
        let foam_ring = smoothstep(radius * 1.2, radius * 0.6, dist)
                      * smoothstep(radius * 0.2, radius * 0.5, dist);
        let foam_noise_val = noise_water(world_xz * 5.0 + vec2<f32>(time * 0.5));
        foam = max(foam, foam_ring * foam_noise_val * intensity * 0.6);
    }

    return vec3<f32>(ripple, shadow, foam);
}

// ── Fragment shader ────────────────────────────────────────────────────────

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let N = normalize(in.world_normal);
    let V = normalize(view.world_position.xyz - in.world_position.xyz);
    let L = normalize(-water.sun_direction.xyz);
    let H = normalize(V + L);

    let N_dot_V = max(dot(N, V), 0.001);
    let N_dot_L = max(dot(N, L), 0.0);
    let N_dot_H = max(dot(N, H), 0.0);

    // ── Fresnel ──
    // Water IOR ~1.33 → F0 ≈ 0.02
    let fresnel = fresnel_schlick(N_dot_V, 0.02);

    // ── Water body color ──
    // Blend deep → shallow based on view angle (steep = deep, head-on = shallow)
    let depth_factor = 1.0 - N_dot_V;
    var water_color = mix(water.shallow_color.rgb, water.deep_color.rgb, depth_factor);

    // Beer's law tint — deeper areas shift toward blue
    let optical_depth = depth_factor * 4.0 + 1.0;
    let absorbed = beer_absorption(optical_depth, water.absorption);
    water_color *= absorbed + 0.4;

    // ── Subsurface scattering ──
    let sss_dot = max(dot(V, -L), 0.0);
    let sss = water.subsurface_strength * pow(sss_dot, 4.0) * water.shallow_color.rgb * 0.4;
    water_color += sss;

    // ── Sky reflection (simplified) ──
    let reflect_dir = reflect(-V, N);
    let sky_grad = smoothstep(-0.1, 0.8, reflect_dir.y);
    let sky_color = mix(
        vec3<f32>(0.3, 0.4, 0.5),
        vec3<f32>(0.5, 0.7, 0.9),
        sky_grad
    );

    // Blend body + reflection via fresnel
    water_color = mix(water_color, sky_color, fresnel);

    // ── Diffuse lighting ──
    water_color *= 0.35 + N_dot_L * 0.65;

    // ── Specular (GGX) ──
    let D = ggx_distribution(N_dot_H, water.roughness);
    let spec_fresnel = fresnel_schlick(max(dot(H, V), 0.0), 0.02);
    let specular = D * spec_fresnel;
    water_color += vec3<f32>(1.0, 0.95, 0.85) * specular;

    // Broader secondary lobe
    let D2 = ggx_distribution(N_dot_H, water.roughness * 4.0);
    water_color += vec3<f32>(1.0, 0.95, 0.85) * D2 * spec_fresnel * 0.08;

    // ── Foam ──
    let crest_height = in.world_position.y;
    let foam_noise = noise_water(in.uv * 40.0 + vec2<f32>(water.time * 0.3, water.time * 0.2));
    let foam_crest = smoothstep(water.foam_threshold, water.foam_threshold + 0.3, crest_height + foam_noise * 0.1);

    // Foam dissolve pattern
    let dissolve = noise_water(in.uv * 80.0 + vec2<f32>(water.time * 0.15, -water.time * 0.1));
    let foam_masked = foam_crest * smoothstep(0.2, 0.5, dissolve);
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

    // Ripple highlights
    water_color += vec3<f32>(1.0, 0.97, 0.9) * obj_ripple;

    // Shadow darkening under objects
    water_color *= 1.0 - obj_shadow;

    // Foam ring around objects
    water_color = mix(water_color, water.foam_color.rgb * (0.8 + N_dot_L * 0.2), obj_foam);

    // ── Atmospheric haze at glancing angles ──
    let haze = pow(1.0 - N_dot_V, 4.0) * 0.12;
    water_color = mix(water_color, sky_color, haze);

    return vec4<f32>(water_color, 1.0);
}
