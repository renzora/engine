// Composite — blend denoised GI, reflections, and contact shadows into HDR color.
//
// For half-res GI/reflections, uses bilinear interpolation from the trace-res textures.
// When deferred GBuffer is available, uses real roughness/metallic for reflection weighting.
// Otherwise falls back to Schlick Fresnel approximation (F0=0.04).

#import bevy_render::view::View
#import renzora_rt::common::{
    RtPushConstants, pc_gi_enabled, pc_reflections_enabled, pc_shadows_enabled,
    pc_is_half_res, pc_gi_intensity, pc_refl_intensity, luminance,
    reconstruct_world_position, estimate_normal_from_depth,
}

@group(0) @binding(0) var hdr_color: texture_storage_2d<rgba16float, read_write>;
@group(0) @binding(1) var depth_texture: texture_depth_2d;
@group(0) @binding(3) var<uniform> view: View;
@group(0) @binding(6) var gi_output: texture_storage_2d<rgba16float, read_write>;
@group(0) @binding(8) var refl_output: texture_storage_2d<rgba16float, read_write>;
@group(0) @binding(10) var shadow_output: texture_storage_2d<r16float, read_write>;
@group(0) @binding(19) var deferred_gbuffer: texture_2d<u32>;

var<push_constant> pc: RtPushConstants;

/// Bilinear sample from gi_output at fractional coordinates.
fn bilinear_sample_gi(fcoord: vec2<f32>, tex_size: vec2<i32>) -> vec3<f32> {
    let base = vec2<i32>(floor(fcoord - 0.5));
    let frac = fcoord - 0.5 - vec2<f32>(base);

    let c00 = textureLoad(gi_output, clamp(base, vec2<i32>(0), tex_size - 1)).rgb;
    let c10 = textureLoad(gi_output, clamp(base + vec2<i32>(1, 0), vec2<i32>(0), tex_size - 1)).rgb;
    let c01 = textureLoad(gi_output, clamp(base + vec2<i32>(0, 1), vec2<i32>(0), tex_size - 1)).rgb;
    let c11 = textureLoad(gi_output, clamp(base + vec2<i32>(1, 1), vec2<i32>(0), tex_size - 1)).rgb;

    let top = mix(c00, c10, frac.x);
    let bot = mix(c01, c11, frac.x);
    return mix(top, bot, frac.y);
}

/// Bilinear sample from refl_output at fractional coordinates.
fn bilinear_sample_refl(fcoord: vec2<f32>, tex_size: vec2<i32>) -> vec3<f32> {
    let base = vec2<i32>(floor(fcoord - 0.5));
    let frac = fcoord - 0.5 - vec2<f32>(base);

    let c00 = textureLoad(refl_output, clamp(base, vec2<i32>(0), tex_size - 1)).rgb;
    let c10 = textureLoad(refl_output, clamp(base + vec2<i32>(1, 0), vec2<i32>(0), tex_size - 1)).rgb;
    let c01 = textureLoad(refl_output, clamp(base + vec2<i32>(0, 1), vec2<i32>(0), tex_size - 1)).rgb;
    let c11 = textureLoad(refl_output, clamp(base + vec2<i32>(1, 1), vec2<i32>(0), tex_size - 1)).rgb;

    let top = mix(c00, c10, frac.x);
    let bot = mix(c01, c11, frac.x);
    return mix(top, bot, frac.y);
}

/// Schlick Fresnel with configurable F0.
fn fresnel_schlick_f0(cos_theta: f32, f0: f32) -> f32 {
    return f0 + (1.0 - f0) * pow(1.0 - cos_theta, 5.0);
}

/// Unpack roughness and metallic from Bevy's deferred GBuffer.
/// gbuffer.x: pack4x8unorm(base_color.rgb, perceptual_roughness)
/// gbuffer.z: pack4x8unorm(reflectance, metallic, diffuse_occlusion, spare)
/// Returns vec3(perceptual_roughness, metallic, reflectance). Returns (-1, 0, 0) if no GBuffer data.
fn unpack_material(coord: vec2<i32>) -> vec3<f32> {
    let gbuffer = textureLoad(deferred_gbuffer, coord, 0);

    // If gbuffer is all zero, no deferred data for this pixel
    if gbuffer.x == 0u && gbuffer.z == 0u {
        return vec3<f32>(-1.0, 0.0, 0.0);
    }

    // Roughness is in the top 8 bits of gbuffer.x
    let roughness = f32((gbuffer.x >> 24u) & 0xFFu) / 255.0;

    // Metallic is in bits 8-15 of gbuffer.z, reflectance in bits 0-7
    let reflectance = f32(gbuffer.z & 0xFFu) / 255.0;
    let metallic = f32((gbuffer.z >> 8u) & 0xFFu) / 255.0;

    return vec3<f32>(roughness, metallic, reflectance);
}

