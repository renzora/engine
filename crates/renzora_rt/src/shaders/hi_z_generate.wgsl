// Hi-Z depth buffer — copy depth to R32Float for ray marching.
//
// Copies the depth buffer into the Hi-Z texture (mip 0).
// Future: full hierarchical mip chain for true Hi-Z traversal
// (requires per-mip bind groups or texture array approach).

#import renzora_rt::common::RtPushConstants

@group(0) @binding(1) var depth_texture: texture_depth_2d;
@group(0) @binding(5) var hi_z_texture: texture_storage_2d<r32float, read_write>;

var<push_constant> pc: RtPushConstants;

@compute @workgroup_size(8, 8, 1)
fn hi_z_generate(@builtin(global_invocation_id) id: vec3<u32>) {
    let tex_size = vec2<u32>(textureDimensions(depth_texture));
    if any(id.xy >= tex_size) {
        return;
    }

    let depth = textureLoad(depth_texture, vec2<i32>(id.xy), 0);
    textureStore(hi_z_texture, vec2<i32>(id.xy), vec4<f32>(depth, 0.0, 0.0, 0.0));
}
