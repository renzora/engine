// Coalesced matrix transpose, so the second FFT pass can run over rows again
// instead of striding down columns.
//
// Ported from GodotOceanWaves (MIT) `transpose.glsl`.
// Source: https://developer.nvidia.com/blog/efficient-matrix-transpose-cuda-cc/
//
// Deviation: 16x16 tiles instead of 32x32 — a 32x32 tile is 1024 invocations
// per workgroup, four times WebGPU's guaranteed maximum. The padded row
// (`TILE_SIZE + 1`) is what keeps the shared-memory access bank-conflict free.

const TILE_SIZE: u32 = 16u;
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
@group(0) @binding(1) var<storage, read_write> fft_data: array<vec2<f32>>;

var<workgroup> tile: array<array<vec2<f32>, 17u>, 16u>;

/// Reads the FFT's output half...
fn data_in(cascade: u32, layer: u32, x: u32, y: u32, map_size: u32) -> u32 {
    let per_cascade = map_size * map_size * NUM_SPECTRA * 2u;
    let out_offset = NUM_SPECTRA * map_size * map_size;
    return cascade * per_cascade + out_offset + layer * map_size * map_size + y * map_size + x;
}

/// ...and writes back to its input half, ready for the second FFT pass.
fn data_out(cascade: u32, layer: u32, x: u32, y: u32, map_size: u32) -> u32 {
    let per_cascade = map_size * map_size * NUM_SPECTRA * 2u;
    return cascade * per_cascade + layer * map_size * map_size + y * map_size + x;
}

@compute @workgroup_size(16, 16, 1)
fn main(
    @builtin(global_invocation_id) gid: vec3<u32>,
    @builtin(local_invocation_id) local_id: vec3<u32>,
    @builtin(workgroup_id) group_id: vec3<u32>,
) {
    let map_size = sim.map_size;
    let spectrum = group_id.z % NUM_SPECTRA;
    let cascade = group_id.z / NUM_SPECTRA;
    if gid.x >= map_size || gid.y >= map_size || cascade >= sim.num_cascades {
        return;
    }

    tile[local_id.y][local_id.x] = fft_data[data_in(cascade, spectrum, gid.x, gid.y, map_size)];
    workgroupBarrier();

    // Swap the block coordinates, keep the thread coordinates: that is what
    // makes both the read and the write contiguous.
    let out_x = group_id.y * TILE_SIZE + local_id.x;
    let out_y = group_id.x * TILE_SIZE + local_id.y;
    fft_data[data_out(cascade, spectrum, out_x, out_y, map_size)] =
        tile[local_id.x][local_id.y];
}
