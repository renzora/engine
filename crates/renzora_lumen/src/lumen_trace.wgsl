// Phase 6: voxel-cache GI trace with temporal accumulation.
//
// Pipeline per pixel:
//   1. Read depth + normal. Sky → pass scene through, clear history.
//   2. Reconstruct world position; trace N cosine-weighted hemisphere
//      rays through the voxel clipmap. First voxel with alpha > 0.5
//      counts as a hit.
//   3. Sample motion vectors → reproject UV to where this surface was
//      last frame; sample history there. Reject if off-screen or the
//      stored linear depth disagrees with the current pixel's depth.
//   4. Blend current trace with valid history.
//   5. Output 0: scene + blended indirect (composite).
//      Output 1: blended indirect + current linear depth → next-frame
//                history.
//
// Bevy's motion-vector convention (from `bevy_pbr/.../prepass.wgsl`):
//   `motion_vector = (clip - prev_clip) * vec2(0.5, -0.5)`. So
//   `history_uv = uv - motion_vector`.

#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput
#import bevy_render::view::View

struct CascadeData {
    origin: vec3<f32>,
    voxel_size: f32,
};

struct VoxelGrid {
    cascades: array<CascadeData, 2>,
    resolution: u32,
    cascade_count: u32,
    _pad0: u32,
    _pad1: u32,
};

struct TraceConfig {
    intensity: f32,
    frame_count: u32,
    debug_mode: u32, // 0 = composite, 1 = indirect-only
    _pad0: u32,
};

@group(0) @binding(0) var scene_tex: texture_2d<f32>;
@group(0) @binding(1) var scene_sampler: sampler;
@group(0) @binding(2) var depth_tex: texture_depth_2d;
@group(0) @binding(3) var normal_tex: texture_2d<f32>;
@group(0) @binding(4) var voxels: texture_3d<f32>;
@group(0) @binding(5) var voxel_sampler: sampler;
@group(0) @binding(6) var history_tex: texture_2d<f32>;
@group(0) @binding(7) var motion_tex: texture_2d<f32>;
@group(0) @binding(8) var<uniform> view: View;
@group(0) @binding(9) var<uniform> grid: VoxelGrid;
@group(0) @binding(10) var<uniform> config: TraceConfig;

const SAMPLES: u32 = 2u;
const MAX_STEPS: u32 = 20u;
const HIT_ALPHA: f32 = 0.5;
// Push the ray origin a full voxel along the normal so the very first
// march step doesn't immediately self-hit the surface voxel.
const NORMAL_BIAS: f32 = 1.5;
const PI: f32 = 3.14159265359;

// Per-sample luminance clamp suppresses fireflies — a single ray hitting
// a very bright voxel can dominate the 2-sample average and become a
// persistent bright dot once temporal kicks in.
const MAX_SAMPLE_LUMINANCE: f32 = 4.0;
// 0.08 = 8% current / 92% history → ~12-frame half-life. Slow accumulation
// kills the noise hard; the cost is response lag for moving lights.
const TEMPORAL_ALPHA: f32 = 0.08;
// View-space linear-depth delta beyond which the reprojected pixel is
// treated as a different surface and history is dropped.
const DEPTH_DISOCCLUSION: f32 = 0.5;

fn uv_to_ndc(uv: vec2<f32>) -> vec2<f32> {
    return uv * vec2<f32>(2.0, -2.0) + vec2<f32>(-1.0, 1.0);
}

fn world_pos_from_depth(uv: vec2<f32>, depth: f32) -> vec3<f32> {
    let ndc = vec3<f32>(uv_to_ndc(uv), depth);
    let world_h = view.world_from_clip * vec4<f32>(ndc, 1.0);
    return world_h.xyz / world_h.w;
}

fn view_pos_from_depth(uv: vec2<f32>, depth: f32) -> vec3<f32> {
    let ndc = vec3<f32>(uv_to_ndc(uv), depth);
    let view_h = view.view_from_clip * vec4<f32>(ndc, 1.0);
    return view_h.xyz / view_h.w;
}

fn hash(seed: u32) -> u32 {
    var s = seed * 747796405u + 2891336453u;
    let word = ((s >> ((s >> 28u) + 4u)) ^ s) * 277803737u;
    return (word >> 22u) ^ word;
}

fn rand(seed: u32) -> f32 {
    return f32(hash(seed)) / 4294967296.0;
}

fn hemisphere_dir(n: vec3<f32>, seed: u32) -> vec3<f32> {
    let r1 = rand(seed);
    let r2 = rand(seed + 1u);
    let phi = 2.0 * PI * r1;
    let cos_theta = sqrt(1.0 - r2);
    let sin_theta = sqrt(r2);
    let local = vec3<f32>(cos(phi) * sin_theta, sin(phi) * sin_theta, cos_theta);

    let up = select(vec3<f32>(0.0, 1.0, 0.0), vec3<f32>(1.0, 0.0, 0.0), abs(n.y) > 0.99);
    let t = normalize(cross(up, n));
    let b = cross(n, t);
    return normalize(t * local.x + b * local.y + n * local.z);
}

