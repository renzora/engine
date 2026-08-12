// Propagates the wave spectrum to the current time and derives the five
// gradient fields the shading needs. Every output is real-valued after the
// inverse transform, so they are packed two-per-complex-signal — the FFT then
// carries two waves for the price of one.
//
// Ported from GodotOceanWaves (MIT) `spectrum_modulate.glsl`.
// Sources: Jerry Tessendorf — Simulating Ocean Water
//          Robert Matusiak — Implementing FFT Algorithms of Real-Valued
//          Sequences with the TMS320 DSP Platform

const PI: f32 = 3.141592653589793;
const G: f32 = 9.81;
const NUM_SPECTRA: u32 = 4u;

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
@group(0) @binding(1) var<storage, read> spectrum: array<vec4<f32>>;
@group(0) @binding(2) var<storage, read_write> fft_data: array<vec2<f32>>;

fn exp_complex(x: f32) -> vec2<f32> {
    return vec2<f32>(cos(x), sin(x));
}

fn mul_complex(a: vec2<f32>, b: vec2<f32>) -> vec2<f32> {
    return vec2<f32>(a.x * b.x - a.y * b.y, a.x * b.y + a.y * b.x);
}

fn conj_complex(x: vec2<f32>) -> vec2<f32> {
    return vec2<f32>(x.x, -x.y);
}

fn dispersion_relation(k: f32, depth: f32) -> f32 {
    return sqrt(G * k * tanh(k * depth));
}

/// Index into the FFT scratch buffer's *input* half.
fn fft_index(cascade: u32, layer: u32, x: u32, y: u32, map_size: u32) -> u32 {
    let per_cascade = map_size * map_size * NUM_SPECTRA * 2u;
    return cascade * per_cascade + layer * map_size * map_size + y * map_size + x;
}

@compute @workgroup_size(16, 16, 1)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let map_size = sim.map_size;
    let cascade_index = gid.z;
    if gid.x >= map_size || gid.y >= map_size || cascade_index >= sim.num_cascades {
        return;
    }

    let c = sim.cascades[cascade_index];
    let dk = 2.0 * PI / max(c.tile_length, vec2<f32>(1e-3));
    let k_vec = (vec2<f32>(gid.xy) - f32(map_size) * 0.5) * dk;
    let k = length(k_vec) + 1e-6;
    let k_unit = k_vec / k;

    // --- Spectrum modulation ---
    let h0 = spectrum[cascade_index * map_size * map_size + gid.y * map_size + gid.x];
    let modulation = exp_complex(dispersion_relation(k, sim.depth) * c.time);
    // Adding the -k conjugate partner is what keeps h Hermitian, so the
    // inverse transform comes out purely real.
    let h = mul_complex(h0.xy, modulation) + mul_complex(h0.zw, conj_complex(modulation));
    // Multiplying by i, precomputed: every term below is h times an imaginary
    // scalar, so this saves five complex multiplies.
    let h_inv = vec2<f32>(-h.y, h.x);

    // --- Displacement ---
    let hx = h_inv * k_unit.y;
    let hy = h;
    let hz = h_inv * k_unit.x;

    // --- Gradients ---
    let dhy_dx = h_inv * k_vec.y;
    let dhy_dz = h_inv * k_vec.x;
    let dhx_dx = -h * k_vec.y * k_unit.y;
    let dhz_dz = -h * k_vec.x * k_unit.x;
    let dhz_dx = -h * k_vec.y * k_unit.x;

    // Two real signals per complex transform: a + i*b.
    fft_data[fft_index(cascade_index, 0u, gid.x, gid.y, map_size)] =
        vec2<f32>(hx.x - hy.y, hx.y + hy.x);
    fft_data[fft_index(cascade_index, 1u, gid.x, gid.y, map_size)] =
        vec2<f32>(hz.x - dhy_dx.y, hz.y + dhy_dx.x);
    fft_data[fft_index(cascade_index, 2u, gid.x, gid.y, map_size)] =
        vec2<f32>(dhy_dz.x - dhx_dx.y, dhy_dz.y + dhx_dx.x);
    fft_data[fft_index(cascade_index, 3u, gid.x, gid.y, map_size)] =
        vec2<f32>(dhz_dz.x - dhz_dx.y, dhz_dz.y + dhz_dx.x);
}
