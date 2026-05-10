// Screen-space GI with temporal accumulation + motion-vector reprojection.
//
// Pipeline per pixel:
//   1. Trace N hemisphere rays, march in view space, sample scene at hit.
//   2. Sample motion vectors → reproject UV to where this surface was last
//      frame; sample history there. Reject if reprojected UV is off-screen
//      or stored linear depth disagrees with the current pixel's depth.
//   3. Blend current trace with valid history.
//   4. Output 0: scene + blended indirect (composited into HDR).
//      Output 1: blended indirect + current linear depth → next-frame history.
//
// Bevy's motion-vector convention (from `bevy_pbr/.../prepass.wgsl`):
//   `motion_vector = (clip - prev_clip) * vec2(0.5, -0.5)`, i.e. UV-space
//   delta from previous to current. So `history_uv = uv - motion_vector`.

#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput
#import bevy_render::view::View

struct RtConfig {
    intensity: f32,
    frame_count: u32,
    // 0 = scene + indirect (composite). 1 = indirect only (debug).
    debug_mode: u32,
    _pad0: f32,
};

@group(0) @binding(0) var scene_tex: texture_2d<f32>;
@group(0) @binding(1) var scene_sampler: sampler;
@group(0) @binding(2) var depth_tex: texture_depth_2d;
@group(0) @binding(3) var normal_tex: texture_2d<f32>;
@group(0) @binding(4) var history_tex: texture_2d<f32>;
@group(0) @binding(5) var motion_tex: texture_2d<f32>;
@group(0) @binding(6) var<uniform> view: View;
@group(0) @binding(7) var<uniform> config: RtConfig;

const SAMPLES: u32 = 4u;
const MARCH_STEPS: u32 = 12u;
const STEP_SIZE: f32 = 0.35;
const NORMAL_BIAS: f32 = 0.05;
const HIT_TOLERANCE: f32 = 0.6;
const PI: f32 = 3.14159265359;

// Maximum per-sample luminance contribution (in HDR units). Bright
// surfaces — sunlit billboards, specular highlights, the sky — produce
// pixels with luminance in the dozens, and a single ray hitting one
// dominates a 4-sample average → bright "fireflies" that temporal then
// propagates. Clamping per-sample radiance to a sane ceiling kills the
// fireflies at the cost of slightly underestimating bounce from very
// bright sources, which is a much better trade than visible sparkle.
const MAX_SAMPLE_LUMINANCE: f32 = 4.0;

// How fast we accept new samples vs trust history.
// 0.05 = 95% history / 5% current → very stable but slow to react.
// 0.20 = 80% history / 20% current → faster reaction, more residual noise.
const TEMPORAL_ALPHA: f32 = 0.08;
// Depth disocclusion threshold (in view-space linear units). Anything
// further than this between the history's stored depth and current depth
// is treated as a different surface and history is rejected.
const DEPTH_DISOCCLUSION: f32 = 0.5;

fn uv_to_ndc(uv: vec2<f32>) -> vec2<f32> {
    return uv * vec2<f32>(2.0, -2.0) + vec2<f32>(-1.0, 1.0);
}

fn ndc_to_uv(ndc: vec2<f32>) -> vec2<f32> {
    return ndc * vec2<f32>(0.5, -0.5) + vec2<f32>(0.5, 0.5);
}

fn view_pos_from_depth(uv: vec2<f32>, depth: f32) -> vec3<f32> {
    let ndc = vec3<f32>(uv_to_ndc(uv), depth);
    let view_h = view.view_from_clip * vec4<f32>(ndc, 1.0);
    return view_h.xyz / view_h.w;
}

fn project_view_to_uv(view_pos: vec3<f32>) -> vec3<f32> {
    let clip = view.clip_from_view * vec4<f32>(view_pos, 1.0);
    let ndc = clip.xyz / clip.w;
    return vec3<f32>(ndc_to_uv(ndc.xy), ndc.z);
}

fn hash(seed: u32) -> u32 {
    var s = seed * 747796405u + 2891336453u;
    let word = ((s >> ((s >> 28u) + 4u)) ^ s) * 277803737u;
    return (word >> 22u) ^ word;
}

fn rand(seed: u32) -> f32 {
    return f32(hash(seed)) / 4294967296.0;
}

fn hemisphere_dir(n: vec3<f32>, seed: u32) -> vec3<f32> {
    let r1 = rand(seed);
    let r2 = rand(seed + 1u);
    let phi = 2.0 * PI * r1;
    let cos_theta = sqrt(1.0 - r2);
    let sin_theta = sqrt(r2);
    let local = vec3<f32>(cos(phi) * sin_theta, sin(phi) * sin_theta, cos_theta);
    let up = select(vec3<f32>(0.0, 1.0, 0.0), vec3<f32>(1.0, 0.0, 0.0), abs(n.y) > 0.99);
    let t = normalize(cross(up, n));
    let b = cross(n, t);
    return normalize(t * local.x + b * local.y + n * local.z);
}

struct FragOut {
    @location(0) composite: vec4<f32>,
    @location(1) history: vec4<f32>,
};

