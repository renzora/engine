// Bakes the two noise volumes the cloud raymarch samples. Runs once, on the
// first frame the pipelines are ready, and never again — the fields are static
// and the wind moves the *sample* position, not the noise.
//
// Both are tileable: every octave wraps its lattice at its own frequency, so
// sampling with a Repeat sampler leaves no seam however far a ray travels.
// This is the Frostbite tileable-noise recipe (Hillaire / sebh's
// TileableVolumeNoise), which is what the Horizon Zero Dawn cloud model wants:
//
//   * `base` (2D, RGBA16F) — the weather/shape atlas, sampled by world XZ.
//       r = Perlin-FBM × Worley, the cloud silhouette
//       g = a Worley sum in [-1, 0], the low end of the remap window; it
//           pushes the silhouette's edge in or out per-region so coverage
//           doesn't advance uniformly everywhere
//       b = a fine Worley used as the height-gradient modifier, so tops billow
//           and bases stay flat
//   * `detail` (3D 32³, RGBA16F) — the high-frequency Worley that erodes the
//       base shape into wisps.
//
// RGBA16F rather than a narrower format for two reasons: the `g` channel is
// negative, and 16-bit float is the narrowest storage-capable format WebGPU
// guarantees for a 3D texture.

@group(0) @binding(0) var base_texture: texture_storage_2d<rgba16float, write>;
@group(0) @binding(1) var detail_texture: texture_storage_3d<rgba16float, write>;

// Hash without Sine, by Dave Hoskins — https://www.shadertoy.com/view/4djSRW
fn hash13(p_in: vec3<f32>) -> f32 {
    var p = fract(p_in * 1031.1031);
    p += dot(p, p.yzx + 19.19);
    return fract((p.x + p.y) * p.z);
}

fn value_hash(p_in: vec3<f32>) -> f32 {
    var p = fract(p_in * 0.1031);
    p += dot(p, p.yzx + 19.19);
    return fract((p.x + p.y) * p.z);
}

// Positive modulo. WGSL's `%` keeps the sign of the dividend, so a cell offset
// of -1 at lattice origin wraps to -1 instead of `tile - 1` and the noise stops
// tiling exactly at the seam it was built to hide.
fn wrap3(v: vec3<f32>, tile: f32) -> vec3<f32> {
    return ((v % tile) + tile) % tile;
}

// Value noise on a lattice that wraps at `tile`.
fn tiled_value_noise(x: vec3<f32>, tile: f32) -> f32 {
    let p = floor(x);
    var f = fract(x);
    f = f * f * (3.0 - 2.0 * f);

    let c000 = value_hash(wrap3(p, tile));
    let c100 = value_hash(wrap3(p + vec3<f32>(1.0, 0.0, 0.0), tile));
    let c010 = value_hash(wrap3(p + vec3<f32>(0.0, 1.0, 0.0), tile));
    let c110 = value_hash(wrap3(p + vec3<f32>(1.0, 1.0, 0.0), tile));
    let c001 = value_hash(wrap3(p + vec3<f32>(0.0, 0.0, 1.0), tile));
    let c101 = value_hash(wrap3(p + vec3<f32>(1.0, 0.0, 1.0), tile));
    let c011 = value_hash(wrap3(p + vec3<f32>(0.0, 1.0, 1.0), tile));
    let c111 = value_hash(wrap3(p + vec3<f32>(1.0, 1.0, 1.0), tile));

    return mix(
        mix(mix(c000, c100, f.x), mix(c010, c110, f.x), f.y),
        mix(mix(c001, c101, f.x), mix(c011, c111, f.x), f.y),
        f.z,
    );
}