fn select_cascade(p: vec3<f32>) -> i32 {
    let res = f32(grid.resolution);
    for (var c: u32 = 0u; c < grid.cascade_count; c = c + 1u) {
        let cascade = grid.cascades[c];
        let local = (p - cascade.origin) / cascade.voxel_size;
        if (all(local >= vec3<f32>(0.0)) && all(local < vec3<f32>(res))) {
            return i32(c);
        }
    }
    return -1;
}

fn cascade_voxel_load(p: vec3<f32>, cascade: u32) -> vec4<f32> {
    let info = grid.cascades[cascade];
    let local = (p - info.origin) / info.voxel_size;
    let idx_local = vec3<i32>(local);
    let idx = vec3<i32>(idx_local.x, idx_local.y, idx_local.z + i32(cascade * grid.resolution));
    return textureLoad(voxels, idx, 0);
}

fn trace_voxel_ray(origin: vec3<f32>, dir: vec3<f32>) -> vec3<f32> {
    var p = origin;
    for (var i: u32 = 0u; i < MAX_STEPS; i = i + 1u) {
        let cascade = select_cascade(p);
        if (cascade < 0) { return vec3<f32>(0.0); }
        let voxel = cascade_voxel_load(p, u32(cascade));
        if (voxel.a > HIT_ALPHA) {
            return voxel.rgb;
        }
        p = p + dir * grid.cascades[cascade].voxel_size;
    }
    return vec3<f32>(0.0);
}

struct FragOut {
    @location(0) composite: vec4<f32>,
    @location(1) history: vec4<f32>,
};

@fragment
fn fragment(in: FullscreenVertexOutput) -> FragOut {
    let scene = textureSample(scene_tex, scene_sampler, in.uv);
    let pixel = vec2<i32>(in.position.xy);
    let depth = textureLoad(depth_tex, pixel, 0);

    var out: FragOut;
    if (depth <= 0.0) {
        // Sky: pass scene through, clear history at this pixel.
        if (config.debug_mode == 1u) {
            out.composite = vec4<f32>(0.0, 0.0, 0.0, 1.0);
        } else {
            out.composite = scene;
        }
        out.history = vec4<f32>(0.0, 0.0, 0.0, 0.0);
        return out;
    }

    let world_pos = world_pos_from_depth(in.uv, depth);
    let view_pos = view_pos_from_depth(in.uv, depth);
    let normal_world = normalize(textureLoad(normal_tex, pixel, 0).xyz * 2.0 - 1.0);

    let inner_voxel_size = grid.cascades[0].voxel_size;
    let origin = world_pos + normal_world * (NORMAL_BIAS * inner_voxel_size);

    let seed_base =
        u32(pixel.x) * 1973u + u32(pixel.y) * 9277u + config.frame_count * 26699u;

    var indirect = vec3<f32>(0.0);
    for (var i: u32 = 0u; i < SAMPLES; i = i + 1u) {
        let dir = hemisphere_dir(normal_world, seed_base + i * 31u);
        var hit_rgb = trace_voxel_ray(origin, dir);
        // Per-sample luminance clamp: scale (not clip) so color is
        // preserved while bounding contribution.
        let lum = max(max(hit_rgb.r, hit_rgb.g), hit_rgb.b);
        let scale = min(1.0, MAX_SAMPLE_LUMINANCE / max(lum, 1e-4));
        indirect = indirect + hit_rgb * scale;
    }
    indirect = indirect / f32(SAMPLES);

    let current_linear_depth = view_pos.z;
    let motion_vector = textureLoad(motion_tex, pixel, 0).rg;
    let history_uv = in.uv - motion_vector;

    var blended_indirect: vec3<f32>;
    if (history_uv.x < 0.0 || history_uv.x > 1.0 || history_uv.y < 0.0 || history_uv.y > 1.0) {
        blended_indirect = indirect;
    } else {
        let history = textureSampleLevel(history_tex, scene_sampler, history_uv, 0.0);
        let history_indirect = history.rgb;
        let history_depth = history.a;
        let depth_delta = abs(current_linear_depth - history_depth);
        // history_depth >= 0.0 means "no surface last frame" (sky branch
        // writes 0.0; view-space Z on real surfaces is negative in Bevy).
        if (history_depth >= 0.0 || depth_delta > DEPTH_DISOCCLUSION) {
            blended_indirect = indirect;
        } else {
            blended_indirect = mix(history_indirect, indirect, TEMPORAL_ALPHA);
        }
    }

    let scaled_indirect = blended_indirect * config.intensity;
    if (config.debug_mode == 1u) {
        out.composite = vec4<f32>(scaled_indirect, 1.0);
    } else {
        out.composite = vec4<f32>(scene.rgb + scaled_indirect, scene.a);
    }
    out.history = vec4<f32>(blended_indirect, current_linear_depth);
    return out;
}
