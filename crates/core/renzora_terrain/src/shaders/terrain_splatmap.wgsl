// Terrain Splatmap Material — PBR-lit blending of up to 8 procedural layers
// Supports dual splatmaps (layers 0-3 and 4-7) and animated layers.
//
// Animation types:
//   0 = solid (tiled color)
//   1 = grass (wind sway + color variation)
//   2 = water (flow animation + ripples)
//   3 = rock (multi-octave noise)
//   4 = sand (ripple dunes)
//   5 = snow (sparkle + drift)
//   6 = dirt (earthy noise)

#import bevy_pbr::forward_io::VertexOutput
#import bevy_pbr::pbr_functions
#import bevy_pbr::pbr_types::{PbrInput, pbr_input_new}
#import bevy_pbr::mesh_view_bindings::globals

struct LayerColor {
    v0: vec4<f32>,
    v1: vec4<f32>,
    v2: vec4<f32>,
    v3: vec4<f32>,
};

struct LayerProps {
    v0: vec4<f32>,
    v1: vec4<f32>,
    v2: vec4<f32>,
    v3: vec4<f32>,
};

struct LayerInfo {
    layer_count: u32,
    _pad0: u32,
    _pad1: u32,
    _pad2: u32,
};

// Layers 0-3
@group(3) @binding(0) var<uniform> layer_colors_a: LayerColor;
@group(3) @binding(1) var<uniform> layer_props_a: LayerProps;
@group(3) @binding(2) var splatmap_a_texture: texture_2d<f32>;
@group(3) @binding(3) var splatmap_sampler: sampler;

// Layers 4-7
@group(3) @binding(4) var<uniform> layer_colors_b: LayerColor;
@group(3) @binding(5) var<uniform> layer_props_b: LayerProps;
@group(3) @binding(6) var splatmap_b_texture: texture_2d<f32>;

// Layer count
@group(3) @binding(7) var<uniform> layer_info: LayerInfo;

// Layer texture arrays
struct LayerTexFlags {
    flags: u32,
    _pad: vec3<u32>,
};

@group(3) @binding(8)  var layer_albedo_array: texture_2d_array<f32>;
@group(3) @binding(9)  var layer_tex_sampler: sampler;
@group(3) @binding(10) var layer_normal_array: texture_2d_array<f32>;
@group(3) @binding(11) var layer_arm_array: texture_2d_array<f32>;
@group(3) @binding(12) var<uniform> layer_tex_flags: LayerTexFlags;

fn layer_has_albedo(idx: u32) -> bool {
    return (layer_tex_flags.flags & (1u << idx)) != 0u;
}

fn layer_has_normal(idx: u32) -> bool {
    return (layer_tex_flags.flags & (1u << (idx + 8u))) != 0u;
}

fn layer_has_arm(idx: u32) -> bool {
    return (layer_tex_flags.flags & (1u << (idx + 16u))) != 0u;
}

// ── Noise functions ────────────────────────────────────────────────────────

fn hash2(p: vec2<f32>) -> f32 {
    let h = dot(p, vec2<f32>(127.1, 311.7));
    return fract(sin(h) * 43758.5453123);
}

fn noise2d(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);

    let a = hash2(i);
    let b = hash2(i + vec2<f32>(1.0, 0.0));
    let c = hash2(i + vec2<f32>(0.0, 1.0));
    let d = hash2(i + vec2<f32>(1.0, 1.0));

    return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}

fn fbm2(p: vec2<f32>, octaves: i32) -> f32 {
    var value = 0.0;
    var amplitude = 0.5;
    var pos = p;
    for (var i = 0; i < octaves; i++) {
        value += amplitude * noise2d(pos);
        pos *= 2.0;
        amplitude *= 0.5;
    }
    return value;
}

fn voronoi(p: vec2<f32>) -> f32 {
    let n = floor(p);
    let f = fract(p);
    var min_dist = 1.0;
    for (var j = -1; j <= 1; j++) {
        for (var i = -1; i <= 1; i++) {
            let neighbor = vec2<f32>(f32(i), f32(j));
            let point = vec2<f32>(hash2(n + neighbor), hash2(n + neighbor + vec2<f32>(37.0, 17.0)));
            let diff = neighbor + point - f;
            min_dist = min(min_dist, dot(diff, diff));
        }
    }
    return sqrt(min_dist);
}

// ── Layer shading functions ────────────────────────────────────────────────

fn shade_solid(base_color: vec3<f32>, world_pos: vec3<f32>, uv_scale: f32) -> vec3<f32> {
    return base_color;
}

