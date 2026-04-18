#import bevy_pbr::mesh_functions
#import bevy_pbr::view_transformations::position_world_to_clip
#import bevy_pbr::forward_io::VertexOutput
#import bevy_pbr::mesh_view_bindings as view_bindings
#import bevy_pbr::pbr_functions
#import bevy_pbr::pbr_types::{PbrInput, pbr_input_new}

// ── Uniform buffer ─────────────────────────────────────────────────────────

struct WaterUniforms {
    time: f32,
    refraction_strength: f32,
    max_depth: f32,
    caustic_intensity: f32,

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
    specular_power: f32,
    wind_speed: f32,
    wind_angle: f32,

    deep_color: vec4<f32>,
    shallow_color: vec4<f32>,
    foam_color: vec4<f32>,
    sun_direction: vec4<f32>,
    absorption_rgb: vec4<f32>,   // (r, g, b, foam_depth)

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

fn gerstner_wave(
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

    return vec3<f32>(
        Q * amplitude * dir.x * cos(d),
        amplitude * sin(d),
        Q * amplitude * dir.y * cos(d),
    );
}

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
    let world_pos = mesh_functions::mesh_position_local_to_world(
        world_from_local,
        vec4<f32>(position, 1.0)
    );

    let displacement = sum_displacement(world_pos.xz, water.time);
    let displaced = world_pos.xyz + displacement;
    let wave_normal = sum_normal(world_pos.xz, water.time);

    out.position = position_world_to_clip(displaced);
    out.world_position = vec4<f32>(displaced, 1.0);
    out.world_normal = wave_normal;
    out.uv = uv;

    return out;
}

// ── Noise primitives ──────────────────────────────────────────────────────

fn hash_water(p: vec2<f32>) -> f32 {
    var p3 = fract(vec3<f32>(p.x, p.y, p.x) * 0.1031);
    p3 += dot(p3, p3.yzx + 33.33);
    return fract((p3.x + p3.y) * p3.z);
}

