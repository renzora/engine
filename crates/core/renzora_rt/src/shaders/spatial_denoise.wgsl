// A-Trous wavelet spatial denoiser with edge-stopping.
//
// Dispatched N times with step sizes 1, 2, 4, 8, 16 (encoded in push constants).
// 5x5 kernel with B3 spline weights.
// Edge-stopping on depth, normal, and luminance.
// z=0: GI, z=1: reflections, z=2: shadows.

#import bevy_render::view::View
#import renzora_rt::common::{
    RtPushConstants, luminance,
    reconstruct_world_position, estimate_normal_from_depth,
}

@group(0) @binding(1) var depth_texture: texture_depth_2d;
@group(0) @binding(3) var<uniform> view: View;
@group(0) @binding(6) var gi_output: texture_storage_2d<rgba16float, read_write>;
@group(0) @binding(8) var refl_output: texture_storage_2d<rgba16float, read_write>;
@group(0) @binding(10) var shadow_output: texture_storage_2d<r16float, read_write>;

var<push_constant> pc: RtPushConstants;

// B3 spline weights: [1, 2/3, 1/6]
fn kernel_weight(offset: i32) -> f32 {
    let a = abs(offset);
    if a == 0 { return 1.0; }
    if a == 1 { return 0.6667; }
    if a == 2 { return 0.1667; }
    return 0.0;
}

/// Map signal-space coord to depth-space coord (handles half-res signals vs full-res depth).
fn depth_coord_for(c: vec2<i32>, tex_size: vec2<i32>, ds: vec2<i32>) -> vec2<i32> {
    let uv = (vec2<f32>(c) + 0.5) / vec2<f32>(tex_size);
    return clamp(vec2<i32>(uv * vec2<f32>(ds)), vec2<i32>(0), ds - 1);
}

@compute @workgroup_size(8, 8, 1)
fn spatial_denoise(@builtin(global_invocation_id) id: vec3<u32>) {
    let signal = id.z;
    let coord = vec2<i32>(id.xy);

    // Decode step size from frame_index upper 8 bits: step = 1 << iteration
    let iteration = pc.frame_index >> 24u;
    let step = i32(1u << iteration);

    let depth_sigma = 50.0;
    let normal_sigma = 32.0;
    let lum_sigma = 4.0;

    let depth_size = vec2<i32>(textureDimensions(depth_texture));
    let depth_size_f = vec2<f32>(depth_size);

    if signal == 0u {
        // --- Spatial denoise GI ---
        let tex_size = vec2<i32>(textureDimensions(gi_output));
        if coord.x >= tex_size.x || coord.y >= tex_size.y { return; }

        let center = textureLoad(gi_output, coord).rgb;
        let center_dc = depth_coord_for(coord, tex_size, depth_size);
        let center_depth = textureLoad(depth_texture, center_dc, 0);
        let center_normal = estimate_normal_from_depth(depth_texture, center_dc, view.world_from_clip, depth_size_f);
        let center_lum = luminance(center);

        var sum = vec3<f32>(0.0);
        var w_sum = 0.0;

        for (var dy = -2i; dy <= 2i; dy++) {
            for (var dx = -2i; dx <= 2i; dx++) {
                let sc = clamp(coord + vec2<i32>(dx, dy) * step, vec2<i32>(0), tex_size - 1);
                let dc = depth_coord_for(sc, tex_size, depth_size);

                let s = textureLoad(gi_output, sc).rgb;
                let sd = textureLoad(depth_texture, dc, 0);
                let sn = estimate_normal_from_depth(depth_texture, dc, view.world_from_clip, depth_size_f);

                let dw = exp(-abs(f32(center_depth) - f32(sd)) * depth_sigma);
                let nw = pow(max(dot(center_normal, sn), 0.0), normal_sigma);
                let lw = exp(-abs(center_lum - luminance(s)) * lum_sigma);
                let kw = kernel_weight(dx) * kernel_weight(dy);
                let w = kw * dw * nw * lw;

                sum += s * w;
                w_sum += w;
            }
        }

        textureStore(gi_output, coord, vec4<f32>(sum / max(w_sum, 0.0001), 1.0));

    } else if signal == 1u {
        // --- Spatial denoise reflections ---
        let tex_size = vec2<i32>(textureDimensions(refl_output));
        if coord.x >= tex_size.x || coord.y >= tex_size.y { return; }

        let center = textureLoad(refl_output, coord).rgb;
        let center_dc = depth_coord_for(coord, tex_size, depth_size);
        let center_depth = textureLoad(depth_texture, center_dc, 0);
        let center_normal = estimate_normal_from_depth(depth_texture, center_dc, view.world_from_clip, depth_size_f);

        var sum = vec3<f32>(0.0);
        var w_sum = 0.0;

        for (var dy = -2i; dy <= 2i; dy++) {
            for (var dx = -2i; dx <= 2i; dx++) {
                let sc = clamp(coord + vec2<i32>(dx, dy) * step, vec2<i32>(0), tex_size - 1);
                let dc = depth_coord_for(sc, tex_size, depth_size);

                let s = textureLoad(refl_output, sc).rgb;
                let sd = textureLoad(depth_texture, dc, 0);
                let sn = estimate_normal_from_depth(depth_texture, dc, view.world_from_clip, depth_size_f);

                let dw = exp(-abs(f32(center_depth) - f32(sd)) * depth_sigma);
                let nw = pow(max(dot(center_normal, sn), 0.0), normal_sigma);
                let kw = kernel_weight(dx) * kernel_weight(dy);
                let w = kw * dw * nw;

                sum += s * w;
                w_sum += w;
            }
        }

        textureStore(refl_output, coord, vec4<f32>(sum / max(w_sum, 0.0001), 1.0));

    } else {
        // --- Spatial denoise shadows ---
        let tex_size = vec2<i32>(textureDimensions(shadow_output));
        if coord.x >= tex_size.x || coord.y >= tex_size.y { return; }

        let center = textureLoad(shadow_output, coord).r;
        let center_dc = depth_coord_for(coord, tex_size, depth_size);
        let center_depth = textureLoad(depth_texture, center_dc, 0);
        let center_normal = estimate_normal_from_depth(depth_texture, center_dc, view.world_from_clip, depth_size_f);

        var sum = 0.0;
        var w_sum = 0.0;

        for (var dy = -2i; dy <= 2i; dy++) {
            for (var dx = -2i; dx <= 2i; dx++) {
                let sc = clamp(coord + vec2<i32>(dx, dy) * step, vec2<i32>(0), tex_size - 1);
                let dc = depth_coord_for(sc, tex_size, depth_size);

                let sv = textureLoad(shadow_output, sc).r;
                let sd = textureLoad(depth_texture, dc, 0);
                let sn = estimate_normal_from_depth(depth_texture, dc, view.world_from_clip, depth_size_f);

                let dw = exp(-abs(f32(center_depth) - f32(sd)) * depth_sigma);
                let nw = pow(max(dot(center_normal, sn), 0.0), normal_sigma);
                let kw = kernel_weight(dx) * kernel_weight(dy);
                let w = kw * dw * nw;

                sum += sv * w;
                w_sum += w;
            }
        }

        textureStore(shadow_output, coord, vec4<f32>(sum / max(w_sum, 0.0001), 0.0, 0.0, 0.0));
    }
}