fn shade_grass(base_color: vec3<f32>, world_pos: vec3<f32>, uv_scale: f32, time: f32, speed: f32) -> vec3<f32> {
    let uv = world_pos.xz * uv_scale;
    let wind_large = sin(uv.x * 0.3 + time * speed * 0.8) * 0.5 + 0.5;
    let wind_small = sin(uv.x * 1.5 + uv.y * 0.7 + time * speed * 1.5) * 0.5 + 0.5;
    let wind = wind_large * 0.6 + wind_small * 0.4;
    let patch_noise = fbm2(uv * 0.4, 3);
    let detail_noise = noise2d(uv * 2.0);
    let warm_green = vec3<f32>(0.35, 0.55, 0.12);
    let cool_green = vec3<f32>(0.15, 0.45, 0.15);
    let dry_green = vec3<f32>(0.45, 0.50, 0.18);
    var color = mix(cool_green, warm_green, patch_noise);
    color = mix(color, dry_green, detail_noise * 0.3);
    color *= base_color * 2.5;
    color += vec3<f32>(0.06, 0.08, 0.02) * wind;
    let blade = sin(uv.x * 15.0 + wind * 2.0) * sin(uv.y * 12.0) * 0.5 + 0.5;
    color = mix(color * 0.85, color * 1.1, blade * 0.3);
    return color;
}

fn shade_water(base_color: vec3<f32>, world_pos: vec3<f32>, uv_scale: f32, time: f32, speed: f32) -> vec3<f32> {
    let uv = world_pos.xz * uv_scale;
    let flow_uv = uv + vec2<f32>(time * speed * 0.15, time * speed * 0.08);
    let wave1 = sin(flow_uv.x * 2.0 + flow_uv.y * 1.5 + time * speed * 0.5) * 0.5 + 0.5;
    let wave2 = sin(flow_uv.x * 1.2 - flow_uv.y * 2.3 + time * speed * 0.7) * 0.5 + 0.5;
    let ripple1 = sin(uv.x * 8.0 + time * speed * 2.0) * sin(uv.y * 6.0 + time * speed * 1.5);
    let ripple2 = sin(uv.x * 5.0 - time * speed * 1.8) * sin(uv.y * 9.0 + time * speed * 2.2);
    let ripples = (ripple1 + ripple2) * 0.25 + 0.5;
    let caustic_uv = uv * 3.0 + vec2<f32>(time * speed * 0.1, time * speed * 0.05);
    let caustics = voronoi(caustic_uv) * voronoi(caustic_uv * 1.3 + vec2<f32>(time * speed * 0.07));
    let deep = vec3<f32>(0.05, 0.15, 0.35);
    let shallow = vec3<f32>(0.1, 0.35, 0.45);
    let foam = vec3<f32>(0.6, 0.7, 0.75);
    var color = mix(deep, shallow, wave1 * 0.6 + wave2 * 0.4);
    color *= base_color * 2.0;
    color += vec3<f32>(0.15, 0.2, 0.1) * caustics * 0.6;
    color += vec3<f32>(0.3, 0.35, 0.4) * ripples * 0.15;
    let foam_mask = smoothstep(0.6, 0.8, fbm2(flow_uv * 2.0, 3));
    color = mix(color, foam, foam_mask * 0.3);
    return color;
}

fn shade_rock(base_color: vec3<f32>, world_pos: vec3<f32>, uv_scale: f32, time: f32, speed: f32) -> vec3<f32> {
    let uv = world_pos.xz * uv_scale;
    let n1 = fbm2(uv * 1.0, 4);
    let n2 = fbm2(uv * 3.0 + vec2<f32>(100.0, 200.0), 3);
    let n3 = noise2d(uv * 8.0);
    let cracks = voronoi(uv * 2.5);
    let crack_mask = smoothstep(0.0, 0.08, cracks);
    let dark_rock = vec3<f32>(0.25, 0.22, 0.20);
    let light_rock = vec3<f32>(0.50, 0.47, 0.42);
    let moss = vec3<f32>(0.20, 0.30, 0.15);
    var color = mix(dark_rock, light_rock, n1);
    color = mix(color, color * 0.7, (1.0 - crack_mask) * 0.5);
    color += vec3<f32>(0.05) * n2;
    color = mix(color, moss, smoothstep(0.55, 0.7, n1) * 0.3);
    color *= 0.85 + n3 * 0.3;
    color *= base_color * 2.2;
    return color;
}

fn shade_sand(base_color: vec3<f32>, world_pos: vec3<f32>, uv_scale: f32, time: f32, speed: f32) -> vec3<f32> {
    let uv = world_pos.xz * uv_scale;
    let ripple = sin(uv.x * 6.0 + uv.y * 2.0 + sin(uv.y * 0.5) * 3.0) * 0.5 + 0.5;
    let fine_grain = noise2d(uv * 20.0);
    let medium = fbm2(uv * 2.0, 3);
    let wind_shift = sin(time * speed * 0.3) * 0.02;
    let wind_ripple = sin((uv.x + wind_shift) * 8.0 + uv.y * 1.5) * 0.5 + 0.5;
    let warm_sand = vec3<f32>(0.76, 0.70, 0.50);
    let cool_sand = vec3<f32>(0.65, 0.58, 0.42);
    var color = mix(cool_sand, warm_sand, ripple * 0.5 + medium * 0.3);
    color *= base_color * 2.0;
    color *= 0.92 + fine_grain * 0.16;
    color += vec3<f32>(0.03, 0.03, 0.01) * wind_ripple;
    return color;
}

