// Resolve the per-frame accumulation buffer into the persistent voxel
// radiance texture, with in-pass neighbor dilation to fill small
// holes in the geometry voxelization.
//
// Accum layout per voxel: [sum_r, sum_g, sum_b, total_count, geom_count]
//
// Radiance texture: rgba16float
//   .rgb = sum_rgb / total_count, temporally blended with prev frame
//          (or averaged neighbor color when the voxel is dilated-only)
//   .a   = occupancy
//          1.0  = direct geometry hit this frame
//          0.7  = dilated from 2+ occupied axis-aligned neighbors
//          (else) = previous-frame value × OCCUPANCY_DECAY
//
// Phase 5's ray tracer uses `.a` as the "is this voxel solid?" signal.
// The 0.7 dilated tier lets it distinguish exact-hit voxels from
// extrapolated ones if it wants to (e.g., trust full hits more).

@group(0) @binding(0) var<storage, read_write> accum: array<atomic<u32>>;
@group(0) @binding(1) var voxels: texture_storage_3d<rgba16float, read_write>;

const FIXED_POINT_SCALE: f32 = 256.0;
const TEMPORAL_ALPHA: f32 = 0.25;
const OCCUPANCY_DECAY: f32 = 0.97;
const DILATED_OCCUPANCY: f32 = 0.7;
const DILATION_MIN_NEIGHBORS: u32 = 2u;

fn voxel_base(idx: vec3<u32>, res: u32) -> u32 {
    return (idx.x + idx.y * res + idx.z * res * res) * 5u;
}

fn read_neighbor_average(idx: vec3<i32>, res: u32) -> vec4<f32> {
    // Returns (avg_rgb, total_count_as_f32). Returns count=0 if the
    // neighbor is outside the grid or has no contributions.
    let r = i32(res);
    if (idx.x < 0 || idx.x >= r || idx.y < 0 || idx.y >= r || idx.z < 0 || idx.z >= r) {
        return vec4<f32>(0.0);
    }
    let base = voxel_base(vec3<u32>(idx), res);
    let count = atomicLoad(&accum[base + 3u]);
    if (count == 0u) { return vec4<f32>(0.0); }
    let sum_r = f32(atomicLoad(&accum[base]));
    let sum_g = f32(atomicLoad(&accum[base + 1u]));
    let sum_b = f32(atomicLoad(&accum[base + 2u]));
    let inv = 1.0 / (f32(count) * FIXED_POINT_SCALE);
    return vec4<f32>(vec3<f32>(sum_r, sum_g, sum_b) * inv, f32(count));
}

fn neighbor_has_geom(idx: vec3<i32>, res: u32) -> bool {
    let r = i32(res);
    if (idx.x < 0 || idx.x >= r || idx.y < 0 || idx.y >= r || idx.z < 0 || idx.z >= r) {
        return false;
    }
    let base = voxel_base(vec3<u32>(idx), res);
    return atomicLoad(&accum[base + 4u]) > 0u;
}

@compute @workgroup_size(4, 4, 4)
fn resolve(@builtin(global_invocation_id) gid: vec3<u32>) {
    let dims = textureDimensions(voxels);
    if (gid.x >= dims.x || gid.y >= dims.y || gid.z >= dims.z) { return; }

    let res = dims.x;
    let base = voxel_base(gid, res);
    let total_count = atomicLoad(&accum[base + 3u]);
    let geom_count = atomicLoad(&accum[base + 4u]);
    let prev = textureLoad(voxels, vec3<i32>(gid));

    // Direct radiance contribution this frame (or hold previous).
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
    } else {
        // Dilation: if enough axis-aligned neighbors had a direct geom
        // hit, mark this voxel as "dilated occupied" with the averaged
        // neighbor color. Fills gaps in sparse triangle sampling so
        // the ray tracer doesn't poke through holes in voxelized walls.
        let i = vec3<i32>(gid);
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
            if (neighbor_has_geom(n_idx, res)) {
                n_occupied = n_occupied + 1u;
                let n_avg = read_neighbor_average(n_idx, res);
                if (n_avg.w > 0.0) {
                    n_color = n_color + n_avg.rgb;
                    n_weight = n_weight + 1.0;
                }
            }
        }

        if (n_occupied >= DILATION_MIN_NEIGHBORS) {
            occupancy = max(occupancy, DILATED_OCCUPANCY);
            if (n_weight > 0.0) {
                let dilated_rgb = n_color / n_weight;
                // If we'd otherwise be holding a stale color, prefer
                // the freshly-averaged neighbor color.
                if (total_count == 0u) {
                    rgb = dilated_rgb;
                }
            }
        }
    }

    textureStore(voxels, vec3<i32>(gid), vec4<f32>(rgb, occupancy));
}
