// Separable 7-tap Gaussian blur for the reflection mip pyramid.
//
// One pipeline variant per direction (selected via `#define HORIZONTAL`):
//   - Horizontal: source = previous mip, destination = temp buffer
//   - Vertical:   source = temp buffer,  destination = current mip
//
// Sample positions are in TARGET-pixel coordinates centred at the
// output pixel, with each tap offset by ±1, ±2, ±3 pixels in the
// blur direction. Source is sampled with linear filtering — since
// the destination is half the source resolution (we blur into a
// smaller mip), this naturally implements a 2× downsample with the
// Gaussian weights baking out aliasing.
//
// Weights from Godot's `screen_space_reflection_filter.glsl`. They
// match a σ ≈ 1.4 Gaussian truncated to ±3 — a sensible balance
// between visibly-smooth blur and tight enough footprint to avoid
// haloing near reflection edges.

@group(0) @binding(0) var source: texture_2d<f32>;
@group(0) @binding(1) var source_sampler: sampler;
@group(0) @binding(2) var dest: texture_storage_2d<rgba16float, write>;

const WEIGHTS: array<f32, 7> = array<f32, 7>(
    0.07130343198685299,
    0.1315141208431224,
    0.18987923288883812,
    0.21460642856237303,
    0.18987923288883812,
    0.1315141208431224,
    0.07130343198685299,
);

@compute @workgroup_size(8, 8, 1)
fn blur(@builtin(global_invocation_id) gid: vec3<u32>) {
    let dest_size = textureDimensions(dest);
    if (any(gid.xy >= dest_size)) {
        return;
    }

    let source_size = vec2<f32>(textureDimensions(source));
    let dest_size_f = vec2<f32>(dest_size);
    // UV of this output pixel's *centre* in source space. The
    // source is roughly 2× the destination (one pyramid level
    // higher), so each output pixel pulls from a 2×2 block. The
    // linear sampler does the 4-tap bilinear bit automatically.
    let center_uv = (vec2<f32>(gid.xy) + vec2<f32>(0.5)) / dest_size_f;

    // Step in source-UV space corresponding to one source pixel in
    // the blur direction. Computed from inverse source size so the
    // blur footprint stays consistent regardless of mip resolution.
    let texel_step = 1.0 / source_size;
#ifdef HORIZONTAL
    let offset_dir = vec2<f32>(texel_step.x, 0.0);
#else
    let offset_dir = vec2<f32>(0.0, texel_step.y);
#endif

    var acc = vec4<f32>(0.0);
    for (var i: i32 = -3; i <= 3; i = i + 1) {
        let w = WEIGHTS[i + 3];
        let uv = center_uv + offset_dir * f32(i);
        acc = acc + textureSampleLevel(source, source_sampler, uv, 0.0) * w;
    }

    textureStore(dest, vec2<i32>(gid.xy), acc);
}
