// Half-resolution dedicated screen-space reflection trace.
//
// One thread per half-res output pixel. For each, reconstruct the
// world surface at the representative full-res pixel (top-left of
// the 2×2 block), march the reflection ray in world space, project
// per step to UV, depth-test against the full-res depth buffer.
// On hit: sample full-res scene color with linear filtering, write
// to half-res output with alpha = validity.
//
// Stage 1 design notes:
//   * Sky pixels (depth == 0) skip the trace entirely — there's
//     nothing to reflect for them, and the lumen_trace composite
//     pass falls back to the IBL sky cubemap.
//   * Validity is encoded in alpha so stage 2's blur (and stage 4's
//     bilateral resolve) can weight neighbours by hit confidence
//     without losing mip-level continuity at the valid/invalid
//     boundary.
//   * Screen-edge fade is baked into validity (smoothstep over the
//     10% inset band) so a high-roughness pixel near the screen
//     edge fades into the voxel-cone fallback gradually rather
//     than cutting off hard.
//
// Stages 2-4 will read this buffer. Stage 1's job is just to put a
// useful, correctly-aligned, half-res reflection signal into the
// output texture.

#import bevy_render::view::View

@group(0) @binding(0) var scene_tex: texture_2d<f32>;
@group(0) @binding(1) var scene_sampler: sampler;
@group(0) @binding(2) var depth_tex: texture_depth_2d;
@group(0) @binding(3) var normal_tex: texture_2d<f32>;
@group(0) @binding(4) var<uniform> view: View;
@group(0) @binding(5) var gbuffer: texture_2d<u32>;
@group(0) @binding(6) var output_color: texture_storage_2d<rgba16float, write>;
@group(0) @binding(7) var output_mip_level: texture_storage_2d<r32float, write>;

// Max mip level we can write — matches `REFLECTION_MIP_COUNT - 1` on
// the host. Used to clamp the computed Godot-style mip level so the
// resolve pass never tries to sample beyond the pyramid.
const MAX_MIP_LEVEL: f32 = 4.0;
const PI: f32 = 3.14159265359;

// 32 steps × 0.5m = 16m of reach. Matches the inline trace we're
// replacing — keep visual parity, then we'll tune in later stages.
const MAX_STEPS: u32 = 32u;
const STEP_DIST: f32 = 0.5;
// View-space depth tolerance for the "did we hit this surface" test.
const HIT_THRESHOLD: f32 = 0.5;
// 10% inset edge fade so the boundary with the voxel fallback is
// smooth, not a hard line.
const EDGE_FADE: f32 = 0.1;

fn uv_to_ndc(uv: vec2<f32>) -> vec2<f32> {
    return uv * vec2<f32>(2.0, -2.0) + vec2<f32>(-1.0, 1.0);
}

fn world_pos_from_depth(uv: vec2<f32>, depth: f32) -> vec3<f32> {
    let ndc = vec3<f32>(uv_to_ndc(uv), depth);
    let world_h = view.world_from_clip * vec4<f32>(ndc, 1.0);
    return world_h.xyz / world_h.w;
}

