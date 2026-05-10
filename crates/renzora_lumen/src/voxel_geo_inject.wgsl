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
    let base = voxel_idx * 5u;

    // Radiance contributions at 1× weight — the resolve pass already
    // averages sums by total_count, and occupancy is tracked separately
    // at slot [4u] so geometry doesn't need to "out-shout" visible
    // pixels to be visible in the debug view.
    atomicAdd(&accum[base], r_fp);
    atomicAdd(&accum[base + 1u], g_fp);
    atomicAdd(&accum[base + 2u], b_fp);
    atomicAdd(&accum[base + 3u], 1u);
    // Occupancy: every geometry sample marks the voxel as "has geometry".
    atomicAdd(&accum[base + 4u], 1u);
}
