// Temporal denoiser — motion-vector reprojection + neighborhood AABB clamping.
//
// Dispatched with z=3: z=0 GI, z=1 reflections, z=2 shadows.
// For each pixel:
// 1. Convert to UV space, sample motion vector at full-res position
// 2. Reproject to previous frame
// 3. Clamp history to 3x3 neighborhood AABB (anti-ghosting)
// 4. Depth-based disocclusion rejection
// 5. Blend current + clamped history

#import renzora_rt::common::{RtPushConstants, luminance}

@group(0) @binding(1) var depth_texture: texture_depth_2d;
@group(0) @binding(2) var motion_vectors_tex: texture_2d<f32>;
@group(0) @binding(6) var gi_output: texture_storage_2d<rgba16float, read_write>;
@group(0) @binding(7) var gi_history: texture_storage_2d<rgba16float, read_write>;
@group(0) @binding(8) var refl_output: texture_storage_2d<rgba16float, read_write>;
@group(0) @binding(9) var refl_history: texture_storage_2d<rgba16float, read_write>;
@group(0) @binding(10) var shadow_output: texture_storage_2d<r16float, read_write>;
@group(0) @binding(11) var shadow_history: texture_storage_2d<r16float, read_write>;

var<push_constant> pc: RtPushConstants;

@compute @workgroup_size(8, 8, 1)
fn temporal_denoise(@builtin(global_invocation_id) id: vec3<u32>) {
    let signal = id.z; // 0=GI, 1=reflections, 2=shadows
    let coord = vec2<i32>(id.xy);

    // Each signal may have a different texture size (half-res vs full-res)
    var tex_size: vec2<i32>;
    if signal == 0u {
        tex_size = vec2<i32>(textureDimensions(gi_output));
    } else if signal == 1u {
        tex_size = vec2<i32>(textureDimensions(refl_output));
    } else {
        tex_size = vec2<i32>(textureDimensions(shadow_output));
    }

    if coord.x >= tex_size.x || coord.y >= tex_size.y { return; }

    // Convert signal-space coord to normalized UV (0..1)
    let uv = (vec2<f32>(coord) + 0.5) / vec2<f32>(tex_size);

    // Sample motion vector at the corresponding full-res position
    let mv_size = vec2<i32>(textureDimensions(motion_vectors_tex));
    let mv_coord = vec2<i32>(uv * vec2<f32>(mv_size));
    let mv_clamped = clamp(mv_coord, vec2<i32>(0), mv_size - 1);
    let motion = textureLoad(motion_vectors_tex, mv_clamped, 0).rg;

    // Reproject: previous UV = current UV + motion vector
    // (Bevy motion vectors point from current to previous frame)
    let prev_uv = uv + motion;

    // Convert previous UV back to signal-space coord
    let prev_coord = vec2<i32>(prev_uv * vec2<f32>(tex_size));
    let prev_valid = prev_uv.x >= 0.0 && prev_uv.x < 1.0 && prev_uv.y >= 0.0 && prev_uv.y < 1.0;

    // Depth-based disocclusion: compare current depth with depth at reprojected position
    let depth_size = vec2<i32>(textureDimensions(depth_texture));
    let depth_coord = vec2<i32>(uv * vec2<f32>(depth_size));
    let current_depth = textureLoad(depth_texture, clamp(depth_coord, vec2<i32>(0), depth_size - 1), 0);
    let prev_depth_coord = vec2<i32>(prev_uv * vec2<f32>(depth_size));
    let prev_depth = textureLoad(depth_texture, clamp(prev_depth_coord, vec2<i32>(0), depth_size - 1), 0);
    let depth_diff = abs(current_depth - prev_depth);
    let disoccluded = depth_diff > 0.01 || !prev_valid;

    // On reset or disocclusion, fully use current frame
    var alpha = select(0.1, 1.0, pc.reset != 0u || disoccluded);

    if signal == 0u {
        // --- GI ---
        let current = textureLoad(gi_output, coord).rgb;
        let prev_clamped = clamp(prev_coord, vec2<i32>(0), tex_size - 1);
        var history = textureLoad(gi_history, prev_clamped).rgb;

        // 3x3 neighborhood AABB clamp (anti-ghosting)
        var aabb_min = vec3<f32>(1e10);
        var aabb_max = vec3<f32>(-1e10);
        for (var dy = -1i; dy <= 1i; dy++) {
            for (var dx = -1i; dx <= 1i; dx++) {
                let sc = clamp(coord + vec2<i32>(dx, dy), vec2<i32>(0), tex_size - 1);
                let s = textureLoad(gi_output, sc).rgb;
                aabb_min = min(aabb_min, s);
                aabb_max = max(aabb_max, s);
            }
        }
        history = clamp(history, aabb_min, aabb_max);

        let result = mix(history, current, alpha);
        textureStore(gi_output, coord, vec4<f32>(result, 1.0));
        textureStore(gi_history, coord, vec4<f32>(result, 1.0));

    } else if signal == 1u {
        // --- Reflections ---
        let current = textureLoad(refl_output, coord).rgb;
        let prev_clamped = clamp(prev_coord, vec2<i32>(0), tex_size - 1);
        var history = textureLoad(refl_history, prev_clamped).rgb;

        // AABB clamp for reflections too
        var aabb_min = vec3<f32>(1e10);
        var aabb_max = vec3<f32>(-1e10);
        for (var dy = -1i; dy <= 1i; dy++) {
            for (var dx = -1i; dx <= 1i; dx++) {
                let sc = clamp(coord + vec2<i32>(dx, dy), vec2<i32>(0), tex_size - 1);
                let s = textureLoad(refl_output, sc).rgb;
                aabb_min = min(aabb_min, s);
                aabb_max = max(aabb_max, s);
            }
        }
        history = clamp(history, aabb_min, aabb_max);

        let result = mix(history, current, alpha);
        textureStore(refl_output, coord, vec4<f32>(result, 1.0));
        textureStore(refl_history, coord, vec4<f32>(result, 1.0));

    } else {
        // --- Shadows ---
        let current = textureLoad(shadow_output, coord).r;
        let prev_clamped = clamp(prev_coord, vec2<i32>(0), tex_size - 1);
        var history = textureLoad(shadow_history, prev_clamped).r;

        // Clamp shadow history to neighborhood min/max
        var s_min = 1.0;
        var s_max = 0.0;
        for (var dy = -1i; dy <= 1i; dy++) {
            for (var dx = -1i; dx <= 1i; dx++) {
                let sc = clamp(coord + vec2<i32>(dx, dy), vec2<i32>(0), tex_size - 1);
                let sv = textureLoad(shadow_output, sc).r;
                s_min = min(s_min, sv);
                s_max = max(s_max, sv);
            }
        }
        history = clamp(history, s_min, s_max);

        let result = mix(history, current, alpha);
        textureStore(shadow_output, coord, vec4<f32>(result, 0.0, 0.0, 0.0));
        textureStore(shadow_history, coord, vec4<f32>(result, 0.0, 0.0, 0.0));
    }
}
