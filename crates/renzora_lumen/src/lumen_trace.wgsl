// Phase 7: voxel-cache GI cone trace with temporal accumulation.
//
// Pipeline per pixel:
//   1. Read depth + normal. Sky → pass scene through, clear history.
//   2. Reconstruct world position; cast N cosine-weighted cones
//      through the voxel clipmap. Each cone is a front-to-back
//      alpha-integrated march with manual trilinear voxel sampling
//      and a step size that grows with distance (widening cone).
//   3. Sample motion vectors → reproject UV to where this surface was
//      last frame; sample history there. Reject if off-screen or the
//      stored linear depth disagrees with the current pixel's depth.
//   4. Blend current trace with valid history.
//   5. Output 0: scene + blended indirect (composite).
//      Output 1: blended indirect + current linear depth → next-frame
//                history.
//
// Bevy's motion-vector convention (from `bevy_pbr/.../prepass.wgsl`):
//   `motion_vector = (clip - prev_clip) * vec2(0.5, -0.5)`. So
//   `history_uv = uv - motion_vector`.

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

struct TraceConfig {
    intensity: f32,
    frame_count: u32,
    debug_mode: u32, // 0 = composite, 1 = indirect-only
    quality_tier: u32, // 0 = SdfLow, 1 = SdfHigh
    sky_intensity: f32, // multiplier for sky cubemap on cone miss
    use_albedo_modulation: u32, // 0 = receiver albedo = white, 1 = read G-buffer
    specular_intensity: f32, // 0 = no specular cone trace; >0 enables it
    _pad0: u32,
};

@group(0) @binding(0) var scene_tex: texture_2d<f32>;
@group(0) @binding(1) var scene_sampler: sampler;
@group(0) @binding(2) var depth_tex: texture_depth_2d;
@group(0) @binding(3) var normal_tex: texture_2d<f32>;
@group(0) @binding(4) var voxels: texture_3d<f32>;
@group(0) @binding(5) var voxel_sampler: sampler;
@group(0) @binding(6) var history_tex: texture_2d<f32>;
@group(0) @binding(7) var motion_tex: texture_2d<f32>;
@group(0) @binding(8) var<uniform> view: View;
@group(0) @binding(9) var<uniform> grid: VoxelGrid;
@group(0) @binding(10) var<uniform> config: TraceConfig;
@group(0) @binding(11) var sky_cube: texture_cube<f32>;
@group(0) @binding(12) var sky_sampler: sampler;
@group(0) @binding(13) var gbuffer: texture_2d<u32>;
// Half-res reflection pyramid (Rgba16Float, N mip levels). Mip 0 is
// the raw screen-space trace; mips 1..N are progressively blurrier
// (built by `screen_reflection_blur` as a separable Gaussian + 2×
// downsample per level). Sampled with `textureSampleLevel(..., uv,
// mip_level)` — bilinear in XY, point-wise across mips. The blur
// pyramid is what makes roughness-aware reflections cheap.
@group(0) @binding(14) var reflection_tex: texture_2d<f32>;
// Per-pixel mip level scalar (R16Float, half-res). Sampled bilinearly
// to full res so the upsampled LOD reads smoothly across the pyramid.
// Encodes blur radius via Godot's cone-of-confusion → log mip formula
// in `screen_reflection.wgsl`.
@group(0) @binding(15) var mip_level_tex: texture_2d<f32>;

// SdfLow defaults; SdfHigh upgrades these via `config.quality_tier == 1u`.
// Lower step counts than Phase 6's first-hit march: each cone step is
// ~8x heavier (manual trilinear) but covers more distance (step-size
// grows ~10% per iteration) and often early-exits on full alpha.
const SAMPLES_LOW: u32 = 2u;
const SAMPLES_HIGH: u32 = 4u;
const MAX_STEPS_LOW: u32 = 12u;
const MAX_STEPS_HIGH: u32 = 20u;

// Distance falloff for the cone trace. Bounce radiance from a voxel at
// distance `d` from the surface is attenuated by `1 / (1 + k * d²)`.
// k = 0.15 is mild compared to physical inverse-square (k = 1.0) but
// enough to localise bounce near sources — voxel-cache sparseness means
// strict 1/d² leaves the scene mostly dark. Tune up for darker scenes,
// down for brighter / more diffuse bounce.
//
// This compensates for the fact that we don't yet have a voxel mipmap
// chain: a proper Lumen/SDFGI cone trace would sample at LOD =
// log2(diameter), so distant content is naturally averaged into wider
// taps without needing this hack. Phase 8 task to do that properly and
// remove this constant.
const DISTANCE_FALLOFF: f32 = 0.15;