fn shade_snow(base_color: vec3<f32>, world_pos: vec3<f32>, uv_scale: f32, time: f32, speed: f32) -> vec3<f32> {
    let uv = world_pos.xz * uv_scale;
    let drift = fbm2(uv * 1.5, 3);
    let sparkle_pattern = noise2d(uv * 30.0 + vec2<f32>(time * speed * 0.5));
    let surface = fbm2(uv * 4.0, 2);
    let sparkle = smoothstep(0.85, 0.95, sparkle_pattern) * (sin(time * speed * 3.0) * 0.5 + 0.5);
    let shadow_snow = vec3<f32>(0.75, 0.80, 0.90);
    let bright_snow = vec3<f32>(0.95, 0.95, 0.97);
    var color = mix(shadow_snow, bright_snow, drift * 0.6 + surface * 0.3);
    color *= base_color * 1.5;
    color += vec3<f32>(1.0, 1.0, 0.95) * sparkle * 0.4;
    return color;
}

fn shade_dirt(base_color: vec3<f32>, world_pos: vec3<f32>, uv_scale: f32, time: f32, speed: f32) -> vec3<f32> {
    let uv = world_pos.xz * uv_scale;
    let large_patch = fbm2(uv * 0.8, 4);
    let medium_detail = fbm2(uv * 3.0, 3);
    let fine = noise2d(uv * 12.0);
    let pebble = voronoi(uv * 5.0);
    let pebble_mask = smoothstep(0.1, 0.2, pebble);
    let dark_dirt = vec3<f32>(0.30, 0.22, 0.14);
    let light_dirt = vec3<f32>(0.50, 0.40, 0.28);
    let clay = vec3<f32>(0.55, 0.35, 0.20);
    var color = mix(dark_dirt, light_dirt, large_patch);
    color = mix(color, clay, medium_detail * 0.3);
    color *= base_color * 2.2;
    color *= 0.88 + fine * 0.24;
    color = mix(color * 0.85, color * 1.1, pebble_mask * 0.3);
    return color;
}

// ── Main layer dispatch ────────────────────────────────────────────────────

fn eval_layer(layer_color: vec4<f32>, layer_prop: vec4<f32>, world_pos: vec3<f32>, time: f32, layer_idx: u32) -> vec3<f32> {
    let uv_scale = layer_color.a;

    // If this layer has an albedo texture, sample it instead of procedural
    if layer_has_albedo(layer_idx) {
        let tex_uv = world_pos.xz * uv_scale;
        return textureSample(layer_albedo_array, layer_tex_sampler, tex_uv, layer_idx).rgb;
    }

    let base_color = layer_color.rgb;
    let anim_type = i32(layer_prop.z);
    let anim_speed = layer_prop.w;

    switch anim_type {
        case 1: { return shade_grass(base_color, world_pos, uv_scale, time, anim_speed); }
        case 2: { return shade_water(base_color, world_pos, uv_scale, time, anim_speed); }
        case 3: { return shade_rock(base_color, world_pos, uv_scale, time, anim_speed); }
        case 4: { return shade_sand(base_color, world_pos, uv_scale, time, anim_speed); }
        case 5: { return shade_snow(base_color, world_pos, uv_scale, time, anim_speed); }
        case 6: { return shade_dirt(base_color, world_pos, uv_scale, time, anim_speed); }
        default: { return shade_solid(base_color, world_pos, uv_scale); }
    }
}

fn eval_layer_pbr(layer_prop: vec4<f32>, world_pos: vec3<f32>, uv_scale: f32, layer_idx: u32) -> vec2<f32> {
    // If this layer has an ARM texture, sample it (G=roughness, B=metallic)
    if layer_has_arm(layer_idx) {
        let tex_uv = world_pos.xz * uv_scale;
        let arm = textureSample(layer_arm_array, layer_tex_sampler, tex_uv, layer_idx);
        return vec2<f32>(arm.b, arm.g); // metallic, roughness
    }
    return vec2<f32>(layer_prop.x, layer_prop.y);
}

fn get_color_a(idx: i32) -> vec4<f32> {
    switch idx {
        case 0: { return layer_colors_a.v0; }
        case 1: { return layer_colors_a.v1; }
        case 2: { return layer_colors_a.v2; }
        case 3: { return layer_colors_a.v3; }
        default: { return vec4<f32>(0.5, 0.5, 0.5, 0.1); }
    }
}

