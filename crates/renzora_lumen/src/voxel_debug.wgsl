// Voxel cache debug visualization.
//
// For each on-screen pixel, reconstruct its world-space surface
// position from the depth prepass and read the voxel at that position.
// The result: a chunky low-res "voxelized" version of the scene that
// shows what the cache currently holds. Voxels outside the cache
// volume or below the alpha threshold pass through to the scene
// texture so you can still see context (sky, foreground objects).

#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput
#import bevy_render::view::View

struct VoxelGrid {
    origin: vec3<f32>,
    voxel_size: f32,
    resolution: u32,
    _pad0: u32,
    _pad1: u32,
    _pad2: u32,
};

@group(0) @binding(0) var voxels: texture_3d<f32>;
@group(0) @binding(1) var voxel_sampler: sampler;
@group(0) @binding(2) var depth_tex: texture_depth_2d;
@group(0) @binding(3) var scene_tex: texture_2d<f32>;
@group(0) @binding(4) var<uniform> view: View;
@group(0) @binding(5) var<uniform> grid: VoxelGrid;

fn uv_to_ndc(uv: vec2<f32>) -> vec2<f32> {
    return uv * vec2<f32>(2.0, -2.0) + vec2<f32>(-1.0, 1.0);
}

fn world_pos_from_depth(uv: vec2<f32>, depth: f32) -> vec3<f32> {
    let ndc = vec3<f32>(uv_to_ndc(uv), depth);
    let world_h = view.world_from_clip * vec4<f32>(ndc, 1.0);
    return world_h.xyz / world_h.w;
}

@fragment
fn fragment(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let pixel = vec2<i32>(in.position.xy);
    let depth = textureLoad(depth_tex, pixel, 0);
    if (depth <= 0.0) {
        // Sky — keep scene.
        return textureSample(scene_tex, voxel_sampler, in.uv);
    }

    let world_pos = world_pos_from_depth(in.uv, depth);
    let local = (world_pos - grid.origin) / grid.voxel_size;
    let extent = f32(grid.resolution);
    if (any(local < vec3<f32>(0.0)) || any(local >= vec3<f32>(extent))) {
        // Outside cache volume — fall back to scene so you can see how
        // far the cache reaches.
        return textureSample(scene_tex, voxel_sampler, in.uv);
    }

    // Sample with linear filter for a slightly smoother chunky look.
    // The +0.5 keeps texel centers aligned with voxel centers.
    let uvw = (local + 0.5) / extent;
    let voxel = textureSampleLevel(voxels, voxel_sampler, uvw, 0.0);
    if (voxel.a < 0.01) {
        // No data injected here — scene fallback (probably a
        // disocclusion the inject pass skipped because of sky).
        return textureSample(scene_tex, voxel_sampler, in.uv);
    }

    return vec4<f32>(voxel.rgb, 1.0);
}
