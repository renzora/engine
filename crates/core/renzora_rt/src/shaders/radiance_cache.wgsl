// World-space radiance cache — hash-grid for off-screen GI persistence.
//
// Decay pass: each cell's life decrements. Dead cells are cleared for reuse.
// Write pass (folded into SSGI in future): screen-space radiance written
// into cache cells using exponential moving average.

#import renzora_rt::common::RtPushConstants

@group(0) @binding(14) var<storage, read_write> cache_checksums: array<u32, 524288u>;
@group(0) @binding(15) var<storage, read_write> cache_life: array<u32, 524288u>;
@group(0) @binding(16) var<storage, read_write> cache_radiance: array<vec4<f32>, 524288u>;
@group(0) @binding(17) var<storage, read_write> cache_normals: array<vec4<f32>, 524288u>;
@group(0) @binding(18) var<storage, read_write> cache_samples: array<u32, 524288u>;

var<push_constant> pc: RtPushConstants;

const CACHE_SIZE: u32 = 524288u; // 2^19

@compute @workgroup_size(256, 1, 1)
fn radiance_cache_update(@builtin(global_invocation_id) id: vec3<u32>) {
    let cell_idx = id.x;
    if cell_idx >= CACHE_SIZE {
        return;
    }

    // Full reset — clear everything
    if pc.reset != 0u {
        cache_checksums[cell_idx] = 0u;
        cache_life[cell_idx] = 0u;
        cache_radiance[cell_idx] = vec4<f32>(0.0);
        cache_normals[cell_idx] = vec4<f32>(0.0);
        cache_samples[cell_idx] = 0u;
        return;
    }

    // Decay life counter
    let life = cache_life[cell_idx];
    if life > 0u {
        cache_life[cell_idx] = life - 1u;
    } else {
        // Dead cell — clear for reuse
        cache_checksums[cell_idx] = 0u;
        cache_radiance[cell_idx] = vec4<f32>(0.0);
        cache_normals[cell_idx] = vec4<f32>(0.0);
        cache_samples[cell_idx] = 0u;
    }
}
