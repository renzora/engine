// Phase 5 V1: ray-trace the voxel cache for off-screen indirect light.
//
// Pipeline per pixel:
//   1. Read depth + normal. Sky → pass scene through.
//   2. Reconstruct world position + view-space normal.
//   3. Cosine-weighted hemisphere sample N rays around the normal.
//   4. For each ray, march through the voxel clipmap picking the
//      tightest-fit cascade at each step. First voxel with alpha > 0.5
//      counts as a hit; record its stored radiance.
//   5. Average hit radiance over the N rays (misses contribute 0),
//      scale by intensity, and add to the scene HDR additively.
//
// No temporal accumulation in this pass — single-frame output, so
// expect noise. Phase 6 task to layer the existing renzora_rt-style
// temporal blend onto this output.

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
@group(0) @binding(6) var<uniform> view: View;
@group(0) @binding(7) var<uniform> grid: VoxelGrid;
@group(0) @binding(8) var<uniform> config: TraceConfig;

// Drastically scaled-down V1 numbers — full ray budgets at 1080p
// caused USB-driver-crash levels of GPU stress on a 3060 + Bistro
// scene. Phase 6 temporal accumulation can let us recover quality
// while keeping per-frame work down; for now, prioritise stability.
const SAMPLES: u32 = 2u;
const MAX_STEPS: u32 = 20u;
const HIT_ALPHA: f32 = 0.5;
// Push the ray origin a full voxel along the normal so the very first
// march step doesn't immediately self-hit the surface voxel.
const NORMAL_BIAS: f32 = 1.5;
const PI: f32 = 3.14159265359;

fn uv_to_ndc(uv: vec2<f32>) -> vec2<f32> {
    return uv * vec2<f32>(2.0, -2.0) + vec2<f32>(-1.0, 1.0);
}

fn world_pos_from_depth(uv: vec2<f32>, depth: f32) -> vec3<f32> {
    let ndc = vec3<f32>(uv_to_ndc(uv), depth);
    let world_h = view.world_from_clip * vec4<f32>(ndc, 1.0);
    return world_h.xyz / world_h.w;
}

fn hash(seed: u32) -> u32 {
    var s = seed * 747796405u + 2891336453u;
    let word = ((s >> ((s >> 28u) + 4u)) ^ s) * 277803737u;
    return (word >> 22u) ^ word;
}

fn rand(seed: u32) -> f32 {
    return f32(hash(seed)) / 4294967296.0;
}

// Cosine-weighted hemisphere sample around `n`.
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

// Returns the tightest cascade (smallest voxel size) whose volume
// contains `p`, or -1 if p is outside the entire clipmap.
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

// March one ray through the cache; return the radiance of the first
// occupied voxel hit, or zero if no hit within MAX_STEPS.
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

@fragment
fn fragment(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let scene = textureSample(scene_tex, scene_sampler, in.uv);
    let pixel = vec2<i32>(in.position.xy);
    let depth = textureLoad(depth_tex, pixel, 0);
    if (depth <= 0.0) { return scene; }

    let world_pos = world_pos_from_depth(in.uv, depth);
    let normal_world = normalize(textureLoad(normal_tex, pixel, 0).xyz * 2.0 - 1.0);

    // Start the rays just off the surface so we don't immediately
    // self-hit on the very voxel we're sitting inside.
    let inner_voxel_size = grid.cascades[0].voxel_size;
    let origin = world_pos + normal_world * (NORMAL_BIAS * inner_voxel_size);

    let seed_base =
        u32(pixel.x) * 1973u + u32(pixel.y) * 9277u + config.frame_count * 26699u;

    var indirect = vec3<f32>(0.0);
    for (var i: u32 = 0u; i < SAMPLES; i = i + 1u) {
        let dir = hemisphere_dir(normal_world, seed_base + i * 31u);
        let hit_rgb = trace_voxel_ray(origin, dir);
        indirect = indirect + hit_rgb;
    }
    indirect = indirect / f32(SAMPLES);

    let scaled_indirect = indirect * config.intensity;
    if (config.debug_mode == 1u) {
        // Indirect-only debug: show just the trace contribution so we
        // can verify rays are actually hitting voxels.
        return vec4<f32>(scaled_indirect, 1.0);
    }
    return vec4<f32>(scene.rgb + scaled_indirect, scene.a);
}
