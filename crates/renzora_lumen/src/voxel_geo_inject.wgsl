// Geometry-sample inject — adds baked mesh sample points (one entry
// per sample = world position + albedo) into the same per-frame
// accumulation buffer the visible-surface pass writes to. Resolve
// pass averages everything together.

struct VoxelGrid {
    origin: vec3<f32>,
    voxel_size: f32,
    resolution: u32,
    _pad0: u32,
    _pad1: u32,
    _pad2: u32,
};

struct Sample {
    world_pos: vec4<f32>, // xyz + pad
    albedo: vec4<f32>,    // rgb + pad
};

@group(0) @binding(0) var<storage, read> samples: array<Sample>;
@group(0) @binding(1) var<storage, read_write> accum: array<atomic<u32>>;
@group(0) @binding(2) var<uniform> grid: VoxelGrid;

const FIXED_POINT_SCALE: f32 = 256.0;
// Geometry samples are pure base color (≤ 1.0 typically), so a tighter
// clamp than the visible-surface inject works fine here.
const MAX_CHANNEL: f32 = 4.0;

// Each geometry sample contributes WEIGHT entries to the accumulation
// buffer, so it doesn't get drowned out by hundreds of visible-surface
// pixels writing to the same voxel. Tunable — boost for "geometry
// dominates" debug, lower toward 1 for proportional averaging.
const GEO_WEIGHT: u32 = 20u;

@compute @workgroup_size(64, 1, 1)
fn inject(@builtin(global_invocation_id) gid: vec3<u32>) {
    let n = arrayLength(&samples);
    if (gid.x >= n) { return; }
    let s = samples[gid.x];

    let local = (s.world_pos.xyz - grid.origin) / grid.voxel_size;
    let idx = vec3<i32>(local);
    if (any(idx < vec3<i32>(0)) || any(idx >= vec3<i32>(i32(grid.resolution)))) {
        return;
    }

    let color = clamp(s.albedo.rgb, vec3<f32>(0.0), vec3<f32>(MAX_CHANNEL));
    let r_fp = u32(color.r * FIXED_POINT_SCALE);
    let g_fp = u32(color.g * FIXED_POINT_SCALE);
    let b_fp = u32(color.b * FIXED_POINT_SCALE);

    let res = grid.resolution;
    let voxel_idx = u32(idx.x) + u32(idx.y) * res + u32(idx.z) * res * res;
    let base = voxel_idx * 4u;

    atomicAdd(&accum[base], r_fp * GEO_WEIGHT);
    atomicAdd(&accum[base + 1u], g_fp * GEO_WEIGHT);
    atomicAdd(&accum[base + 2u], b_fp * GEO_WEIGHT);
    atomicAdd(&accum[base + 3u], GEO_WEIGHT);
}
