#import bevy_pbr::mesh_functions
#import bevy_pbr::view_transformations::position_world_to_clip
#import bevy_pbr::forward_io::VertexOutput
#import bevy_pbr::mesh_view_bindings as view_bindings
#import bevy_pbr::pbr_functions

// ── Material uniforms (group 3) ───────────────────────────────────────────────

struct PoolWaterUniforms {
    light_direction: vec4<f32>,
    deep_color: vec4<f32>,
    shallow_color: vec4<f32>,
    ior: f32,
    fresnel_min: f32,
    caustic_intensity: f32,
    time: f32,
    height_scale: f32,
    specular_power: f32,
    refraction_strength: f32,
    max_depth: f32,
    absorption: vec4<f32>,
    foam_color: vec4<f32>,
};

@group(3) @binding(0) var<uniform> params: PoolWaterUniforms;
@group(3) @binding(1) var heightfield_texture: texture_2d<f32>;
@group(3) @binding(2) var heightfield_sampler: sampler;

const PI: f32 = 3.14159265359;

// ── Noise ─────────────────────────────────────────────────────────────────────

fn hash_pw(p: vec2<f32>) -> f32 {
    var p3 = fract(vec3<f32>(p.x, p.y, p.x) * 0.1031);
    p3 += dot(p3, p3.yzx + 33.33);
    return fract((p3.x + p3.y) * p3.z);
}

fn noise_pw(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    return mix(
        mix(hash_pw(i), hash_pw(i + vec2<f32>(1.0, 0.0)), u.x),
        mix(hash_pw(i + vec2<f32>(0.0, 1.0)), hash_pw(i + vec2<f32>(1.0, 1.0)), u.x),
        u.y
    );
}

// Procedural caustic pattern — overlapping animated noise rings.
fn caustic_pattern(p: vec2<f32>, t: f32) -> f32 {
    let p1 = p * 8.0 + vec2<f32>(t * 0.3, t * 0.2);
    let p2 = p * 12.0 + vec2<f32>(-t * 0.2, t * 0.4);
    let p3 = p * 6.0 + vec2<f32>(t * 0.15, -t * 0.25);
    var c = 0.0;
    c += smoothstep(0.48, 0.52, noise_pw(p1)) * 0.5;
    c += smoothstep(0.46, 0.54, noise_pw(p2)) * 0.35;
    c += smoothstep(0.44, 0.50, noise_pw(p3)) * 0.25;
    return c;
}

// ── Depth utilities ───────────────────────────────────────────────────────────

// Linearize a depth buffer value to view-space depth.
fn linearize_depth(ndc_depth: f32) -> f32 {
    let near = view_bindings::view.clip_from_view[3][2];
    let far_factor = view_bindings::view.clip_from_view[2][2];
    return near / (far_factor + ndc_depth);
}

// Convert world position to screen UV [0,1].
fn world_to_screen_uv(world_pos: vec4<f32>) -> vec2<f32> {
    let clip = view_bindings::view.clip_from_world * world_pos;
    let ndc = clip.xy / clip.w;
    return ndc * vec2<f32>(0.5, -0.5) + 0.5;
}

// ── Vertex shader ─────────────────────────────────────────────────────────────

@vertex
fn vertex(
    @builtin(instance_index) instance_index: u32,
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
) -> VertexOutput {
    var out: VertexOutput;

    let world_from_local = mesh_functions::get_world_from_local(instance_index);

    // Sample heightfield at this vertex UV
    let info = textureSampleLevel(heightfield_texture, heightfield_sampler, uv, 0.0);

    // Displace Y by height
    var pos = position;
    pos.y += info.r * params.height_scale;

    let world_position = mesh_functions::mesh_position_local_to_world(
        world_from_local,
        vec4<f32>(pos, 1.0)
    );

    // Reconstruct normal from heightfield BA channels
    let ba = vec2<f32>(info.b, info.a);
    let wave_normal = vec3<f32>(ba.x, sqrt(max(0.0, 1.0 - dot(ba, ba))), ba.y);

    out.position = position_world_to_clip(world_position.xyz);
    out.world_position = world_position;
    out.world_normal = mesh_functions::mesh_normal_local_to_world(wave_normal, instance_index);
    out.uv = uv;

    return out;
}

// ── Fragment shader ───────────────────────────────────────────────────────────

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    // ── UV refinement loop (from webgpu-water) ──
    var uv = in.uv;
    var info = textureSampleLevel(heightfield_texture, heightfield_sampler, uv, 0.0);
    for (var i = 0; i < 5; i++) {
        uv += info.ba * 0.005;
        info = textureSampleLevel(heightfield_texture, heightfield_sampler, uv, 0.0);
    }

    // Reconstruct surface normal
    let ba = vec2<f32>(info.b, info.a);
    let local_normal = normalize(vec3<f32>(info.b, sqrt(max(0.0, 1.0 - dot(ba, ba))), info.a));
    let N = normalize(mix(in.world_normal, local_normal, 0.7));

    // View direction
    let V = normalize(view_bindings::view.world_position.xyz - in.world_position.xyz);
    let N_dot_V = max(dot(N, V), 0.001);

    // ── Screen UV of this fragment ──
    let screen_uv = world_to_screen_uv(in.world_position);

    // ── Scene depth → water depth ──
    // Read scene depth at this pixel from the depth prepass
