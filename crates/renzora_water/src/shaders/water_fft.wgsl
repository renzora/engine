// A coalesced decimation-in-time Stockham FFT kernel: one workgroup transforms
// one row of one spectrum of one cascade, ping-ponging through workgroup
// memory.
//
// Ported from GodotOceanWaves (MIT) `fft_compute.glsl`.
// Source: http://wwwa.pikara.ne.jp/okojisan/otfft-en/stockham3.html
//
// Deviation from the original: the GLSL runs one thread per column, i.e. up to
// 1024 threads per workgroup. WebGPU guarantees only 256, so each thread here
// walks a stride of `FFT_THREADS` columns instead. `MAP_SIZE` and `FFT_THREADS`
// are shader defs, so every simulation resolution gets its own pipeline rather
// than paying for the largest one's workgroup memory.

const MAP_SIZE: u32 = #{MAP_SIZE}u;
const FFT_THREADS: u32 = #{FFT_THREADS}u;
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
@group(0) @binding(1) var<storage, read> butterfly: array<vec4<f32>>;
@group(0) @binding(2) var<storage, read_write> fft_data: array<vec2<f32>>;

/// Ping-pong buffer for one row.
var<workgroup> row_shared: array<vec2<f32>, 2u * MAP_SIZE>;

fn mul_complex(a: vec2<f32>, b: vec2<f32>) -> vec2<f32> {
    return vec2<f32>(a.x * b.x - a.y * b.y, a.x * b.y + a.y * b.x);
}

/// Input half of the scratch buffer (written by the modulation/transpose pass).
fn data_in(cascade: u32, layer: u32, x: u32, y: u32) -> u32 {
    let per_cascade = MAP_SIZE * MAP_SIZE * NUM_SPECTRA * 2u;
    return cascade * per_cascade + layer * MAP_SIZE * MAP_SIZE + y * MAP_SIZE + x;
}

/// Output half — the transpose reads from here, and so does the unpack.
fn data_out(cascade: u32, layer: u32, x: u32, y: u32) -> u32 {
    let per_cascade = MAP_SIZE * MAP_SIZE * NUM_SPECTRA * 2u;
    let out_offset = NUM_SPECTRA * MAP_SIZE * MAP_SIZE;
    return cascade * per_cascade + out_offset + layer * MAP_SIZE * MAP_SIZE + y * MAP_SIZE + x;
}

@compute @workgroup_size(#{FFT_THREADS}, 1, 1)
fn main(
    @builtin(local_invocation_id) local_id: vec3<u32>,
    @builtin(workgroup_id) group_id: vec3<u32>,
) {
    let num_stages = firstLeadingBit(MAP_SIZE); // log2(MAP_SIZE)
    let row = group_id.y;
    // One dispatch covers every spectrum of every cascade; z packs both.
    let spectrum = group_id.z % NUM_SPECTRA;
    let cascade = group_id.z / NUM_SPECTRA;

    for (var col = local_id.x; col < MAP_SIZE; col += FFT_THREADS) {
        row_shared[col] = fft_data[data_in(cascade, spectrum, col, row)];
    }

    for (var stage = 0u; stage < num_stages; stage += 1u) {
        workgroupBarrier();
        let read_buf = (stage % 2u) * MAP_SIZE;
        let write_buf = ((stage + 1u) % 2u) * MAP_SIZE;

        for (var col = local_id.x; col < MAP_SIZE; col += FFT_THREADS) {
            let factors = butterfly[stage * MAP_SIZE + col];
            let read_indices = vec2<u32>(
                bitcast<u32>(factors.x),
                bitcast<u32>(factors.y),
            );
            let upper = row_shared[read_buf + read_indices.x];
            let lower = row_shared[read_buf + read_indices.y];
            row_shared[write_buf + col] = upper + mul_complex(lower, factors.zw);
        }
    }

    // Without this the last stage's writes from other threads may not be
    // visible when this thread reads its columns back out.
    workgroupBarrier();
    let final_buf = (num_stages % 2u) * MAP_SIZE;
    for (var col = local_id.x; col < MAP_SIZE; col += FFT_THREADS) {
        fft_data[data_out(cascade, spectrum, col, row)] = row_shared[final_buf + col];
    }
}
