// Unpacks the inverse-transformed spectra into the displacement and normal
// maps the water material samples, and accumulates foam.
//
// Foam is not painted on: it comes from the Jacobian of the horizontal
// displacement. Where the Jacobian goes negative the surface has folded over
// itself — a wave has broken — so foam is grown there and decays everywhere
// else. That is why whitecaps land on real breaking crests rather than on
// "anything above height X".
//
// Ported from GodotOceanWaves (MIT) `fft_unpack.glsl`.
//
// Deviations: 16x16x1 threads (the original's 16x16x2 exceeds WebGPU's 256
// invocation limit) with each thread writing both maps, and foam accumulation
// held in a storage buffer instead of read-modify-writing the normal map —
// read_write storage textures of this format are not portable.

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
@group(0) @binding(1) var<storage, read> fft_data: array<vec2<f32>>;
@group(0) @binding(2) var<storage, read_write> foam_accum: array<f32>;
@group(0) @binding(3) var displacement_map: texture_storage_2d_array<rgba16float, write>;
@group(0) @binding(4) var normal_map: texture_storage_2d_array<rgba16float, write>;

/// The FFT leaves its result in the output half of the scratch buffer; there is
/// no second transpose (rotating the wave field by PI/2 is invisible), so this
/// reads from that half directly.
fn fft_index(cascade: u32, layer: u32, x: u32, y: u32, map_size: u32) -> u32 {
    let per_cascade = map_size * map_size * NUM_SPECTRA * 2u;
    let out_offset = NUM_SPECTRA * map_size * map_size;
    return cascade * per_cascade + out_offset + layer * map_size * map_size + y * map_size + x;
}

@compute @workgroup_size(16, 16, 1)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let map_size = sim.map_size;
    let cascade = gid.z;
    if gid.x >= map_size || gid.y >= map_size || cascade >= sim.num_cascades {
        return;
    }

    let c = sim.cascades[cascade];
    // (-1)^(x+y) — an ifftshift folded into a sign flip, because the spectrum
    // was built centred on DC.
    var sign_shift = 1.0;
    if ((gid.x & 1u) ^ (gid.y & 1u)) == 1u {
        sign_shift = -1.0;
    }

    let layer0 = fft_data[fft_index(cascade, 0u, gid.x, gid.y, map_size)];
    let layer1 = fft_data[fft_index(cascade, 1u, gid.x, gid.y, map_size)];
    let layer2 = fft_data[fft_index(cascade, 2u, gid.x, gid.y, map_size)];
    let layer3 = fft_data[fft_index(cascade, 3u, gid.x, gid.y, map_size)];

    // --- Displacement ---
    let hx = layer0.x;
    let hy = layer0.y;
    let hz = layer1.x;
    let coords = vec2<i32>(gid.xy);
    textureStore(
        displacement_map,
        coords,
        i32(cascade),
        vec4<f32>(hx, hy, hz, 0.0) * sign_shift,
    );

    // --- Normals + foam ---
    let dhy_dx = layer1.y * sign_shift;
    let dhy_dz = layer2.x * sign_shift;
    let dhx_dx = layer2.y * sign_shift;
    let dhz_dz = layer3.x * sign_shift;
    let dhz_dx = layer3.y * sign_shift;

    let jacobian = (1.0 + dhx_dx) * (1.0 + dhz_dz) - dhz_dx * dhz_dx;
    let foam_factor = -min(0.0, jacobian - c.whitecap);

    let foam_index = cascade * map_size * map_size + gid.y * map_size + gid.x;
    var foam = foam_accum[foam_index];
    foam *= exp(-c.foam_decay_rate);
    foam += foam_factor * c.foam_grow_rate;
    foam = clamp(foam, 0.0, 1.0);
    foam_accum[foam_index] = foam;

    let gradient = vec2<f32>(dhy_dx, dhy_dz) / (1.0 + abs(vec2<f32>(dhx_dx, dhz_dz)));
    textureStore(
        normal_map,
        coords,
        i32(cascade),
        vec4<f32>(gradient, dhx_dx, foam),
    );
}