@compute @workgroup_size(8, 8, 1)
fn composite(@builtin(global_invocation_id) id: vec3<u32>) {
    let tex_size = vec2<i32>(textureDimensions(hdr_color));
    let coord = vec2<i32>(id.xy);

    if coord.x >= tex_size.x || coord.y >= tex_size.y {
        return;
    }

    let depth = textureLoad(depth_texture, coord, 0);
    if depth >= 1.0 || depth <= 0.0 {
        return; // sky
    }

    let direct = textureLoad(hdr_color, coord);

    // Approximate albedo from lit color (clamp to physical range)
    let lum = luminance(direct.rgb);
    let albedo = clamp(direct.rgb / max(lum, 0.01), vec3<f32>(0.0), vec3<f32>(1.0));

    // Compute view-dependent Fresnel
    let depth_size = vec2<f32>(textureDimensions(depth_texture));
    let uv = (vec2<f32>(coord) + 0.5) / vec2<f32>(tex_size);
    let world_pos = reconstruct_world_position(depth, uv, view.world_from_clip);
    let normal = estimate_normal_from_depth(depth_texture, coord, view.world_from_clip, depth_size);
    let view_dir = normalize(view.world_position.xyz - world_pos);
    let n_dot_v = max(dot(normal, view_dir), 0.0);

    // Try to read material properties from the deferred GBuffer
    let material = unpack_material(coord);
    let has_gbuffer = material.x >= 0.0;

    // Compute reflection weight based on material properties
    var refl_weight: f32;
    if has_gbuffer {
        let roughness = material.x;
        let metallic = material.y;
        let reflectance = material.z;

        // Smoothness-based reflection strength
        let smoothness = 1.0 - roughness;

        // F0 for dielectrics comes from reflectance, metals use albedo luminance
        // Reflectance of 0.5 = F0 of 0.04 (typical dielectric)
        let f0_dielectric = 0.16 * reflectance * reflectance;
        let f0 = mix(f0_dielectric, 0.7, metallic);

        let fresnel = fresnel_schlick_f0(n_dot_v, f0);

        // Windows/puddles: high smoothness, low-medium metallic → strong reflections
        // Rough walls: low smoothness → minimal reflections
        // Smooth metals: high smoothness, high metallic → very strong reflections
        refl_weight = fresnel * smoothness * smoothness;
    } else {
        // No GBuffer data — conservative Fresnel-only (F0=0.04)
        refl_weight = fresnel_schlick_f0(n_dot_v, 0.04);
    }

    // Contact shadows darken only direct light
    var shadow_factor = 1.0;
    if pc_shadows_enabled(pc) {
        shadow_factor = textureLoad(shadow_output, coord).r;
    }

    var result = direct.rgb * shadow_factor;

    // Add indirect light (not affected by contact shadows)
    // Use ratio-based bilinear sampling — works for any resolution (probes, half-res, full-res)
    let full_size_f = vec2<f32>(tex_size);
    var gi_indirect = vec3<f32>(0.0);
    var refl_indirect = vec3<f32>(0.0);

    if pc_gi_enabled(pc) {
        let gi_size = vec2<i32>(textureDimensions(gi_output));
        let gi_fcoord = vec2<f32>(coord) * vec2<f32>(gi_size) / full_size_f;
        let gi_raw = bilinear_sample_gi(gi_fcoord, gi_size);
        gi_indirect = gi_raw / (1.0 + luminance(gi_raw));
    }

    if pc_reflections_enabled(pc) {
        let refl_size = vec2<i32>(textureDimensions(refl_output));
        let refl_fcoord = vec2<f32>(coord) * vec2<f32>(refl_size) / full_size_f;
        let refl_raw = bilinear_sample_refl(refl_fcoord, refl_size);
        refl_indirect = refl_raw / (1.0 + luminance(refl_raw));
    }

    // GI diffuse: reduce contribution for metallic surfaces (metals don't scatter light diffusely)
    var gi_diffuse_weight = 1.0;
    if has_gbuffer {
        gi_diffuse_weight = 1.0 - material.y; // (1 - metallic)
    }

    result += albedo * gi_indirect * pc_gi_intensity(pc) * gi_diffuse_weight;
    result += refl_indirect * refl_weight * pc_refl_intensity(pc);

    textureStore(hdr_color, coord, vec4<f32>(result, direct.a));
}
