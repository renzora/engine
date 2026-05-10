// Zero the per-frame accumulation buffer. Layout: 4 u32s per voxel —
// fixed-point R, G, B (×256 each), plus a contributor count. The
// resolve pass divides by count to recover the true frame-averaged
// color, eliminating the per-pixel-per-voxel race entirely.

@group(0) @binding(0) var<storage, read_write> accum: array<atomic<u32>>;

@compute @workgroup_size(64, 1, 1)
fn clear(@builtin(global_invocation_id) gid: vec3<u32>) {
    let n = arrayLength(&accum);
    if (gid.x >= n) { return; }
    atomicStore(&accum[gid.x], 0u);
}
