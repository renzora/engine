import constants::WORLD_CACHE_SIZE;
import bevy_pbr::render::utils::{rand_f, rand_vec2f};
import bevy_render::maths::orthonormalize;
import package::realtime::bindings::{
    world_cache,
    constants,
};

enable wgpu_ray_query;

/// Maximum amount of frames a cell can live for without being queried
const WORLD_CACHE_CELL_LIFETIME: u32 = 10u;
/// Maximum amount of attempts to find a cache entry after a hash collision
///
/// RENZORA PATCH: 8, was 3. Linear probing in a table that is mostly full
/// needs far more than three steps to find a free slot, so in a large enclosed
/// scene (the Bistro, about 90% full) most new cells were never made and the
/// surfaces needing them went dark, crawling as the camera moved. A lookup
/// that finds its cell, or an empty slot, at the first step costs the same as
/// before; only the crowded case probes further.
const WORLD_CACHE_MAX_SEARCH_STEPS: u32 = 8u;

/// Marker value for an empty cell
const WORLD_CACHE_EMPTY_CELL: u32 = 0u;

@if(!WORLD_CACHE_NON_ATOMIC_LIFE_BUFFER)
fn query_world_cache(world_position_in: vec3<f32>, world_normal: vec3<f32>, view_position: vec3<f32>, ray_t: f32, cell_lifetime: u32, rng: ptr<function, u32>) -> vec3<f32> {
    return query_world_cache_cell(world_position_in, world_normal, view_position, ray_t, cell_lifetime, true, rng).rgb;
}

// RENZORA PATCH: the cell itself, light in rgb and how many samples it has
// averaged in a (zero for a cell created by this lookup). The see-through pass
// (transparent.wesl) reads the cache with no denoiser after it, and weighs each
// lookup by its samples: a cell a moving camera has just created holds one
// random bounce ray, and read raw those showed as splotches crawling over a
// car's clear coat.
//
// With `create` false a lookup only reads: it neither creates a missing cell
// nor extends a found one's life. The see-through pass makes several lookups
// at every point it lights, and doing both for each filled the cache to 95% of
// its capacity in the Bistro, where lookups then collided and failed and each
// cell was refreshed only one frame in twelve: the opaque image's bounce light
// went blotchy everywhere, the pavement included. The cache's own update pass
// reads this way too once the cache is crowded (world_cache_update.wesl).
@if(!WORLD_CACHE_NON_ATOMIC_LIFE_BUFFER)
fn query_world_cache_cell(world_position_in: vec3<f32>, world_normal: vec3<f32>, view_position: vec3<f32>, ray_t: f32, cell_lifetime: u32, create: bool, rng: ptr<function, u32>) -> vec4<f32> {
    var world_position = world_position_in;
    var cell_size = get_cell_size(world_position, view_position, ray_t, rng);

@if(!NO_JITTER_WORLD_CACHE) {
    // Jitter query point, which essentially blurs the cache a bit so it's not so grid-like
    // https://tomclabault.github.io/blog/2025/regir, jitter_world_position_tangent_plane
    let TBN = orthonormalize(world_normal);
    let offset = (rand_vec2f(rng) * 2.0 - 1.0) * cell_size * 0.5;
    world_position += offset.x * TBN[0] + offset.y * TBN[1];
    cell_size = get_cell_size(world_position, view_position, ray_t, rng);
}

    let world_position_quantized = bitcast<vec3<u32>>(quantize_position(world_position, cell_size));
    let world_normal_quantized = bitcast<vec3<u32>>(quantize_normal(world_normal));
    var key = compute_key(world_position_quantized, world_normal_quantized);
    let checksum = compute_checksum(world_position_quantized, world_normal_quantized);

    for (var i = 0u; i < WORLD_CACHE_MAX_SEARCH_STEPS; i++) {
        if !create {
            let existing_checksum = atomicLoad(&world_cache.checksums[key]);
            if existing_checksum == checksum {
                return world_cache.radiance[key];
            }
            if existing_checksum == WORLD_CACHE_EMPTY_CELL {
                return vec4(0.0);
            }
            key += 1u;
            continue;
        }
        let cas = atomicCompareExchangeWeak(&world_cache.checksums[key], WORLD_CACHE_EMPTY_CELL, checksum);
        let existing_checksum = cas.old_value;

        // Cell already exists or is empty - reset lifetime
        if existing_checksum == checksum || existing_checksum == WORLD_CACHE_EMPTY_CELL {
            @if(!WORLD_CACHE_QUERY_ATOMIC_MAX_LIFETIME)
            atomicStore(&world_cache.life[key], cell_lifetime);
            @else
            atomicMax(&world_cache.life[key], cell_lifetime);
        }

        if existing_checksum == checksum {
            // Cache entry already exists - get radiance
            return world_cache.radiance[key];
        } else if existing_checksum == WORLD_CACHE_EMPTY_CELL && cas.exchanged {
            // Cell is empty - initialize it
            world_cache.geometry_data[key].world_position = world_position;
            world_cache.geometry_data[key].world_normal = world_normal;
            return vec4(0.0);
        } else {
            // Collision - linear probe to next entry
            key += 1u;
        }
    }

    return vec4(0.0);
}

fn get_cell_size(world_position: vec3<f32>, view_position: vec3<f32>, ray_t: f32, rng: ptr<function, u32>) -> f32 {
    let camera_distance = distance(view_position, world_position) / constants.world_cache_position_lod_scale;
    let lod_f = log2(1.0 + camera_distance);
    let lod_fract = fract(lod_f);
    let lod = floor(lod_f) + select(0.0, 1.0, rand_f(rng) < lod_fract * lod_fract * lod_fract);
    return constants.world_cache_position_base_cell_size * exp2(lod);
}

fn quantize_position(world_position: vec3<f32>, quantization_factor: f32) -> vec3<f32> {
    return floor(world_position / quantization_factor + 0.0001);
}

fn quantize_normal(world_normal: vec3<f32>) -> vec3<f32> {
    return floor(world_normal + 0.0001);
}

fn compute_key(world_position: vec3<u32>, world_normal: vec3<u32>) -> u32 {
    var key = pcg_hash(world_position.x);
    key = pcg_hash(key + world_position.y);
    key = pcg_hash(key + world_position.z);
    key = pcg_hash(key + world_normal.x);
    key = pcg_hash(key + world_normal.y);
    key = pcg_hash(key + world_normal.z);
    return wrap_key(key);
}

fn compute_checksum(world_position: vec3<u32>, world_normal: vec3<u32>) -> u32 {
    var key = iqint_hash(world_position.x);
    key = iqint_hash(key + world_position.y);
    key = iqint_hash(key + world_position.z);
    key = iqint_hash(key + world_normal.x);
    key = iqint_hash(key + world_normal.y);
    key = iqint_hash(key + world_normal.z);
    return max(key, 1u); // 0u is reserved for WORLD_CACHE_EMPTY_CELL
}

fn pcg_hash(input: u32) -> u32 {
    let state = input * 747796405u + 2891336453u;
    let word = ((state >> ((state >> 28u) + 4u)) ^ state) * 277803737u;
    return (word >> 22u) ^ word;
}

fn iqint_hash(input: u32) -> u32 {
    let n = (input << 13u) ^ input;
    return n * (n * n * 15731u + 789221u) + 1376312589u;
}

fn wrap_key(key: u32) -> u32 {
    return key & (u32(WORLD_CACHE_SIZE) - 1u);
}
