// Generates the initial wave spectrum: a JONSWAP/TMA ocean spectrum with
// Hasselmann directional spreading, sampled with Gaussian-distributed random
// numbers. This is the time-*independent* sea state — it only needs rebuilding
// when the cascade parameters change.
//
// Ported from GodotOceanWaves (MIT) `spectrum_compute.glsl`.
// Sources: Jerry Tessendorf — Simulating Ocean Water
//          Christopher J. Horvath — Empirical Directional Wave Spectra for
//          Computer Graphics
//
// Deviations from the GLSL original, both forced by the portable WebGPU
// feature set (this engine exports to wasm):
//   * The cascade index arrives as the dispatch's z dimension instead of a push
//     constant, and all per-cascade parameters live in one uniform array.
//   * The spectrum lands in a storage *buffer* rather than a storage texture —
//     nothing ever samples it, and read-only storage textures are not portable.

const PI: f32 = 3.141592653589793;
const G: f32 = 9.81;

struct Cascade {
    @align(16) tile_length: vec2<f32>,
    alpha: f32,
    peak_frequency: f32,
    wind_speed: f32,
    angle: f32,
    swell: f32,
    detail: f32,
    spread: f32,
    time: f32,
    whitecap: f32,
    foam_grow_rate: f32,
    foam_decay_rate: f32,
    pad: f32,
    seed: vec2<i32>,
}

struct WaterSim {
    map_size: u32,
    num_cascades: u32,
    depth: f32,
    pad: f32,
    cascades: array<Cascade, 8>,
}

@group(0) @binding(0) var<uniform> sim: WaterSim;
// xy = h0(k), zw = conj(h0(-k)) — both halves are needed by the modulation
// stage, so they are packed together here.
@group(0) @binding(1) var<storage, read_write> spectrum: array<vec4<f32>>;

// Source: https://www.shadertoy.com/view/Xt3cDn
fn hash(x: vec2<u32>) -> vec2<f32> {
    var h32 = x.y + 374761393u + x.x * 3266489917u;
    h32 = 2246822519u * (h32 ^ (h32 >> 15u));
    h32 = 3266489917u * (h32 ^ (h32 >> 13u));
    let n = h32 ^ (h32 >> 16u);
    let rz = vec2<u32>(n, n * 48271u);
    return vec2<f32>((rz >> vec2<u32>(1u)) & vec2<u32>(0x7FFFFFFFu)) / f32(0x7FFFFFFF);
}

/// Box-Muller: uniform pair -> bivariate normal.
fn gaussian(x: vec2<f32>) -> vec2<f32> {
    // max() guards the log: a zero draw would produce infinity and poison one
    // texel of the spectrum for the lifetime of the sea state.
    let r = sqrt(-2.0 * log(max(x.x, 1e-9)));
    let theta = 2.0 * PI * x.y;
    return vec2<f32>(r * cos(theta), r * sin(theta));
}

fn conj_complex(x: vec2<f32>) -> vec2<f32> {
    return vec2<f32>(x.x, -x.y);
}

/// Dispersion relation and its derivative with respect to k.
fn dispersion_relation(k: f32, depth: f32) -> vec2<f32> {
    let a = k * depth;
    let b = tanh(a);
    let w = sqrt(G * k * b);
    let dw = 0.5 * G * (b + a * (1.0 - b * b)) / max(w, 1e-9);
    return vec2<f32>(w, dw);
}

/// Normalization factor approximation for the Longuet-Higgins function.
fn longuet_higgins_normalization(s: f32) -> f32 {
    let a = sqrt(s);
    if s < 0.4 {
        return (0.5 / PI) + s * (0.220636 + s * (-0.109 + s * 0.090));
    }
    return inverseSqrt(PI) * (a * 0.5 + (1.0 / a) * 0.0625);
}

fn longuet_higgins_function(s: f32, theta: f32) -> f32 {
    return longuet_higgins_normalization(s) * pow(abs(cos(theta * 0.5)), 2.0 * s);
}

fn hasselmann_directional_spread(
    w: f32,
    w_p: f32,
    wind_speed: f32,
    theta: f32,
    angle: f32,
    swell: f32,
) -> f32 {
    let p = w / w_p;
    var s: f32;
    if w <= w_p {
        s = 6.97 * pow(abs(p), 4.06);
    } else {
        s = 9.77 * pow(abs(p), -2.33 - 1.45 * (wind_speed * w_p / G - 1.17));
    }
    let s_xi = 16.0 * tanh(w_p / w) * swell * swell;
    return longuet_higgins_function(s + s_xi, theta - angle);
}

fn tma_spectrum(w: f32, w_p: f32, alpha: f32, depth: f32) -> f32 {
    let beta = 1.25;
    let gamma = 3.3; // Spectral peak shape constant

    var sigma = 0.09;
    if w <= w_p {
        sigma = 0.07;
    }
    let r = exp(-(w - w_p) * (w - w_p) / (2.0 * sigma * sigma * w_p * w_p));
    let jonswap = (alpha * G * G) / pow(w, 5.0) * exp(-beta * pow(w_p / w, 4.0)) * pow(gamma, r);

    // Kitaigorodskii depth attenuation — shallow water steals energy from the
    // long waves, which is what makes a lake read differently from open sea.
    let w_h = min(w * sqrt(depth / G), 2.0);
    var attenuation = 1.0 - 0.5 * (2.0 - w_h) * (2.0 - w_h);
    if w_h <= 1.0 {
        attenuation = 0.5 * w_h * w_h;
    }

    return jonswap * attenuation;
}

fn spectrum_amplitude(id: vec2<i32>, c: Cascade, map_size: u32) -> vec2<f32> {
    let dk = 2.0 * PI / max(c.tile_length, vec2<f32>(1e-3));
    let k_vec = (vec2<f32>(id) - f32(map_size) * 0.5) * dk;
    let k = length(k_vec) + 1e-6;
    let theta = atan2(k_vec.x, k_vec.y);

    let dispersion = dispersion_relation(k, sim.depth);
    let w = dispersion.x;
    let w_norm = dispersion.y / k * dk.x * dk.y;
    let s = tma_spectrum(w, c.peak_frequency, c.alpha, sim.depth);
    let directional = hasselmann_directional_spread(
        w, c.peak_frequency, c.wind_speed, theta, c.angle, c.swell,
    );
    let isotropic = 0.5 / PI;
    let d = mix(isotropic, directional, 1.0 - c.spread)
        * exp(-(1.0 - c.detail) * (1.0 - c.detail) * k * k);

    return gaussian(hash(bitcast<vec2<u32>>(id + c.seed))) * sqrt(max(2.0 * s * d * w_norm, 0.0));
}

@compute @workgroup_size(16, 16, 1)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let map_size = sim.map_size;
    let cascade_index = gid.z;
    if gid.x >= map_size || gid.y >= map_size || cascade_index >= sim.num_cascades {
        return;
    }

    let c = sim.cascades[cascade_index];
    let n = i32(map_size);
    let id0 = vec2<i32>(i32(gid.x), i32(gid.y));
    // The -k partner of this texel, wrapped into the grid.
    let id1 = ((-id0 % n) + n) % n;

    let index = cascade_index * map_size * map_size + gid.y * map_size + gid.x;
    spectrum[index] = vec4<f32>(
        spectrum_amplitude(id0, c, map_size),
        conj_complex(spectrum_amplitude(id1, c, map_size)),
    );
}