fn hash2_water(p: vec2<f32>) -> vec2<f32> {
    let p2 = vec2<f32>(
        dot(p, vec2<f32>(127.1, 311.7)),
        dot(p, vec2<f32>(269.5, 183.3))
    );
    return fract(sin(p2) * 43758.5453123);
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

// ── Voronoi for cellular foam ─────────────────────────────────────────────

fn voronoi_water(p: vec2<f32>, t: f32) -> vec3<f32> {
    let cell = floor(p);
    let local = fract(p);

    var f1 = 8.0;
    var f2 = 8.0;
    var cell_id = 0.0;

    for (var j = -1; j <= 1; j++) {
        for (var i = -1; i <= 1; i++) {
            let neighbor = vec2<f32>(f32(i), f32(j));
            let cell_pos = cell + neighbor;
            let rnd = hash2_water(cell_pos);
            // Animate cell points slowly for organic drift
            let point = neighbor + 0.5 + 0.4 * sin(t * 0.3 + 6.2831 * rnd) - local;
            let d = dot(point, point);
            if d < f1 {
                f2 = f1;
                f1 = d;
                cell_id = rnd.x;
            } else if d < f2 {
                f2 = d;
            }
        }
    }

    return vec3<f32>(sqrt(f1), sqrt(f2), cell_id);
}

fn voronoi_foam(world_xz: vec2<f32>, time: f32) -> f32 {
    let v1 = voronoi_water(world_xz * 8.0 + vec2<f32>(time * 0.05, time * 0.03), time);
    let v2 = voronoi_water(world_xz * 20.0 + vec2<f32>(-time * 0.03, time * 0.04), time);
    let edge1 = 1.0 - smoothstep(0.0, 0.12, v1.y - v1.x);
    let edge2 = 1.0 - smoothstep(0.0, 0.08, v2.y - v2.x);
    return edge1 * 0.6 + edge2 * 0.4;
}

// ── Multi-octave FBM with domain rotation ─────────────────────────────────

fn fbm_5oct(p: vec2<f32>) -> f32 {
    var v = 0.0;
    var amp = 0.5;
    var fp = p;
    for (var i = 0; i < 5; i++) {
        v += noise_water(fp) * amp;
        amp *= 0.5;
        // Rotate domain ~37 degrees (unit magnitude) then scale by lacunarity 2.0
        let rx = fp.x * 0.8 - fp.y * 0.6;
        let ry = fp.x * 0.6 + fp.y * 0.8;
        fp = vec2<f32>(rx, ry) * 2.0;
    }
    return v;
}

// ── Dual-layer detail normals ─────────────────────────────────────────────

fn detail_normal_multi(world_xz: vec2<f32>, time: f32) -> vec3<f32> {
    let strength = 0.25;
    let eps = 0.15;

    // Layer 1: large flowing ripples
    let p1 = world_xz * 1.5 + vec2<f32>(time * 0.15, time * 0.1);
    // Layer 2: medium cross-chop
    let p2 = world_xz * 4.0 + vec2<f32>(-time * 0.08, time * 0.18);

    let h1  = fbm_5oct(p1);
    let h1x = fbm_5oct(p1 + vec2<f32>(eps, 0.0));
    let h1z = fbm_5oct(p1 + vec2<f32>(0.0, eps));

    let h2  = fbm_5oct(p2);
    let h2x = fbm_5oct(p2 + vec2<f32>(eps, 0.0));
    let h2z = fbm_5oct(p2 + vec2<f32>(0.0, eps));

    let dx = ((h1x - h1) + (h2x - h2) * 0.5) / eps;
    let dz = ((h1z - h1) + (h2z - h2) * 0.5) / eps;

    return normalize(vec3<f32>(-dx * strength, 1.0, -dz * strength));
}

// ── Procedural sky dome ───────────────────────────────────────────────────

fn procedural_sky(dir: vec3<f32>, sun_dir: vec3<f32>, intensity: f32) -> vec3<f32> {
    let sun_pos = normalize(-sun_dir);
    let sun_height = -sun_dir.y; // positive when sun is above horizon

    // Zenith-to-horizon gradient (Rayleigh approximation)
    let y = max(dir.y, 0.0);
    let horizon_factor = pow(1.0 - y, 3.0);

    // Base sky colors adapt to sun elevation
    let zenith_day = vec3<f32>(0.15, 0.3, 0.65);
    let horizon_day = vec3<f32>(0.5, 0.6, 0.7);
    let zenith_night = vec3<f32>(0.005, 0.005, 0.015);
    let horizon_night = vec3<f32>(0.01, 0.01, 0.02);

    let zenith = mix(zenith_night, zenith_day, intensity);
    let horizon = mix(horizon_night, horizon_day, intensity);
    var sky = mix(zenith, horizon, horizon_factor);

    // Sunset/sunrise coloring when sun is near horizon
    let sunset_amount = smoothstep(0.3, 0.0, abs(sun_height)) * intensity;
    let sunset_color = vec3<f32>(0.8, 0.3, 0.1);
    let sunset_mask = pow(horizon_factor, 1.5);
    // Stronger sunset color in the direction of the sun
    let sun_side = max(dot(normalize(vec2<f32>(dir.x, dir.z)), normalize(vec2<f32>(sun_pos.x, sun_pos.z))), 0.0);
    let sunset_directional = sunset_mask * (0.3 + 0.7 * pow(sun_side, 2.0));
    sky = mix(sky, sunset_color * intensity, sunset_amount * sunset_directional * 0.7);

    // Sun disc + bloom
    let sun_dot = max(dot(dir, sun_pos), 0.0);
    let sun_disc = pow(sun_dot, 800.0) * 10.0 * intensity;
    let sun_bloom = pow(sun_dot, 8.0) * 0.4 * intensity;
    let sun_color = mix(vec3<f32>(1.0, 0.5, 0.2), vec3<f32>(1.0, 0.95, 0.8), clamp(sun_height * 3.0, 0.0, 1.0));
    sky += sun_color * (sun_disc + sun_bloom);

    // Below-horizon darkening
    if dir.y < 0.0 {
        sky *= max(1.0 + dir.y * 3.0, 0.05);
    }

    return sky;
}

// ── Fresnel ───────────────────────────────────────────────────────────────

fn fresnel_schlick(cos_theta: f32, f0: f32) -> f32 {
    return f0 + (1.0 - f0) * pow(clamp(1.0 - cos_theta, 0.0, 1.0), 5.0);
}

// ── GGX specular ──────────────────────────────────────────────────────────

fn ggx_distribution(n_dot_h: f32, roughness: f32) -> f32 {
    let a = roughness * roughness;
    let a2 = a * a;
    let d = n_dot_h * n_dot_h * (a2 - 1.0) + 1.0;
    return a2 / (PI * d * d + 0.0001);
}

// ── Caustic pattern ──────────────────────────────────────────────────────

fn caustic_pattern(p: vec2<f32>, t: f32) -> f32 {
    let p1 = p * 8.0 + vec2<f32>(t * 0.3, t * 0.2);
    let p2 = p * 12.0 + vec2<f32>(-t * 0.2, t * 0.4);
    let p3 = p * 6.0 + vec2<f32>(t * 0.15, -t * 0.25);
    var c = 0.0;
    c += smoothstep(0.48, 0.52, noise_water(p1)) * 0.5;
    c += smoothstep(0.46, 0.54, noise_water(p2)) * 0.35;
    c += smoothstep(0.44, 0.50, noise_water(p3)) * 0.25;
    return c;
}

// ── Depth utilities ───────────────────────────────────────────────────────

fn linearize_depth(ndc_depth: f32) -> f32 {
    let near = view_bindings::view.clip_from_view[3][2];
    let far_factor = view_bindings::view.clip_from_view[2][2];
    return near / (far_factor + ndc_depth);
}

fn world_to_screen_uv(world_pos: vec4<f32>) -> vec2<f32> {
    let clip = view_bindings::view.clip_from_world * world_pos;
    let ndc = clip.xy / clip.w;
    return ndc * vec2<f32>(0.5, -0.5) + 0.5;
}

// ── Object interaction helpers ────────────────────────────────────────────

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

        // Shadow
        let shadow_radius = radius;
        let s = 1.0 - smoothstep(0.0, shadow_radius, dist);
        let shadow_noise = noise_water(world_xz * 3.0 + vec2<f32>(time * 0.1)) * 0.3;
        shadow = max(shadow, s * intensity * (0.25 + shadow_noise));

        // Ripples
        let falloff = smoothstep(radius * 6.0, radius * 0.3, dist);
        if falloff > 0.001 {
            let noise_uv = to_obj * 1.5 + vec2<f32>(time * 0.8, time * 0.6);
            let angle_noise = (noise_water(noise_uv) - 0.5) * 0.5;
            let dist_warped = dist + angle_noise;

            let phase = f32(i) * 1.7;
            let w1 = sin(dist_warped * 10.0 - time * 6.0 + phase) * 0.45;
            let w2 = sin(dist_warped * 6.5 - time * 4.5 + phase + 2.0) * 0.3;
            let w3 = sin(dist_warped * 15.0 - time * 8.0 + phase + 4.5) * 0.15;

            let pulse = 0.7 + 0.3 * sin(time * 2.0 + phase);
            let wave_sum = (w1 + w2 + w3) * pulse;
            let energy = 1.0 / (1.0 + dist * dist * 0.2);

            let break_uv = world_xz * 6.0 + vec2<f32>(time * 1.2, -time * 0.8);
            let break_noise = noise_water(break_uv) * 0.7 + 0.3;
            let modulated = wave_sum * energy * falloff * break_noise;

            ripple += modulated * intensity * 0.2;
        }

        // Foam ring
        let foam_inner = smoothstep(radius * 0.15, radius * 0.5, dist);
        let foam_outer = smoothstep(radius * 1.5, radius * 0.7, dist);
        let foam_ring = foam_inner * foam_outer;
        if foam_ring > 0.001 {
            let fn1 = noise_water(world_xz * 6.0 + vec2<f32>(time * 0.4, time * 0.3));
            let fn2 = noise_water(world_xz * 12.0 + vec2<f32>(-time * 0.3, time * 0.5));
            let foam_pattern = fn1 * 0.6 + fn2 * 0.4;
            let foam_val = foam_ring * smoothstep(0.3, 0.6, foam_pattern) * intensity;
            foam = max(foam, foam_val * 0.7);
        }
    }

    return vec3<f32>(clamp(ripple, -0.15, 0.15), shadow, foam);
}

