// Bilateral upsample resolve — Stage 4 of the Godot-style reflection
// filter pipeline.
//
// For each full-res output pixel, sample the four nearest half-res
// pyramid taps. Weight each neighbour by similarity in:
//   * depth   — `exp(-depth_diff * 2048)`     (Godot's DEPTH_FACTOR)
//   * normal  — `exp(-normal_diff * 32)`      (NORMAL_FACTOR)
//   * roughness — `exp(-roughness_diff * 16)` (ROUGHNESS_FACTOR)
//
// Multiplied by the standard 2×2 bilinear partition. Result: smooth
// upscale within a single material/depth surface, hard cutoff at
// boundaries — no reflection bleed across edges.
//
// The mip level to sample is read per-pixel from `mip_level_tex`
// (also bilinear-upscaled), so a smooth surface inside an
// otherwise-rough region picks its own sharper mip without being
// dragged blurry by neighbours.

@group(0) @binding(0) var reflection_pyramid: texture_2d<f32>;
@group(0) @binding(1) var pyramid_sampler: sampler;
@group(0) @binding(2) var mip_level_tex: texture_2d<f32>;
@group(0) @binding(3) var depth_tex: texture_depth_2d;
@group(0) @binding(4) var normal_tex: texture_2d<f32>;
@group(0) @binding(5) var gbuffer: texture_2d<u32>;
@group(0) @binding(6) var output_resolved: texture_storage_2d<rgba16float, write>;

const DEPTH_FACTOR: f32 = 2048.0;
const NORMAL_FACTOR: f32 = 32.0;
const ROUGHNESS_FACTOR: f32 = 16.0;

struct Neighbour {
    color: vec4<f32>,
    depth: f32,
    normal: vec3<f32>,
    roughness: f32,
};

fn sample_neighbour(
    half_pixel: vec2<i32>,
    full_size: vec2<i32>,
    mip: f32,
) -> Neighbour {
    var out: Neighbour;
    out.color = vec4<f32>(0.0);
    out.depth = 0.0;
    out.normal = vec3<f32>(0.0, 0.0, 1.0);
    out.roughness = 0.5;

    // Representative full-res pixel for this half-res sample (top-left
    // of the 2×2 block — matches the convention `screen_reflection.wgsl`
    // uses on the trace side).
    let full_pixel = clamp(half_pixel * 2, vec2<i32>(0), full_size - vec2<i32>(1));

    let depth = textureLoad(depth_tex, full_pixel, 0);
    if (depth <= 0.0) {
        // Sky neighbour — leave depth=0 so the weight gets killed by
        // the depth term against any solid centre pixel.
        return out;
    }

    out.depth = depth;
    out.normal = normalize(textureLoad(normal_tex, full_pixel, 0).xyz * 2.0 - 1.0);

    let gb = textureLoad(gbuffer, full_pixel, 0).r;
    let base_rough = unpack4x8unorm(gb);
    out.roughness = base_rough.a;

    // Sample the pyramid colour at the half-res pixel's UV using the
    // chosen mip level. Linear filter handles the in-mip lookup; the
    // mip itself is integer-trilinear via textureSampleLevel.
    let half_size = vec2<f32>(textureDimensions(reflection_pyramid));
    let uv = (vec2<f32>(half_pixel) + vec2<f32>(0.5)) / half_size;
    out.color = textureSampleLevel(reflection_pyramid, pyramid_sampler, uv, mip);

    return out;
}

@compute @workgroup_size(8, 8, 1)
fn resolve(@builtin(global_invocation_id) gid: vec3<u32>) {
    let pixel = vec2<i32>(gid.xy);
    let full_size = vec2<i32>(textureDimensions(depth_tex));
    if (any(pixel >= full_size)) {
        return;
    }

    // Centre pixel data — what we're weighting neighbours against.
    let centre_depth = textureLoad(depth_tex, pixel, 0);
    if (centre_depth <= 0.0) {
        // Sky pixel — no reflection to resolve. Write zero, which
        // also signals "no reflection contribution" via alpha=0 to
        // `lumen_trace`'s blend.
        textureStore(output_resolved, pixel, vec4<f32>(0.0));
        return;
    }

    let centre_normal = normalize(textureLoad(normal_tex, pixel, 0).xyz * 2.0 - 1.0);
    let centre_gb = textureLoad(gbuffer, pixel, 0).r;
    let centre_rough = unpack4x8unorm(centre_gb).a;

    // Per-pixel mip choice, bilinear-upsampled from half-res.
    let full_size_f = vec2<f32>(full_size);
    let uv = (vec2<f32>(pixel) + vec2<f32>(0.5)) / full_size_f;
    let mip = textureSampleLevel(mip_level_tex, pyramid_sampler, uv, 0.0).r;

    // Half-res neighbour anchor — half_pixel.xy in [0, half_size).
    // Use the floor of half_pos so the 2×2 block surrounds the full-res
    // pixel correctly for the bilinear partition.
    let half_pos = (vec2<f32>(pixel) + vec2<f32>(0.5)) * 0.5 - vec2<f32>(0.5);
    let half_pixel_tl = vec2<i32>(floor(half_pos));
    let frac = half_pos - vec2<f32>(half_pixel_tl);

    // Bilinear partition over the 2×2 half-res neighbours.
    let bilinear = vec4<f32>(
        (1.0 - frac.x) * (1.0 - frac.y), // TL
        frac.x * (1.0 - frac.y),         // TR
        (1.0 - frac.x) * frac.y,         // BL
        frac.x * frac.y,                 // BR
    );

    let neighbours = array<vec2<i32>, 4>(
        half_pixel_tl + vec2<i32>(0, 0),
        half_pixel_tl + vec2<i32>(1, 0),
        half_pixel_tl + vec2<i32>(0, 1),
        half_pixel_tl + vec2<i32>(1, 1),
    );

    var sum_color = vec4<f32>(0.0);
    var sum_weight = 0.0;

    for (var i: i32 = 0; i < 4; i = i + 1) {
        let n = sample_neighbour(neighbours[i], full_size, mip);

        // Bilateral weights — exponentials match Godot's
        // `screen_space_reflection_resolve.glsl`. Each falls to ~0
        // very quickly: depth_diff > 0.005 kills the depth term,
        // normal angle > 8° kills the normal term, etc. Edges get
        // hard cutoffs without per-pixel branches.
        let depth_diff = abs(centre_depth - n.depth);
        let w_depth = exp(-depth_diff * DEPTH_FACTOR);

        let normal_diff = clamp(1.0 - dot(centre_normal, n.normal), 0.0, 1.0);
        let w_normal = exp(-normal_diff * NORMAL_FACTOR);

        let rough_diff = abs(centre_rough - n.roughness);
        let w_rough = exp(-rough_diff * ROUGHNESS_FACTOR);

        let weight = bilinear[i] * w_depth * w_normal * w_rough;
        sum_color = sum_color + n.color * weight;
        sum_weight = sum_weight + weight;
    }

    var resolved = vec4<f32>(0.0);
    if (sum_weight > 0.0) {
        resolved = sum_color / sum_weight;
    }
    textureStore(output_resolved, pixel, resolved);
}
