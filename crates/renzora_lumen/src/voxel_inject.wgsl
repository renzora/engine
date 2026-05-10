// Inject visible-surface lighting into the per-frame voxel
// accumulation buffer using atomicAdd, so multi-pixel-per-voxel
// contention produces an exact average instead of last-write-wins
// flicker.

#import bevy_render::view::View

struct VoxelGrid {
    origin: vec3<f32>,
    voxel_size: f32,
    resolution: u32,
    _pad0: u32,
    _pad1: u32,
    _pad2: u32,
};

@group(0) @binding(0) var scene_tex: texture_2d<f32>;
@group(0) @binding(1) var depth_tex: texture_depth_2d;
@group(0) @binding(2) var<storage, read_write> accum: array<atomic<u32>>;
@group(0) @binding(3) var<uniform> view: View;
@group(0) @binding(4) var<uniform> grid: VoxelGrid;

const FIXED_POINT_SCALE: f32 = 256.0;
// Cap per-pixel contribution before fixed-point encoding so a single
// bright pixel can't blow out the u32 sum (the resolve pass divides by
// count anyway, so a clamped color still averages correctly with
// neighbors).
const MAX_CHANNEL: f32 = 8.0;

fn uv_to_ndc(uv: vec2<f32>) -> vec2<f32> {
    return uv * vec2<f32>(2.0, -2.0) + vec2<f32>(-1.0, 1.0);
}

fn world_pos_from_depth(uv: vec2<f32>, depth: f32) -> vec3<f32> {
    let ndc = vec3<f32>(uv_to_ndc(uv), depth);
    let world_h = view.world_from_clip * vec4<f32>(ndc, 1.0);
    return world_h.xyz / world_h.w;
}

@compute @workgroup_size(8, 8, 1)
fn inject(@builtin(global_invocation_id) gid: vec3<u32>) {
    let scene_dims = textureDimensions(scene_tex);
    if (gid.x >= scene_dims.x || gid.y >= scene_dims.y) { return; }

    let pixel = vec2<i32>(gid.xy);
    let depth = textureLoad(depth_tex, pixel, 0);
    if (depth <= 0.0) { return; }

    let uv = (vec2<f32>(pixel) + 0.5) / vec2<f32>(scene_dims);
    let world_pos = world_pos_from_depth(uv, depth);

    let local = (world_pos - grid.origin) / grid.voxel_size;
    let idx = vec3<i32>(local);
    if (any(idx < vec3<i32>(0)) || any(idx >= vec3<i32>(i32(grid.resolution)))) {
        return;
    }

    let color = textureLoad(scene_tex, pixel, 0).rgb;
    let clamped = clamp(color, vec3<f32>(0.0), vec3<f32>(MAX_CHANNEL));
    let r_fp = u32(clamped.r * FIXED_POINT_SCALE);
    let g_fp = u32(clamped.g * FIXED_POINT_SCALE);
    let b_fp = u32(clamped.b * FIXED_POINT_SCALE);

    let res = grid.resolution;
    let voxel_idx = u32(idx.x) + u32(idx.y) * res + u32(idx.z) * res * res;
    let base = voxel_idx * 4u;

    atomicAdd(&accum[base], r_fp);
    atomicAdd(&accum[base + 1u], g_fp);
    atomicAdd(&accum[base + 2u], b_fp);
    atomicAdd(&accum[base + 3u], 1u);
}
