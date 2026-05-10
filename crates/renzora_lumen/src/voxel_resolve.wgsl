// Resolve the per-frame accumulation buffer into the persistent voxel
// radiance texture. For each voxel:
//   - If it received any contributions this frame, average them and
//     blend with the existing texture value at TEMPORAL_ALPHA.
//   - If it received nothing (camera moved away, voxel is occluded),
//     decay the existing value toward black so stale data fades.

@group(0) @binding(0) var<storage, read_write> accum: array<atomic<u32>>;
@group(0) @binding(1) var voxels: texture_storage_3d<rgba16float, read_write>;

const FIXED_POINT_SCALE: f32 = 256.0;
// Per-frame mix weight for new contributions. 0.25 = noticeable
// reaction to scene changes, ~5 frames to settle.
const TEMPORAL_ALPHA: f32 = 0.25;
// Per-frame multiplier applied to voxels that didn't receive a
// contribution. Lets stale data fade out as the camera moves.
const STALE_DECAY: f32 = 0.97;

@compute @workgroup_size(4, 4, 4)
fn resolve(@builtin(global_invocation_id) gid: vec3<u32>) {
    let dims = textureDimensions(voxels);
    if (gid.x >= dims.x || gid.y >= dims.y || gid.z >= dims.z) { return; }

    let res = dims.x;
    let voxel_idx = gid.x + gid.y * res + gid.z * res * res;
    let base = voxel_idx * 4u;

    let count = atomicLoad(&accum[base + 3u]);
    let prev = textureLoad(voxels, vec3<i32>(gid));

    if (count == 0u) {
        // No contribution — decay history toward zero.
        let decayed = prev * STALE_DECAY;
        textureStore(voxels, vec3<i32>(gid), decayed);
        return;
    }

    let sum_r = f32(atomicLoad(&accum[base]));
    let sum_g = f32(atomicLoad(&accum[base + 1u]));
    let sum_b = f32(atomicLoad(&accum[base + 2u]));
    let inv = 1.0 / (f32(count) * FIXED_POINT_SCALE);
    let avg = vec3<f32>(sum_r, sum_g, sum_b) * inv;

    // Temporal blend with the previous voxel value. Smooths out the
    // small frame-to-frame changes from camera micro-motion / aliasing
    // without losing responsiveness when the scene actually changes.
    let blended = mix(prev.rgb, avg, TEMPORAL_ALPHA);
    textureStore(voxels, vec3<i32>(gid), vec4<f32>(blended, 1.0));
}
