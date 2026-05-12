// 2× box-filter downsample for the voxel radiance pyramid.
//
// For each destination voxel at (x, y, z) in mip N+1, read a single
// trilinear-filtered sample from mip N at the centre of the 2×2×2
// source block. The linear sampler does the 8-tap average in
// hardware — cheaper than eight `textureLoad` calls + manual blend.
//
// Cascade safety: each cascade has a power-of-two Z extent at every
// mip (64, 32, 16, 8 for VOXEL_RES = 64 with 4 mips). The 2× pair
// at any destination z lands at source z = 2k, 2k+1, both inside
// the same cascade's range. Linear filter taps stay within those
// bounds when we sample at the pair midpoint. ClampToEdge sampler
// addressing means edge fetches don't pull from outside the texture
// either.

@group(0) @binding(0) var src: texture_3d<f32>;
@group(0) @binding(1) var src_sampler: sampler;
@group(0) @binding(2) var dst: texture_storage_3d<rgba16float, write>;

@compute @workgroup_size(4, 4, 4)
fn downsample(@builtin(global_invocation_id) gid: vec3<u32>) {
    let dst_size = textureDimensions(dst);
    if (any(gid >= dst_size)) {
        return;
    }

    // UV-style normalized coords in source texture. The midpoint of
    // the 2×2×2 source block centred on this destination voxel is
    // exactly `(gid + 0.5) / dst_size` — at that point the linear
    // sampler gives the 8-tap average of the 2×2×2 source voxels.
    let src_size = vec3<f32>(textureDimensions(src));
    let uvw = (vec3<f32>(gid) + vec3<f32>(0.5)) / vec3<f32>(dst_size);

    let sampled = textureSampleLevel(src, src_sampler, uvw, 0.0);
    textureStore(dst, vec3<i32>(gid), sampled);
}