@compute @workgroup_size(8, 8, 1)
fn trace(@builtin(global_invocation_id) gid: vec3<u32>) {
    let half_size = textureDimensions(output_color);
    if (any(gid.xy >= half_size)) {
        return;
    }

    // Representative full-res pixel for this half-res output. Top-
    // left of the 2×2 block — bilateral resolve in stage 4 will
    // recover the offset properly when upsampling.
    let full_pixel = vec2<i32>(gid.xy * 2u);
    let full_size = vec2<f32>(textureDimensions(depth_tex));
    // UV of that full-res pixel center (+0.5 for pixel center).
    let uv = (vec2<f32>(full_pixel) + vec2<f32>(0.5)) / full_size;

    // Sky → no reflection, write zero validity. Stage 4 will fall
    // through to voxel-cone (or sky cubemap) for these pixels.
    let depth = textureLoad(depth_tex, full_pixel, 0);
    if (depth <= 0.0) {
        textureStore(output_color, vec2<i32>(gid.xy), vec4<f32>(0.0));
        textureStore(output_mip_level, vec2<i32>(gid.xy), vec4<f32>(0.0));
        return;
    }

    let world_pos = world_pos_from_depth(uv, depth);
    let normal_world = normalize(textureLoad(normal_tex, full_pixel, 0).xyz * 2.0 - 1.0);
    let view_dir = normalize(view.world_position.xyz - world_pos);
    let reflect_dir = reflect(-view_dir, normal_world);

    // Unpack perceptual roughness from the G-buffer R channel
    // (`pack_unorm4x8_(vec4(base_color_srgb, perceptual_roughness))`).
    // Drives the mip_level computation below — rougher surfaces or
    // longer rays produce wider cones, which we encode as higher mip.
    let gb = textureLoad(gbuffer, full_pixel, 0).r;
    let base_rough = unpack4x8unorm(gb);
    let perceptual_roughness = base_rough.a;

    // Tiny bias along the normal so the first step doesn't immediately
    // self-hit the surface voxel we're shading.
    let origin = world_pos + normal_world * 0.05;

    // ── Screen-space ray march ──────────────────────────────────
    var p = origin;
    var hit_color = vec3<f32>(0.0);
    var validity = 0.0;
    // World-space distance the ray traveled before hitting (used in
    // the mip_level cone-of-confusion math below).
    var ray_length = 0.0;

    for (var i: u32 = 0u; i < MAX_STEPS; i = i + 1u) {
        p = p + reflect_dir * STEP_DIST;

        let clip = view.clip_from_world * vec4<f32>(p, 1.0);
        if (clip.w <= 0.0) { break; }
        let ndc = clip.xyz / clip.w;

        if (any(abs(ndc.xy) > vec2<f32>(1.0))) { break; }
        if (ndc.z < 0.0 || ndc.z > 1.0) { break; }

        let sample_uv = ndc.xy * vec2<f32>(0.5, -0.5) + vec2<f32>(0.5);
        let sample_pixel = vec2<i32>(sample_uv * full_size);
        let scene_depth = textureLoad(depth_tex, sample_pixel, 0);
        if (scene_depth <= 0.0) { continue; }

        let surface_world = world_pos_from_depth(sample_uv, scene_depth);
        let marched_view_z = (view.view_from_world * vec4<f32>(p, 1.0)).z;
        let surface_view_z = (view.view_from_world * vec4<f32>(surface_world, 1.0)).z;
        let depth_diff = surface_view_z - marched_view_z;

        if (depth_diff > 0.0 && depth_diff < HIT_THRESHOLD) {
            hit_color = textureSampleLevel(scene_tex, scene_sampler, sample_uv, 0.0).rgb;
            let edge_dist = min(min(sample_uv.x, 1.0 - sample_uv.x), min(sample_uv.y, 1.0 - sample_uv.y));
            validity = smoothstep(0.0, EDGE_FADE, edge_dist);
            ray_length = length(p - origin);
            break;
        }
    }

    // ── mip_level computation (roughness × distance) ────────────
    // Pick a pyramid level whose effective blur radius roughly
    // matches the GGX lobe footprint at the hit point. Godot's
    // cone-of-confusion math is more rigorous but expects ray_length
    // in screen-UV (0-1) space; we trace in world-space meters here,
    // so the literal port over-blurs.
    //
    // The mapping that actually looks right in practice:
    //   * squared roughness — matches how artists perceive "rough" vs
    //     "smooth" (linear roughness feels weighted toward the rough
    //     end of the slider). Glass at perceptual 0.05 stays sharp;
    //     concrete at 0.6 lands around mip 1.4.
    //   * mild distance modulation — far reflections are already
    //     covered by fewer pixels of the half-res buffer so they need
    //     less explicit blur than the formula suggests. 20m ramp.
    //
    // No-hit (ray_length == 0) writes mip 0, which the resolve stage
    // ignores anyway because validity = 0 there.
    var mip_level = 0.0;
    if (perceptual_roughness > 0.001 && ray_length > 0.001) {
        let roughness_factor = perceptual_roughness * perceptual_roughness * MAX_MIP_LEVEL;
        let distance_factor = clamp(ray_length / 20.0, 0.0, 1.0) * 0.5;
        mip_level = clamp(
            roughness_factor + roughness_factor * distance_factor,
            0.0,
            MAX_MIP_LEVEL,
        );
    }

    textureStore(output_color, vec2<i32>(gid.xy), vec4<f32>(hit_color, validity));
    textureStore(output_mip_level, vec2<i32>(gid.xy), vec4<f32>(mip_level, 0.0, 0.0, 0.0));
}