fn get_props_a(idx: i32) -> vec4<f32> {
    switch idx {
        case 0: { return layer_props_a.v0; }
        case 1: { return layer_props_a.v1; }
        case 2: { return layer_props_a.v2; }
        case 3: { return layer_props_a.v3; }
        default: { return vec4<f32>(0.0, 0.5, 0.0, 0.0); }
    }
}

fn get_color_b(idx: i32) -> vec4<f32> {
    switch idx {
        case 0: { return layer_colors_b.v0; }
        case 1: { return layer_colors_b.v1; }
        case 2: { return layer_colors_b.v2; }
        case 3: { return layer_colors_b.v3; }
        default: { return vec4<f32>(0.5, 0.5, 0.5, 0.1); }
    }
}

fn get_props_b(idx: i32) -> vec4<f32> {
    switch idx {
        case 0: { return layer_props_b.v0; }
        case 1: { return layer_props_b.v1; }
        case 2: { return layer_props_b.v2; }
        case 3: { return layer_props_b.v3; }
        default: { return vec4<f32>(0.0, 0.5, 0.0, 0.0); }
    }
}

fn get_weight_a(weights: vec4<f32>, idx: i32) -> f32 {
    switch idx {
        case 0: { return weights.r; }
        case 1: { return weights.g; }
        case 2: { return weights.b; }
        case 3: { return weights.a; }
        default: { return 0.0; }
    }
}

// ── Fragment entry ─────────────────────────────────────────────────────────

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    // Sample both splatmaps
    let weights_a = textureSample(splatmap_a_texture, splatmap_sampler, in.uv);
    let weights_b = textureSample(splatmap_b_texture, splatmap_sampler, in.uv);

    let count = i32(layer_info.layer_count);
    let time = globals.time;
    let world_pos = in.world_position.xyz;

    // Compute total weight for normalization
    var total_weight = 0.0;
    for (var i = 0; i < count; i++) {
        if i < 4 {
            total_weight += get_weight_a(weights_a, i);
        } else {
            total_weight += get_weight_a(weights_b, i - 4);
        }
    }
    total_weight = max(total_weight, 0.001);

    // Blend all active layers
    var blended_color = vec3<f32>(0.0);
    var blended_metallic = 0.0;
    var blended_roughness = 0.0;
    var blended_normal = vec3<f32>(0.0, 0.0, 0.0);
    var has_tex_normal = false;

    for (var i = 0; i < count; i++) {
        var w: f32;
        var lc: vec4<f32>;
        var lp: vec4<f32>;

        if i < 4 {
            w = get_weight_a(weights_a, i) / total_weight;
            lc = get_color_a(i);
            lp = get_props_a(i);
        } else {
            w = get_weight_a(weights_b, i - 4) / total_weight;
            lc = get_color_b(i - 4);
            lp = get_props_b(i - 4);
        }

        if w > 0.001 {
            let layer_idx = u32(i);
            blended_color += eval_layer(lc, lp, world_pos, time, layer_idx) * w;
            let pbr = eval_layer_pbr(lp, world_pos, lc.a, layer_idx);
            blended_metallic += pbr.x * w;
            blended_roughness += pbr.y * w;

            // Normal mapping from texture array
            if layer_has_normal(layer_idx) {
                let tex_uv = world_pos.xz * lc.a;
                let n_sample = textureSample(layer_normal_array, layer_tex_sampler, tex_uv, layer_idx).rgb;
                let tangent_normal = n_sample * 2.0 - 1.0;
                blended_normal += tangent_normal * w;
                has_tex_normal = true;
            }
        }
    }

    // PBR lighting
    var N = normalize(in.world_normal);

    // Apply blended tangent-space normal if any textured layers contributed
    if has_tex_normal {
        let world_n = normalize(in.world_normal);
        // Build tangent frame from world normal (terrain is mostly Y-up)
        let up = select(vec3<f32>(0.0, 1.0, 0.0), vec3<f32>(1.0, 0.0, 0.0), abs(world_n.y) > 0.999);
        let T = normalize(cross(up, world_n));
        let B = cross(world_n, T);
        let tn = normalize(blended_normal);
        N = normalize(T * tn.x + B * tn.y + world_n * tn.z);
    }
    let V = pbr_functions::calculate_view(in.world_position, false);

    var pbr_input: PbrInput = pbr_input_new();
    pbr_input.material.base_color = vec4<f32>(blended_color, 1.0);
    pbr_input.material.metallic = blended_metallic;
    pbr_input.material.perceptual_roughness = blended_roughness;
    pbr_input.world_normal = N;
    pbr_input.world_position = in.world_position;
    pbr_input.N = N;
    pbr_input.V = V;

    var color = pbr_functions::apply_pbr_lighting(pbr_input);
    color.a = 1.0;
    return color;
}
