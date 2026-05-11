// Voxel cache debug — ray-march through the clipmap volume and show
// the first occupied voxel hit. At each marching step we pick the
// **tightest-fit cascade** that contains the world position; if that
// cascade's voxel is occupied we return its color. Otherwise we step
// forward by the current cascade's voxel size and try again.

#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput
#import bevy_render::view::View

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

@group(0) @binding(0) var voxels: texture_3d<f32>;
@group(0) @binding(1) var voxel_sampler: sampler;
@group(0) @binding(2) var depth_tex: texture_depth_2d;
@group(0) @binding(3) var scene_tex: texture_2d<f32>;
@group(0) @binding(4) var<uniform> view: View;
@group(0) @binding(5) var<uniform> grid: VoxelGrid;

const HIT_ALPHA: f32 = 0.5;

fn uv_to_ndc(uv: vec2<f32>) -> vec2<f32> {
    return uv * vec2<f32>(2.0, -2.0) + vec2<f32>(-1.0, 1.0);
}

fn reconstruct_world_ray(uv: vec2<f32>) -> vec3<f32> {
    let ndc_xy = uv_to_ndc(uv);
    let near_clip = vec4<f32>(ndc_xy, 1.0, 1.0);
    let near_h = view.world_from_clip * near_clip;
    let near_pos = near_h.xyz / near_h.w;
    return normalize(near_pos - view.world_position);
}

fn surface_distance(pixel: vec2<i32>, uv: vec2<f32>) -> f32 {
    let depth = textureLoad(depth_tex, pixel, 0);
    if (depth <= 0.0) { return 1.0e6; }
    let ndc = vec3<f32>(uv_to_ndc(uv), depth);
    let world_h = view.world_from_clip * vec4<f32>(ndc, 1.0);
    let world_pos = world_h.xyz / world_h.w;
    return length(world_pos - view.world_position);
}

// Combined AABB of all cascades — used to bound the initial march.
fn outermost_aabb() -> mat2x4<f32> {
    let outer = grid.cascades[grid.cascade_count - 1u];
    let extent = f32(grid.resolution) * outer.voxel_size;
    return mat2x4<f32>(
        vec4<f32>(outer.origin, 0.0),
        vec4<f32>(outer.origin + vec3<f32>(extent, extent, extent), 0.0),
    );
}

struct RayAabbHit {
    enter: f32,
    exit: f32,
    valid: bool,
};

fn ray_aabb(origin: vec3<f32>, dir: vec3<f32>, box_min: vec3<f32>, box_max: vec3<f32>) -> RayAabbHit {
    let inv = 1.0 / dir;
    let t1 = (box_min - origin) * inv;
    let t2 = (box_max - origin) * inv;
    let tmin = min(t1, t2);
    let tmax = max(t1, t2);
    let enter = max(max(tmin.x, tmin.y), tmin.z);
    let exit = min(min(tmax.x, tmax.y), tmax.z);
    var hit: RayAabbHit;
    hit.enter = enter;
    hit.exit = exit;
    hit.valid = exit >= enter && exit >= 0.0;
    return hit;
}

// Returns the tightest cascade (smallest voxel size) whose volume
// contains `p`, or -1 if `p` is outside the entire clipmap.
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
    // Voxels in cascade C live at Z = [C * RES, (C+1) * RES).
    let idx = vec3<i32>(idx_local.x, idx_local.y, idx_local.z + i32(cascade * grid.resolution));
    return textureLoad(voxels, idx, 0);
}

@fragment
fn fragment(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let scene = textureSample(scene_tex, voxel_sampler, in.uv);
    let pixel = vec2<i32>(in.position.xy);

    let ray_dir = reconstruct_world_ray(in.uv);
    let camera = view.world_position;

    let outer = outermost_aabb();
    let hit = ray_aabb(camera, ray_dir, outer[0].xyz, outer[1].xyz);
    if (!hit.valid) { return scene; }

    let surface_t = surface_distance(pixel, in.uv);
    var t = max(hit.enter, 0.0);
    let max_t = min(hit.exit, surface_t);

    for (var i: u32 = 0u; i < 256u; i = i + 1u) {
        if (t > max_t) { break; }
        let p = camera + ray_dir * t;
        let cascade = select_cascade(p);
        if (cascade < 0) {
            // Outside the clipmap (shouldn't happen between hit.enter
            // and hit.exit, but safe fallback).
            t = t + grid.cascades[grid.cascade_count - 1u].voxel_size;
            continue;
        }
        let voxel = cascade_voxel_load(p, u32(cascade));
        if (voxel.a > HIT_ALPHA) {
            return vec4<f32>(voxel.rgb, 1.0);
        }
        // Step by the current cascade's voxel size so we don't skip
        // small voxels near the camera nor waste samples in coarse
        // cascades far away.
        t = t + grid.cascades[cascade].voxel_size;
    }

    return scene;
}