// Worley / cellular noise, wrapping at `tile`. Returns 1 - d² to the nearest
// feature point, so cloud-like blobs come out bright.
fn worley(x: vec3<f32>, tile: f32) -> f32 {
    let p = floor(x);
    let f = fract(x);

    var res = 100.0;
    for (var k = -1.0; k < 1.1; k += 1.0) {
        for (var j = -1.0; j < 1.1; j += 1.0) {
            for (var i = -1.0; i < 1.1; i += 1.0) {
                let b = vec3<f32>(i, j, k);
                let c = wrap3(p + b, tile);
                let r = b - f + hash13(c);
                res = min(res, dot(r, r));
            }
        }
    }
    return 1.0 - res;
}

fn worley_fbm(p: vec3<f32>, octaves: i32, first_freq: f32) -> f32 {
    var freq = first_freq;
    var amplitude = 1.0;
    var noise = 0.0;
    var weight = 0.0;

    for (var i = 0; i < octaves; i += 1) {
        noise += amplitude * worley(p * freq, freq);
        freq *= 2.0;
        weight += amplitude;
        amplitude *= 0.5;
    }
    return noise / weight;
}

fn perlin_fbm(p: vec3<f32>, octaves: i32, first_freq: f32) -> f32 {
    var freq = first_freq;
    var amplitude = 1.0;
    var noise = 0.0;
    var weight = 0.0;

    for (var i = 0; i < octaves; i += 1) {
        noise += amplitude * tiled_value_noise(p * freq, freq);
        freq *= 2.0;
        weight += amplitude;
        amplitude *= 0.5;
    }
    return noise / weight;
}

@compute @workgroup_size(8, 8, 1)
fn bake_base(@builtin(global_invocation_id) id: vec3<u32>) {
    let size = textureDimensions(base_texture);
    if id.x >= size.x || id.y >= size.y {
        return;
    }

    // A horizontal slice through the 3D field: the base map is sampled by world
    // XZ only, so it needs no third dimension of its own.
    let uv = (vec2<f32>(id.xy) + vec2<f32>(0.5)) / vec2<f32>(size);
    let coord = vec3<f32>(uv, 0.5);

    // Perlin-Worley: the Perlin FBM gives connected, wispy structure and the
    // Worley multiply carves the puffy cauliflower edges neither produces alone.
    let shape = mix(1.0, perlin_fbm(coord, 7, 4.0), 0.9)
        * mix(1.0, worley_fbm(coord, 8, 9.0), 0.7);

    // Remap window low end. The weights sum to 1, so this lands in [-1, 0] and
    // widens the window in some regions and narrows it in others.
    let window = 0.625 * worley_fbm(coord, 3, 15.0)
        + 0.250 * worley_fbm(coord, 3, 19.0)
        + 0.125 * worley_fbm(coord, 3, 23.0)
        - 1.0;

    // Height-gradient modifier, offset so it does not correlate with the shape.
    let height_mod = 1.0 - worley_fbm(coord + 0.5, 6, 9.0);

    textureStore(base_texture, id.xy, vec4<f32>(shape, window, height_mod, 1.0));
}

@compute @workgroup_size(4, 4, 4)
fn bake_detail(@builtin(global_invocation_id) id: vec3<u32>) {
    let size = textureDimensions(detail_texture);
    if id.x >= size.x || id.y >= size.y || id.z >= size.z {
        return;
    }

    let coord = (vec3<f32>(id) + vec3<f32>(0.5)) / vec3<f32>(size);

    // Three Worley bands, inverted so the dense cores read as *low* erosion —
    // subtracting this from the base shape eats the edges, not the middles.
    // Eight octaves, not the reference's sixteen: past the eighth each one
    // contributes under 1/256 of the sum and sits far below one texel of a 32³
    // volume, so it is pure grain with no shape in it.
    let r = worley_fbm(coord, 8, 3.0);
    let g = worley_fbm(coord, 4, 8.0);
    let b = worley_fbm(coord, 4, 16.0);
    let c = max(0.0, 1.0 - (r + g * 0.5 + b * 0.25) / 1.75);

    textureStore(detail_texture, id, vec4<f32>(c));
}