// Half-angle tangent for the diffuse cone. 0.577 ≈ tan(30°), giving a
// 60° cone — same value Godot's SDFGI uses for its 6-cone diffuse
// gather. Specular cones use a much narrower angle derived from
// surface roughness (see fragment shader). Step size = half the cone
// diameter at the current distance, so adjacent steps overlap by 50%
// (no gaps, minimal redundant work).
const TAN_HALF_ANGLE_DIFFUSE: f32 = 0.577;
// Specular cone width is clamped so the trace never degenerates to
// a sub-voxel ray (which would alias against the voxel grid). 0.05 ≈
// tan(2.86°), narrow enough for "glossy" but still wider than one
// voxel at our typical view distance.
const TAN_HALF_ANGLE_SPEC_MIN: f32 = 0.05;
// Specular cones also cap at the diffuse cone width — anything wider
// than 30° half-angle is effectively diffuse anyway.
const TAN_HALF_ANGLE_SPEC_MAX: f32 = 0.577;

// The screen-space hybrid trace previously inlined here has moved to
// a dedicated half-res compute pass (`screen_reflection.wgsl`), part
// of Stage 1 of the Godot-style filter pipeline. The composite pass
// below samples the resolved buffer with linear filtering for a
// 2× bilinear upsample — stages 2-4 will add proper Gaussian blur,
// a mip pyramid, and bilateral upsample on top of that buffer.
// Push the ray origin a full voxel along the normal so the very first
// march step doesn't immediately self-hit the surface voxel.
const NORMAL_BIAS: f32 = 1.5;
const PI: f32 = 3.14159265359;

// Per-sample luminance clamp suppresses fireflies — a single ray hitting
// a very bright voxel can dominate the 2-sample average and become a
// persistent bright dot once temporal kicks in.
const MAX_SAMPLE_LUMINANCE: f32 = 4.0;
// Per-ray sky contribution ceiling. Tighter than the general per-sample
// clamp because (a) Bevy's diffuse-prefiltered daytime sky can hit
// 10-50 HDR units, and (b) in open urban scenes nearly every cone ray
// escapes to open sky, so the sum stacks fast and the scene goes
// uniformly bluish-white. 1.0 is conservative enough to keep sky an
// "ambient fill" rather than a primary light source. Until we have
// proper albedo modulation, this is the cleanest gate.
const MAX_SKY_LUMINANCE: f32 = 1.0;
// 0.08 = 8% current / 92% history → ~12-frame half-life. Slow accumulation
// kills the noise hard; the cost is response lag for moving lights.
const TEMPORAL_ALPHA: f32 = 0.08;
// View-space linear-depth delta beyond which the reprojected pixel is
// treated as a different surface and history is dropped.
const DEPTH_DISOCCLUSION: f32 = 0.5;

fn uv_to_ndc(uv: vec2<f32>) -> vec2<f32> {
    return uv * vec2<f32>(2.0, -2.0) + vec2<f32>(-1.0, 1.0);
}

fn world_pos_from_depth(uv: vec2<f32>, depth: f32) -> vec3<f32> {
    let ndc = vec3<f32>(uv_to_ndc(uv), depth);
    let world_h = view.world_from_clip * vec4<f32>(ndc, 1.0);
    return world_h.xyz / world_h.w;
}