// ── Fragment shader ───────────────────────────────────────────────────────

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let world_xz = in.world_position.xz;

    // ── Distance from camera ──
    let camera_pos = view_bindings::view.world_position.xyz;
    let frag_dist = distance(camera_pos, in.world_position.xyz);
    let dist_factor = clamp(frag_dist / 150.0, 0.0, 1.0);

    // ── Normal: Gerstner + detail with distance LOD ──
    let gerstner_N = normalize(in.world_normal);
    let detail_blend = 0.35 * (1.0 - smoothstep(50.0, 120.0, frag_dist));
    var N: vec3<f32>;
    if detail_blend > 0.01 {
        let detail_N = detail_normal_multi(world_xz, water.time);
        N = normalize(mix(gerstner_N, detail_N, detail_blend));
    } else {
        N = gerstner_N;
    }

    let V = pbr_functions::calculate_view(in.world_position, false);
    let N_dot_V = max(dot(N, V), 0.001);
    let L = normalize(-water.sun_direction.xyz);
    let sun_intensity = water.sun_direction.w;

    // ── Wave slope for whitecap detection ──
    let wave_slope = 1.0 - dot(gerstner_N, vec3<f32>(0.0, 1.0, 0.0));
    let crest_height = in.world_position.y;

    // ── Screen UV ──
    let screen_uv = world_to_screen_uv(in.world_position);

    // ── Scene depth ──
