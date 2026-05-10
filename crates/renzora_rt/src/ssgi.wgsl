// Screen-space global illumination — Phase 1 minimal.
//
// For each visible fragment: reconstruct view-space position from depth,
// shoot N hemisphere rays oriented around the world-space normal, march
// each in view space comparing against the depth buffer, sample scene
// color at hits, accumulate, add to scene.
//
// Knowingly cheap: no Hi-Z (linear march), no temporal accumulation,
// no spatial denoise. Phases 5+ of the Lumen plan layer those on.

#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput
#import bevy_render::view::View

struct RtConfig {
    intensity: f32,
    _pad0: f32,
    _pad1: f32,
    _pad2: f32,
};

@group(0) @binding(0) var scene_tex: texture_2d<f32>;
@group(0) @binding(1) var scene_sampler: sampler;
@group(0) @binding(2) var depth_tex: texture_depth_2d;
@group(0) @binding(3) var normal_tex: texture_2d<f32>;
@group(0) @binding(4) var<uniform> view: View;
@group(0) @binding(5) var<uniform> config: RtConfig;

const SAMPLES: u32 = 4u;
const MARCH_STEPS: u32 = 12u;
const STEP_SIZE: f32 = 0.35;
const NORMAL_BIAS: f32 = 0.05;
const HIT_TOLERANCE: f32 = 0.6;
const PI: f32 = 3.14159265359;

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

// 32-bit integer hash (PCG)
fn hash(seed: u32) -> u32 {
    var s = seed * 747796405u + 2891336453u;
    let word = ((s >> ((s >> 28u) + 4u)) ^ s) * 277803737u;
    return (word >> 22u) ^ word;
}

fn rand(seed: u32) -> f32 {
    return f32(hash(seed)) / 4294967296.0;
}

// Cosine-weighted hemisphere sample around `n`.
fn hemisphere_dir(n: vec3<f32>, seed: u32) -> vec3<f32> {
    let r1 = rand(seed);
    let r2 = rand(seed + 1u);
    let phi = 2.0 * PI * r1;
    let cos_theta = sqrt(1.0 - r2);
    let sin_theta = sqrt(r2);
    let local = vec3<f32>(cos(phi) * sin_theta, sin(phi) * sin_theta, cos_theta);

    // Build orthonormal basis around n.
    let up = select(vec3<f32>(0.0, 1.0, 0.0), vec3<f32>(1.0, 0.0, 0.0), abs(n.y) > 0.99);
    let t = normalize(cross(up, n));
    let b = cross(n, t);
    return normalize(t * local.x + b * local.y + n * local.z);
}

@fragment
fn fragment(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let scene = textureSample(scene_tex, scene_sampler, in.uv);

    // Sky / no-prepass: pass scene through.
    let pixel = vec2<i32>(in.position.xy);
    let depth = textureLoad(depth_tex, pixel, 0);
    if (depth <= 0.0) {
        return scene;
    }

    let view_pos = view_pos_from_depth(in.uv, depth);
    let normal_world = normalize(textureLoad(normal_tex, pixel, 0).xyz * 2.0 - 1.0);
    // Transform world normal to view space (rotation only; mat3 cast).
    let normal_view = normalize((view.view_from_world * vec4<f32>(normal_world, 0.0)).xyz);

    let dims = textureDimensions(depth_tex);
    let seed_base = u32(pixel.x) * 1973u + u32(pixel.y) * 9277u + view.frame_count * 26699u;

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
            // In Bevy's reverse-Z view space, view_z is negative going away
            // from the camera. A "hit" means our marched point is behind the
            // scene surface by a small amount.
            let depth_diff = scene_view.z - march_pos.z;
            if (depth_diff > 0.0 && depth_diff < HIT_TOLERANCE) {
                hit_color = textureSampleLevel(scene_tex, scene_sampler, proj.xy, 0.0).rgb;
                hit = true;
                break;
            }
        }

        if (hit) {
            indirect = indirect + hit_color * cos_term;
        }
    }

    indirect = indirect / f32(SAMPLES);
    return vec4<f32>(scene.rgb + indirect * config.intensity, scene.a);
}
