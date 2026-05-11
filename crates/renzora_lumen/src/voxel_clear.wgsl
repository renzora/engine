// Zero the per-frame accumulation buffer. Layout: 4 u32s per voxel —
// fixed-point R, G, B (×256 each), plus a contributor count. The
// resolve pass divides by count to recover the true frame-averaged
// color, eliminating the per-pixel-per-voxel race entirely.

@group(0) @binding(0) var<storage, read_write> accum: array<atomic<u32>>;

// Workgroup size bumped from 64 to 256 to keep the 1D dispatch count
// under wgpu's per-dimension limit (65535) at higher cascade counts.
// At 4 cascades: 64³ × 4 × 5 u32 = 5,242,880 entries; /256 = 20,480
// workgroups, well under the limit. Going wider (1024) isn't reliable
// across all GPUs we want to ship to.
@compute @workgroup_size(256, 1, 1)
fn clear(@builtin(global_invocation_id) gid: vec3<u32>) {
    let n = arrayLength(&accum);
    if (gid.x >= n) { return; }
    atomicStore(&accum[gid.x], 0u);
}
