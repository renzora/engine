// Resolve the per-frame accumulation buffer into the persistent
// clipmap radiance texture. The texture stores all cascades stacked
// along Z: cascade C occupies Z slice [C * RES, (C+1) * RES).
//
// Per-voxel logic (same for every cascade):
//   .rgb = sum / total_count, temporally blended with prev
//   .a   = occupancy
//          1.0  = direct geometry hit this frame
//          0.7  = dilated from 2+ occupied axis-aligned neighbors
//          else = prev × OCCUPANCY_DECAY

@group(0) @binding(0) var<storage, read_write> accum: array<atomic<u32>>;
@group(0) @binding(1) var voxels: texture_storage_3d<rgba16float, read_write>;

const FIXED_POINT_SCALE: f32 = 256.0;
const TEMPORAL_ALPHA: f32 = 0.25;
const OCCUPANCY_DECAY: f32 = 0.97;
const DILATED_OCCUPANCY: f32 = 0.7;
const DILATION_MIN_NEIGHBORS: u32 = 2u;
// Dilation reads 24 atomic values per empty voxel — ~10M ops/frame for
// a 2-cascade cache. We can pay that to fill small holes left by
// stochastic sampling, but Phase 5's cone marcher should absorb the
// sparseness for cheaper. Off by default; flip to enable.
const ENABLE_DILATION: bool = false;

fn voxel_buffer_base(cascade: u32, local_idx: u32, voxels_per_cascade: u32) -> u32 {
    return (cascade * voxels_per_cascade + local_idx) * 5u;
}

fn local_index(local: vec3<u32>, res: u32) -> u32 {
    return local.x + local.y * res + local.z * res * res;
}

fn neighbor_has_geom(local: vec3<i32>, cascade: u32, res: u32, voxels_per_cascade: u32) -> bool {
    let r = i32(res);
    if (local.x < 0 || local.x >= r || local.y < 0 || local.y >= r || local.z < 0 || local.z >= r) {
        return false;
    }
    let base = voxel_buffer_base(cascade, local_index(vec3<u32>(local), res), voxels_per_cascade);
    return atomicLoad(&accum[base + 4u]) > 0u;
}

fn neighbor_average(local: vec3<i32>, cascade: u32, res: u32, voxels_per_cascade: u32) -> vec4<f32> {
    let r = i32(res);
    if (local.x < 0 || local.x >= r || local.y < 0 || local.y >= r || local.z < 0 || local.z >= r) {
        return vec4<f32>(0.0);
    }
    let base = voxel_buffer_base(cascade, local_index(vec3<u32>(local), res), voxels_per_cascade);
    let count = atomicLoad(&accum[base + 3u]);
    if (count == 0u) { return vec4<f32>(0.0); }
    let sum_r = f32(atomicLoad(&accum[base]));
    let sum_g = f32(atomicLoad(&accum[base + 1u]));
    let sum_b = f32(atomicLoad(&accum[base + 2u]));
    let inv = 1.0 / (f32(count) * FIXED_POINT_SCALE);
    return vec4<f32>(vec3<f32>(sum_r, sum_g, sum_b) * inv, f32(count));
}

@compute @workgroup_size(4, 4, 4)
fn resolve(@builtin(global_invocation_id) gid: vec3<u32>) {
    let dims = textureDimensions(voxels);
    if (gid.x >= dims.x || gid.y >= dims.y || gid.z >= dims.z) { return; }

    let res = dims.x; // square in xy, Z is res * cascade_count
    // Decode (cascade, local_z) from the stacked Z dimension.
    let cascade = gid.z / res;
    let local = vec3<u32>(gid.x, gid.y, gid.z % res);
    let voxels_per_cascade = res * res * res;
    let base = voxel_buffer_base(cascade, local_index(local, res), voxels_per_cascade);

    let total_count = atomicLoad(&accum[base + 3u]);
    let geom_count = atomicLoad(&accum[base + 4u]);
    let prev = textureLoad(voxels, vec3<i32>(gid));

    var rgb = prev.rgb;
    if (total_count > 0u) {
        let sum_r = f32(atomicLoad(&accum[base]));
        let sum_g = f32(atomicLoad(&accum[base + 1u]));
        let sum_b = f32(atomicLoad(&accum[base + 2u]));
        let inv = 1.0 / (f32(total_count) * FIXED_POINT_SCALE);
        let avg = vec3<f32>(sum_r, sum_g, sum_b) * inv;
        rgb = mix(prev.rgb, avg, TEMPORAL_ALPHA);
    }

    var occupancy = prev.a * OCCUPANCY_DECAY;

    if (geom_count > 0u) {
        occupancy = 1.0;
    } else if (ENABLE_DILATION) {
        let i = vec3<i32>(local);
        var n_occupied = 0u;
        var n_color = vec3<f32>(0.0);
        var n_weight = 0.0;

        let dirs = array<vec3<i32>, 6>(
            vec3<i32>(1, 0, 0), vec3<i32>(-1, 0, 0),
            vec3<i32>(0, 1, 0), vec3<i32>(0, -1, 0),
            vec3<i32>(0, 0, 1), vec3<i32>(0, 0, -1),
        );

        for (var k: u32 = 0u; k < 6u; k = k + 1u) {
            let d = dirs[k];
            let n_idx = i + d;
            if (neighbor_has_geom(n_idx, cascade, res, voxels_per_cascade)) {
                n_occupied = n_occupied + 1u;
                let n_avg = neighbor_average(n_idx, cascade, res, voxels_per_cascade);
                if (n_avg.w > 0.0) {
                    n_color = n_color + n_avg.rgb;
                    n_weight = n_weight + 1.0;
                }
            }
        }

        if (n_occupied >= DILATION_MIN_NEIGHBORS) {
            occupancy = max(occupancy, DILATED_OCCUPANCY);
            if (n_weight > 0.0 && total_count == 0u) {
                rgb = n_color / n_weight;
            }
        }
    }

    textureStore(voxels, vec3<i32>(gid), vec4<f32>(rgb, occupancy));
}