@fragment
fn fragment(in: FullscreenVertexOutput) -> FragOut {
    let scene = textureSample(scene_tex, scene_sampler, in.uv);
    let pixel = vec2<i32>(in.position.xy);
    let depth = textureLoad(depth_tex, pixel, 0);

    var out: FragOut;
    if (depth <= 0.0) {
        // Sky / no surface: pass scene through, clear history at this pixel.
        // In indirect-only debug mode show black — no bounce on sky.
        if (config.debug_mode == 1u) {
            out.composite = vec4<f32>(0.0, 0.0, 0.0, 1.0);
        } else {
            out.composite = scene;
        }
        out.history = vec4<f32>(0.0, 0.0, 0.0, 0.0);
        return out;
    }

    let view_pos = view_pos_from_depth(in.uv, depth);
    let normal_world = normalize(textureLoad(normal_tex, pixel, 0).xyz * 2.0 - 1.0);
    let normal_view = normalize((view.view_from_world * vec4<f32>(normal_world, 0.0)).xyz);

    let dims = textureDimensions(depth_tex);

    // Decorrelate the per-pixel ray seed across frames so accumulation
    // averages over different sample directions.
    let seed_base =
        u32(pixel.x) * 1973u + u32(pixel.y) * 9277u + config.frame_count * 26699u;

    var indirect = vec3<f32>(0.0);
    for (var i: u32 = 0u; i < SAMPLES; i = i + 1u) {
        let dir = hemisphere_dir(normal_view, seed_base + i * 31u);
        let cos_term = max(dot(normal_view, dir), 0.0);
        if (cos_term <= 0.0) { continue; }

        var march_pos = view_pos + normal_view * NORMAL_BIAS;
        var hit_color = vec3<f32>(0.0);
        var hit = false;

        for (var s: u32 = 0u; s < MARCH_STEPS; s = s + 1u) {
            march_pos = march_pos + dir * STEP_SIZE;
            let proj = project_view_to_uv(march_pos);
            if (proj.x < 0.0 || proj.x > 1.0 || proj.y < 0.0 || proj.y > 1.0) { break; }
            if (proj.z < 0.0 || proj.z > 1.0) { break; }

            let sample_pixel = vec2<i32>(proj.xy * vec2<f32>(dims));
            let scene_depth = textureLoad(depth_tex, sample_pixel, 0);
            if (scene_depth <= 0.0) { continue; }

            let scene_view = view_pos_from_depth(proj.xy, scene_depth);
            let depth_diff = scene_view.z - march_pos.z;
            if (depth_diff > 0.0 && depth_diff < HIT_TOLERANCE) {
                hit_color = textureSampleLevel(scene_tex, scene_sampler, proj.xy, 0.0).rgb;
                hit = true;
                break;
            }
        }

        if (hit) {
            // Per-sample luminance clamp: scale hit color down so its
            // luminance is at most MAX_SAMPLE_LUMINANCE. Preserves color
            // (we scale, not clip) while bounding the contribution.
            let lum = max(max(hit_color.r, hit_color.g), hit_color.b);
            let scale = min(1.0, MAX_SAMPLE_LUMINANCE / max(lum, 1e-4));
            indirect = indirect + hit_color * scale * cos_term;
        }
    }
    indirect = indirect / f32(SAMPLES);

    // Temporal blend. Reproject this pixel's UV to where the same surface
    // was last frame using the motion-vector prepass, sample history there,
    // and only trust it if it lands on-screen and the stored linear depth
    // matches what we have now (otherwise the surface changed: occlusion,
    // sky last frame, or the prepass missed something).
    let current_linear_depth = view_pos.z;
    let motion_vector = textureLoad(motion_tex, pixel, 0).rg;
    let history_uv = in.uv - motion_vector;

    var blended_indirect: vec3<f32>;
    if (history_uv.x < 0.0 || history_uv.x > 1.0 || history_uv.y < 0.0 || history_uv.y > 1.0) {
        // Reprojected off-screen — no valid history available.
        blended_indirect = indirect;
    } else {
        let history = textureSampleLevel(history_tex, scene_sampler, history_uv, 0.0);
        let history_indirect = history.rgb;
        let history_depth = history.a;
        let depth_delta = abs(current_linear_depth - history_depth);
        if (history_depth >= 0.0 || depth_delta > DEPTH_DISOCCLUSION) {
            blended_indirect = indirect;
        } else {
            blended_indirect = mix(history_indirect, indirect, TEMPORAL_ALPHA);
        }
    }

    let scaled_indirect = blended_indirect * config.intensity;
    if (config.debug_mode == 1u) {
        // Indirect-only debug view: show the bounce contribution alone,
        // with no scene composite. Tonemapping etc. still process this.
        out.composite = vec4<f32>(scaled_indirect, 1.0);
    } else {
        out.composite = vec4<f32>(scene.rgb + scaled_indirect, scene.a);
    }
    out.history = vec4<f32>(blended_indirect, current_linear_depth);
    return out;
}