#ifdef DEPTH_PREPASS
    let raw_depth = bevy_pbr::prepass_utils::prepass_depth(in.position, 0u);
    let scene_depth = linearize_depth(raw_depth);
    let water_depth_linear = linearize_depth(in.position.z);
    let water_depth = max(scene_depth - water_depth_linear, 0.0);
    let depth_factor = clamp(water_depth / water.max_depth, 0.0, 1.0);
#else
    let water_depth = 2.0;
    let depth_factor = 0.5;
#endif

    // ── Beer's law absorption (per-channel) ──
    let absorption_coeffs = water.absorption_rgb.rgb;
    let beer = exp(-absorption_coeffs * water_depth);

    // ── Screen-space refraction ──
    let refraction_offset = N.xz * water.refraction_strength * (1.0 + water_depth * 0.5);
    let refracted_uv = clamp(screen_uv + refraction_offset, vec2<f32>(0.001), vec2<f32>(0.999));

    let scene_color = textureSampleLevel(
        view_bindings::view_transmission_texture,
        view_bindings::view_transmission_sampler,
        refracted_uv,
        0.0
    ).rgb;
    let scene_adjusted = scene_color / view_bindings::view.exposure;

    // ── Water body color (refraction) ──
    let depth_curve = pow(depth_factor, 0.6);
    let water_tint = mix(water.shallow_color.rgb, water.deep_color.rgb, depth_curve);
    let refracted_color = scene_adjusted * beer + water_tint * (1.0 - beer) * 1.4;

    // ── Caustics on underwater scene ──
    let caustic = caustic_pattern(world_xz * 0.1, water.time) * water.caustic_intensity;
    let caustic_contribution = caustic * beer * (1.0 - depth_factor * 0.5);

    // ── PBR lighting — shadows, environment map, all lights ──
    var pbr = pbr_input_new();
    pbr.material.base_color = vec4<f32>(water_tint, 1.0);
    pbr.material.metallic = 0.0;
    pbr.material.perceptual_roughness = water.roughness;
    pbr.world_normal = N;
    pbr.world_position = in.world_position;
    pbr.N = N;
    pbr.V = V;
    var pbr_result = pbr_functions::apply_pbr_lighting(pbr);
    let pbr_color = pbr_result.rgb;

    // ── Fresnel ──
    let fresnel_base = fresnel_schlick(N_dot_V, 0.02);
    let fresnel = mix(fresnel_base, 1.0, dist_factor * dist_factor * 0.7);

    // ── Combine: refraction underneath + PBR reflection on top via Fresnel ──
    var water_color = mix(
        refracted_color + caustic_contribution,
        pbr_color,
        fresnel
    );

    // ── Subsurface scattering ──
    let sss_dot = max(dot(V, -L), 0.0);
    let sss = water.subsurface_strength * pow(sss_dot, 4.0) * water.shallow_color.rgb * 0.4 * sun_intensity;
    water_color += sss;

    // ── Multi-type foam system (world-space) ──
    // Use PBR luminance to approximate shadow/light on foam
    let pbr_luma = dot(pbr_color, vec3<f32>(0.299, 0.587, 0.114));
    let foam_light = clamp(pbr_luma * 2.0, 0.15, 1.0);
    let foam_lit = water.foam_color.rgb * foam_light;
    var total_foam = 0.0;

    let foam_dist_fade = 1.0 - smoothstep(60.0, 120.0, frag_dist);

    if foam_dist_fade > 0.01 {
        // (a) Whitecap foam — only on the very steepest, tallest crests
        let whitecap_trigger = smoothstep(0.4, 0.7, wave_slope)
                             * smoothstep(water.foam_threshold + 0.3, water.foam_threshold + 0.8, crest_height);
        if whitecap_trigger > 0.01 {
            let whitecap_tex = voronoi_foam(world_xz, water.time);
            total_foam += whitecap_trigger * whitecap_tex * 0.6;
        }

        // (b) Trailing foam — only just below the highest crests, very sparse
        let trail_trigger = smoothstep(water.foam_threshold + 0.1, water.foam_threshold + 0.6, crest_height);
        if trail_trigger > 0.01 {
            let trail_dissolve = noise_water(world_xz * 5.0 + vec2<f32>(water.time * 0.12, -water.time * 0.08));
            total_foam += trail_trigger * smoothstep(0.55, 0.8, trail_dissolve) * 0.2;
        }

        // (c) Shoreline foam — only near terrain
#ifdef DEPTH_PREPASS
        let foam_depth_threshold = water.absorption_rgb.w;

        // Contact line — bright foam at the water/terrain intersection
        let contact = 1.0 - smoothstep(0.0, 0.12, water_depth);
        let contact_noise = noise_water(world_xz * 20.0 + vec2<f32>(water.time * 0.5, water.time * 0.3));
        total_foam += contact * (0.6 + contact_noise * 0.3);

        // Wave lapping — animated foam bands at the shore
        if water_depth < foam_depth_threshold * 2.0 {
            let shore_grad = smoothstep(0.0, foam_depth_threshold * 2.0, water_depth);
            let lap_phase = water_depth * 8.0 - water.time * 2.0;
            let lap1 = smoothstep(0.3, 0.6, sin(lap_phase)) * (1.0 - shore_grad);
            let lap2 = smoothstep(0.4, 0.7, sin(lap_phase * 1.7 + 1.5)) * (1.0 - shore_grad) * 0.5;
            let lap_noise = noise_water(world_xz * 10.0 + vec2<f32>(water.time * 0.2, -water.time * 0.15));
            total_foam += (lap1 + lap2) * smoothstep(0.3, 0.6, lap_noise) * 0.5;
        }
#endif
    }

    total_foam = clamp(total_foam * foam_dist_fade, 0.0, 1.0);
    water_color = mix(water_color, foam_lit, total_foam);

    // ── Micro sparkles (world-space, distance-faded) ──
    let sp_coord = world_xz * 30.0 + vec2<f32>(water.time * 2.5, water.time * 2.0);
    let sp = noise_water(sp_coord);
    let sparkle = pow(sp, 24.0) * 0.8 * sun_intensity;
    let sparkle_mask = smoothstep(0.3, 0.8, N.y) * (1.0 - dist_factor);
    water_color += vec3<f32>(1.0, 0.97, 0.9) * sparkle * sparkle_mask;

    // ── Object interactions ──
    let fx = object_effects(world_xz, water.time);
    water_color += water_color * fx.x;
    water_color *= 1.0 - fx.y;
    water_color = mix(water_color, foam_lit, fx.z);

    // ── Horizon blend — fade toward PBR reflection at distance ──
    let haze_amount = pow(dist_factor, 2.5) * 0.5;
    water_color = mix(water_color, pbr_color, haze_amount);

    // ── Depth-based opacity ──
#ifdef DEPTH_PREPASS
    let edge_fade = smoothstep(0.0, 0.4, water_depth);
    let depth_opacity = mix(0.75, 1.0, depth_factor);
    let foam_opacity_boost = total_foam * 0.2;
    let alpha = clamp(edge_fade * depth_opacity + foam_opacity_boost, 0.0, 1.0);
#else
    let alpha = 0.92;
#endif

    return vec4<f32>(water_color, alpha);
}