fn view_pos_from_depth(uv: vec2<f32>, depth: f32) -> vec3<f32> {
    let ndc = vec3<f32>(uv_to_ndc(uv), depth);
    let view_h = view.view_from_clip * vec4<f32>(ndc, 1.0);
    return view_h.xyz / view_h.w;
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

// Manual trilinear voxel sample. Clamps within the current cascade's Z
// slice so we never blend across the cascade boundary in the stacked
// mega-texture — adjacent cascades have different voxel sizes and the
// blend would be physically meaningless.
fn cascade_voxel_sample(p: vec3<f32>, cascade: u32) -> vec4<f32> {
    let info = grid.cascades[cascade];
    let res_i = i32(grid.resolution);
    let z_base = i32(cascade) * res_i;
    // Voxel-space position, biased by 0.5 so integer lattice sits at
    // voxel centers (matches how `cascade_voxel_load` indexed before).
    let local = (p - info.origin) / info.voxel_size - vec3<f32>(0.5);
    let i0 = vec3<i32>(floor(local));
    let frac = local - vec3<f32>(i0);

    var sum = vec4<f32>(0.0);
    for (var k: i32 = 0; k < 8; k = k + 1) {
        let d = vec3<i32>(k & 1, (k >> 1) & 1, (k >> 2) & 1);
        let i = clamp(i0 + d, vec3<i32>(0), vec3<i32>(res_i - 1));
        let w = mix(1.0 - frac.x, frac.x, f32(d.x))
              * mix(1.0 - frac.y, frac.y, f32(d.y))
              * mix(1.0 - frac.z, frac.z, f32(d.z));
        sum = sum + textureLoad(voxels, vec3<i32>(i.x, i.y, i.z + z_base), 0) * w;
    }
    return sum;
}

// Cone trace: front-to-back alpha integration with step size that grows
// with distance to approximate a widening cone. Stops on full coverage
// or when the march leaves the clipmap.
//
// Distance falloff attenuates the radiance contribution from each voxel
// based on how far the cone has travelled — bounce localises near the
// shaded surface. Note the occlusion accumulator (acc_alpha) is *not*
// attenuated: a wall 5m away still blocks bounce from further behind it,
// it just doesn't contribute as much of its own light back to the
// shaded surface.
//
// On miss (cone leaves the clipmap or runs out of steps with remaining
// alpha budget), fills the unfilled portion with the prefiltered sky
// cubemap sampled in the cone direction. This is what gives upward-
// facing surfaces ambient sky bounce when no voxel cache content is
// available — without it, sky-facing rays return pure black and the
// scene loses outdoor ambient.
fn trace_voxel_cone(origin: vec3<f32>, dir: vec3<f32>, max_steps: u32, tan_half_angle: f32) -> vec3<f32> {
    var p = origin;
    var acc_color = vec3<f32>(0.0);
    var acc_alpha = 0.0;
    var distance_travelled = 0.0;

    for (var i: u32 = 0u; i < max_steps; i = i + 1u) {
        let cascade = select_cascade(p);
        if (cascade < 0) { break; }
        let voxel = cascade_voxel_sample(p, u32(cascade));

        // Coverage = how much of the remaining cone budget this voxel
        // fills. Drives both how much light we accept and how much of
        // the cone is "blocked" going further.
        let coverage = voxel.a * (1.0 - acc_alpha);
        let falloff = 1.0 / (1.0 + DISTANCE_FALLOFF * distance_travelled * distance_travelled);
        acc_color = acc_color + voxel.rgb * coverage * falloff;
        acc_alpha = acc_alpha + coverage;
        if (acc_alpha >= 0.95) { break; }

        // Proper cone step: diameter widens linearly with distance from
        // the cone origin, step = diameter / 2 (50% overlap between
        // successive samples — no gaps). Clamped to one inner voxel so
        // we don't take sub-voxel steps near the origin.
        let inner_size = grid.cascades[0].voxel_size;
        let diameter = max(inner_size, 2.0 * tan_half_angle * distance_travelled);
        let step_dist = diameter * 0.5;
        p = p + dir * step_dist;
        distance_travelled = distance_travelled + step_dist;
    }

    // Sky-cubemap fallback for the remaining unfilled cone budget.
    // `sky_intensity` is 0 when the WE entity has no env map (or when
    // sun_horizon_factor faded it to 0 for night), so this term
    // disappears cleanly without needing a runtime branch.
    //
    // Per-channel luminance clamp at MAX_SKY_LUMINANCE — preserves
    // colour ratio (scale, don't clip) so blue sky still reads blue,
    // but ceilinged so an open-sky scene doesn't stack to white. Until
    // we have albedo modulation, this is a conservative "ambient fill"
    // contribution rather than a primary light source.
    if (acc_alpha < 0.95) {
        let sky_raw = textureSampleLevel(sky_cube, sky_sampler, dir, 0.0).rgb;
        let sky_lum = max(max(sky_raw.r, sky_raw.g), sky_raw.b);
        let sky_scale = min(1.0, MAX_SKY_LUMINANCE / max(sky_lum, 1e-4));
        let sky = sky_raw * sky_scale;
        acc_color = acc_color + (1.0 - acc_alpha) * sky * config.sky_intensity;
    }

    return acc_color;
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
        // Sky: pass scene through, clear history at this pixel.
        if (config.debug_mode == 1u) {
            out.composite = vec4<f32>(0.0, 0.0, 0.0, 1.0);
        } else {
            out.composite = scene;
        }
        out.history = vec4<f32>(0.0, 0.0, 0.0, 0.0);
        return out;
    }

    let world_pos = world_pos_from_depth(in.uv, depth);
    let view_pos = view_pos_from_depth(in.uv, depth);
    let normal_world = normalize(textureLoad(normal_tex, pixel, 0).xyz * 2.0 - 1.0);

    let inner_voxel_size = grid.cascades[0].voxel_size;
    let origin = world_pos + normal_world * (NORMAL_BIAS * inner_voxel_size);

    let seed_base =
        u32(pixel.x) * 1973u + u32(pixel.y) * 9277u + config.frame_count * 26699u;

    let samples = select(SAMPLES_LOW, SAMPLES_HIGH, config.quality_tier == 1u);
    let max_steps = select(MAX_STEPS_LOW, MAX_STEPS_HIGH, config.quality_tier == 1u);

    var indirect = vec3<f32>(0.0);
    for (var i: u32 = 0u; i < samples; i = i + 1u) {
        let dir = hemisphere_dir(normal_world, seed_base + i * 31u);
        var hit_rgb = trace_voxel_cone(origin, dir, max_steps, TAN_HALF_ANGLE_DIFFUSE);
        // Per-sample luminance clamp: scale (not clip) so color is
        // preserved while bounding contribution.
        let lum = max(max(hit_rgb.r, hit_rgb.g), hit_rgb.b);
        let scale = min(1.0, MAX_SAMPLE_LUMINANCE / max(lum, 1e-4));
        indirect = indirect + hit_rgb * scale;
    }
    indirect = indirect / f32(samples);

    let current_linear_depth = view_pos.z;
    let motion_vector = textureLoad(motion_tex, pixel, 0).rg;
    let history_uv = in.uv - motion_vector;

    var blended_indirect: vec3<f32>;
    if (history_uv.x < 0.0 || history_uv.x > 1.0 || history_uv.y < 0.0 || history_uv.y > 1.0) {
        blended_indirect = indirect;
    } else {
        let history = textureSampleLevel(history_tex, scene_sampler, history_uv, 0.0);
        let history_indirect = history.rgb;
        let history_depth = history.a;
        let depth_delta = abs(current_linear_depth - history_depth);
        // history_depth >= 0.0 means "no surface last frame" (sky branch
        // writes 0.0; view-space Z on real surfaces is negative in Bevy).
        if (history_depth >= 0.0 || depth_delta > DEPTH_DISOCCLUSION) {
            blended_indirect = indirect;
        } else {
            blended_indirect = mix(history_indirect, indirect, TEMPORAL_ALPHA);
        }
    }

    // Unpack G-buffer once: R channel packs
    // `(base_color_srgb, perceptual_roughness)` as unorm4x8. We use
    // `.rgb` for receiver albedo (diffuse modulation) and `.a` for
    // roughness (specular cone width). Gated on `use_albedo_modulation`
    // since Forward views have a dummy 1x1 uint texture bound.
    var albedo = vec3<f32>(1.0);
    var perceptual_roughness = 1.0;
    if (config.use_albedo_modulation == 1u) {
        let gb = textureLoad(gbuffer, pixel, 0).r;
        let base_rough = unpack4x8unorm(gb);
        albedo = pow(base_rough.rgb, vec3<f32>(2.2));
        perceptual_roughness = base_rough.a;
    }

    // ── Specular voxel-cone trace ────────────────────────────────
    // One cone in the reflection direction, cone width derived from
    // surface roughness. Single ray suffices because the cone integral
    // already does the BRDF lobe integration — a wider cone IS a
    // rougher reflection. Sharp surfaces get a narrow cone, rough
    // surfaces get a wide one that converges with the diffuse trace.
    //
    // Why this exists at all: screen-space reflections (Bevy's SSR)
    // can only reflect on-screen content. The far/left/right edges of
    // reflective surfaces fall back to IBL because SSR has nothing to
    // march into. Voxel-cone specular reads from the same world-space
    // voxel cache as the diffuse trace, so it covers off-screen and
    // behind-camera reflections that SSR can't reach.
    //
    // v1 caveats:
    //   * Dielectric F0 = 0.04 (no metallic from G-buffer yet).
    //   * No temporal accumulation; one fresh trace per frame.
    //   * Layered ADDITIVELY on top of Bevy's IBL specular and any
    //     active SSR. `specular_intensity` (default 0.3) tunes the
    //     amount; crank to 1.0 with SSR off for "voxel only" mode.
    var specular = vec3<f32>(0.0);
    if (config.specular_intensity > 0.0) {
        // `roughness²` is Bevy's convention for the BRDF roughness
        // input (perceptual is roughly "what an artist sets"). Cone
        // half-angle scales with that — quadratic falloff matches the
        // GGX lobe broadening, narrow at low roughness, wide at high.
        let linear_roughness = perceptual_roughness * perceptual_roughness;
        let spec_tan_half = clamp(
            linear_roughness,
            TAN_HALF_ANGLE_SPEC_MIN,
            TAN_HALF_ANGLE_SPEC_MAX,
        );

        // View-space reflection vector. world_pos and view position
        // give us the incoming direction; reflect across the surface
        // normal to get the outgoing reflection ray.
        let view_dir = normalize(view.world_position.xyz - world_pos);
        let reflect_dir = reflect(-view_dir, normal_world);

        // Hybrid: half-res screen-space reflection pyramid (built by
        // `screen_reflection` + `screen_reflection_blur` earlier in
        // the frame) blended with a world-space voxel cone for
        // off-screen / behind-camera coverage. The per-pixel mip_level
        // — computed from roughness × ray_length in the trace stage,
        // bilinearly upsampled here — picks how blurry to read the
        // pyramid. Mip 0 = sharp screen-space hit (mirror-like
        // surfaces), mip N = wide cone integration (rough surfaces).
        //
        // Trace from the same biased origin as the diffuse pass to
        // avoid self-hits on the surface voxel.
        let spec_steps = select(MAX_STEPS_LOW, MAX_STEPS_HIGH, config.quality_tier == 1u);
        let mip_level_pixel = textureSampleLevel(mip_level_tex, scene_sampler, in.uv, 0.0).r;
        let screen = textureSampleLevel(reflection_tex, scene_sampler, in.uv, mip_level_pixel);
        let voxel_spec = trace_voxel_cone(origin, reflect_dir, spec_steps, spec_tan_half);
        let spec_rgb = mix(voxel_spec, screen.rgb, screen.a);

        // Schlick Fresnel: F0 + (1 - F0)(1 - cos_theta)^5. For
        // dielectrics F0 ≈ 0.04. View-angle dependent: grazing angles
        // get near-full reflection, head-on surfaces show base F0
        // only.
        let f0 = 0.04;
        let cos_theta = max(dot(view_dir, normal_world), 0.0);
        let fresnel = f0 + (1.0 - f0) * pow(1.0 - cos_theta, 5.0);

        // Smoothness attenuation: rough surfaces shouldn't get
        // directional specular — their reflection is already
        // scattered diffusely by the wide-hemisphere diffuse cone
        // trace. Without this attenuation, even at perceptual
        // roughness = 1.0 the cone width caps at 30° half-angle, so
        // tarmac/concrete/matte materials would still show mirror-like
        // reflection at grazing angles where Fresnel is strong. The
        // `(1 - roughness)²` curve matches how GGX lobes vanish into
        // the hemisphere as roughness increases. Glass at 0.0 keeps
        // full specular; tarmac at ~0.9 contributes ~1%.
        let smoothness = 1.0 - perceptual_roughness;
        let smoothness_factor = smoothness * smoothness;

        specular = spec_rgb * fresnel * smoothness_factor * config.specular_intensity;
    }

    let scaled_indirect = blended_indirect * albedo * config.intensity;
    if (config.debug_mode == 1u) {
        // Debug view shows diffuse only — specular is suppressed by
        // setting `specular_intensity` to 0 host-side in that mode.
        out.composite = vec4<f32>(scaled_indirect, 1.0);
    } else {
        out.composite = vec4<f32>(scene.rgb + scaled_indirect + specular, scene.a);
    }
    // Keep history un-modulated. Albedo can change frame to frame (e.g.
    // texture animation, light-color changes); storing pre-albedo
    // indirect lets temporal accumulation operate on a stable signal
    // and re-modulate fresh each frame. Specular intentionally not in
    // history — reflection direction is view-dependent, so reprojected
    // specular is almost always invalid.
    out.history = vec4<f32>(blended_indirect, current_linear_depth);
    return out;
}