#ifdef DEPTH_PREPASS
    let raw_depth = bevy_pbr::prepass_utils::prepass_depth(in.position, 0u);
    let scene_depth = linearize_depth(raw_depth);
    let water_depth_linear = linearize_depth(in.position.z);
    let water_depth = max(scene_depth - water_depth_linear, 0.0);
    let depth_factor = clamp(water_depth / params.max_depth, 0.0, 1.0);
#else
    // Fallback: no depth info, assume medium depth
    let water_depth = 2.0;
    let depth_factor = 0.5;
#endif

    // ── Beer's law absorption ──
    // Red absorbed first, green moderate, blue least
    let absorption = exp(-params.absorption.rgb * water_depth);

    // ── Screen-space refraction ──
    // Offset screen UV by surface normal for refraction distortion
    let refraction_offset = N.xz * params.refraction_strength * (1.0 + water_depth * 0.5);
    let refracted_uv = clamp(screen_uv + refraction_offset, vec2<f32>(0.001), vec2<f32>(0.999));

    // Sample the opaque scene behind the water (transmission texture)
    let scene_color = textureSampleLevel(
        view_bindings::view_transmission_texture,
        view_bindings::view_transmission_sampler,
        refracted_uv,
        0.0
    ).rgb;

    // Compensate for exposure
    let scene_adjusted = scene_color / view_bindings::view.exposure;

    // ── Refracted color: scene tinted by absorption ──
    let water_tint = mix(params.shallow_color.rgb, params.deep_color.rgb, depth_factor);
    let refracted_color = scene_adjusted * absorption + water_tint * (1.0 - absorption);

    // ── Caustics on the scene below ──
    let caustic = caustic_pattern(in.world_position.xz * 0.1, params.time) * params.caustic_intensity;
    let caustic_contribution = caustic * absorption * (1.0 - depth_factor * 0.5);

    // ── Fresnel ──
    let fresnel = mix(params.fresnel_min, 1.0, pow(1.0 - N_dot_V, 5.0));

    // ── Reflection ──
    let reflect_dir = reflect(-V, N);

    // Sky gradient reflection
    let sky_t = smoothstep(-0.1, 0.8, reflect_dir.y);
    var sky_color = mix(
        vec3<f32>(0.35, 0.45, 0.55),
        vec3<f32>(0.5, 0.7, 0.95),
        sky_t
    );

    // Sun specular highlight
    let L = normalize(-params.light_direction.xyz);
    let H = normalize(V + L);
    let sun_spec = pow(max(dot(N, H), 0.0), params.specular_power) * 5.0;
    sky_color += vec3<f32>(1.0, 0.95, 0.8) * sun_spec;

    // ── Subsurface scattering ──
    let sss_dot = max(dot(V, -L), 0.0);
    let sss = pow(sss_dot, 4.0) * params.shallow_color.rgb * 0.15;

    // ── Combine ──
    var final_color = mix(refracted_color + caustic_contribution, sky_color, fresnel);
    final_color += sss;

    // ── Shoreline foam (depth-based) ──
#ifdef DEPTH_PREPASS
    let foam_depth = params.absorption.w; // foam_depth stored in absorption.w
    let foam_factor = 1.0 - smoothstep(0.0, foam_depth, water_depth);
    let foam_noise = noise_pw(in.world_position.xz * 6.0 + vec2<f32>(params.time * 0.3, params.time * 0.2));
    let foam_dissolve = noise_pw(in.world_position.xz * 12.0 + vec2<f32>(-params.time * 0.15, params.time * 0.1));
    let foam = foam_factor * smoothstep(0.3, 0.5, foam_noise) * smoothstep(0.2, 0.6, foam_dissolve);
    final_color = mix(final_color, params.foam_color.rgb, clamp(foam * 0.8, 0.0, 1.0));
#endif

    // ── Micro sparkles ──
    let sp = noise_pw(in.world_position.xz * 50.0 + vec2<f32>(params.time * 3.0, params.time * 2.5));
    let sparkle = pow(sp, 12.0) * 1.5;
    let sparkle_mask = smoothstep(0.3, 0.8, N.y);
    final_color += vec3<f32>(1.0, 0.97, 0.9) * sparkle * sparkle_mask;

    // ── Edge fade (soft intersection with scene geometry) ──
#ifdef DEPTH_PREPASS
    let edge_alpha = smoothstep(0.0, 0.1, water_depth);
#else
    let edge_alpha = 1.0;
#endif

    return vec4<f32>(final_color, edge_alpha);
}
