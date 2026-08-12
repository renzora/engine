// Precomputes the butterfly factors for the Stockham FFT kernel: for every
// stage and column, which two entries to combine and with what twiddle factor.
// Only depends on `map_size`, so this runs once per simulation rebuild.
//
// Ported from GodotOceanWaves (MIT) `fft_butterfly.glsl`.

const PI: f32 = 3.141592653589793;

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
// xy = the two source indices, bit-cast to f32; zw = the twiddle factor.
@group(0) @binding(1) var<storage, read_write> butterfly: array<vec4<f32>>;

/// exp(j*x). The positive exponent makes this an *inverse* transform.
fn exp_complex(x: f32) -> vec2<f32> {
    return vec2<f32>(cos(x), sin(x));
}

@compute @workgroup_size(64, 1, 1)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let map_size = sim.map_size;
    let col = gid.x;
    let stage = gid.y;
    if col >= map_size / 2u {
        return;
    }

    let stride = 1u << stage;
    let mid = map_size >> (stage + 1u);
    let i = col >> stage;
    let j = col % stride;

    let twiddle = exp_complex(PI / f32(stride) * f32(j));
    let r0 = stride * i + j;
    let r1 = stride * (i + mid) + j;
    let w0 = stride * (2u * i) + j;
    let w1 = stride * (2u * i + 1u) + j;

    let read_indices = vec2<f32>(bitcast<f32>(r0), bitcast<f32>(r1));
    butterfly[stage * map_size + w0] = vec4<f32>(read_indices, twiddle);
    butterfly[stage * map_size + w1] = vec4<f32>(read_indices, -twiddle);
}
