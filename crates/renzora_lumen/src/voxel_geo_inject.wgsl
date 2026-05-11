// Inject baked geometry samples into every cascade whose AABB
// contains the sample world position. Each sample contributes both
// radiance (clamped base color, fixed-point sum) and an occupancy
// flag (geom_count) so the resolve pass can produce a clean
// 'solid vs. empty' signal.

struct CascadeData {
    origin: vec3<f32>,
    voxel_size: f32,
};

struct VoxelGrid {
    cascades: array<CascadeData, 4>,
    resolution: u32,
    cascade_count: u32,
    _pad0: u32,
    _pad1: u32,
};

struct Sample {
    world_pos: vec4<f32>, // xyz + pad
    albedo: vec4<f32>,    // rgb + pad
};

@group(0) @binding(0) var<storage, read> samples: array<Sample>;
@group(0) @binding(1) var<storage, read_write> accum: array<atomic<u32>>;
@group(0) @binding(2) var<uniform> grid: VoxelGrid;

const FIXED_POINT_SCALE: f32 = 256.0;
const MAX_CHANNEL: f32 = 4.0;

@compute @workgroup_size(64, 1, 1)
fn inject(@builtin(global_invocation_id) gid: vec3<u32>) {
    let n = arrayLength(&samples);
    if (gid.x >= n) { return; }
    let s = samples[gid.x];
    let world_pos = s.world_pos.xyz;
    let color = clamp(s.albedo.rgb, vec3<f32>(0.0), vec3<f32>(MAX_CHANNEL));
    let r_fp = u32(color.r * FIXED_POINT_SCALE);
    let g_fp = u32(color.g * FIXED_POINT_SCALE);
    let b_fp = u32(color.b * FIXED_POINT_SCALE);

    let res = grid.resolution;
    let voxels_per_cascade = res * res * res;
    for (var c: u32 = 0u; c < grid.cascade_count; c = c + 1u) {
        let cascade = grid.cascades[c];
        let local = (world_pos - cascade.origin) / cascade.voxel_size;
        let idx = vec3<i32>(local);
        if (any(idx < vec3<i32>(0)) || any(idx >= vec3<i32>(i32(res)))) { continue; }

        let voxel_idx = u32(idx.x) + u32(idx.y) * res + u32(idx.z) * res * res;
        let base = (c * voxels_per_cascade + voxel_idx) * 5u;
        atomicAdd(&accum[base], r_fp);
        atomicAdd(&accum[base + 1u], g_fp);
        atomicAdd(&accum[base + 2u], b_fp);
        atomicAdd(&accum[base + 3u], 1u);
        atomicAdd(&accum[base + 4u], 1u);
    }
}
