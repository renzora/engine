//! Pick the clusters to draw for one view.
//!
//! One thread per (instance, cluster) pair. A cluster survives when it is
//! part of the LOD cut at this distance (its own error projects under the
//! threshold and its group's simplified error does not) and its sphere is
//! inside the frustum. Survivors are appended to the visible list: single-
//! sided ones from the front, double-sided ones from the back, so one buffer
//! feeds two raster draws with different cull modes. Alpha-masked and
//! transparent ones go to buckets past both, one indirect draw each.

import super::types::{VgInstance, VgCluster, VgCullParams, VG_DOUBLE_SIDED, VG_MASKED, VG_TRANSPARENT, VG_SKINNED, VG_NO_GROUP};
import bevy_render::view::View;
import bevy_render::maths::affine3_to_square;

@group(0) @binding(0) var<uniform> view: View;
@group(0) @binding(1) var<uniform> params: VgCullParams;
@group(0) @binding(2) var<storage, read> instances: array<VgInstance>;
@group(0) @binding(3) var<storage, read> clusters: array<VgCluster>;
@group(0) @binding(4) var<storage, read_write> visible: array<vec2<u32>>;
/// Two indirect draws: [vertex_count, instance_count, first_vertex,
/// first_instance] for single-sided, then the same for double-sided.
@group(0) @binding(5) var<storage, read_write> draws: array<atomic<u32>, 8>;
/// Per masked bucket: where its slots start, relative to `masked_offset`.
@group(0) @binding(6) var<storage, read> masked_bases: array<u32>;
/// Per masked bucket, an indirect draw like the two above.
@group(0) @binding(7) var<storage, read_write> masked_draws: array<atomic<u32>>;
/// One bit per group: its page is on the GPU.
@group(0) @binding(8) var<storage, read> resident: array<u32>;
/// One bit per group, set here and read back by the CPU (`streaming.rs`):
/// wanted but missing, and drawn this frame.
@group(0) @binding(9) var<storage, read_write> requests: array<atomic<u32>>;
@group(0) @binding(10) var<storage, read_write> drawn: array<atomic<u32>>;

fn is_resident(group: u32) -> bool {
    return (resident[group >> 5u] & (1u << (group & 31u))) != 0u;
}

/// The error of a sphere-bounded patch of geometry, projected to pixels, the
/// way meshoptimizer's cluster LOD builder defines it: rotation-invariant and
/// measured from the sphere's nearest point, so a parent's projection is
/// never smaller than its children's.
fn projected_error(sphere: vec4<f32>, error: f32, world_from_local: mat4x4<f32>, scale: f32) -> f32 {
    if error >= 3.0e38 {
        return 3.0e38;
    }
    let center = (world_from_local * vec4(sphere.xyz, 1.0)).xyz;
    let radius = sphere.w * scale;
    let world_error = error * scale;
    let proj = view.clip_from_view[1][1];
    let height = view.viewport.w;
    if view.clip_from_view[3][3] == 1.0 {
        return world_error * proj * 0.5 * height;
    }
    let znear = view.clip_from_view[3][2];
    let distance = max(length(center - view.world_position) - radius, znear);
    return world_error / distance * proj * 0.5 * height;
}

@compute @workgroup_size(64)
fn cull(@builtin(global_invocation_id) id: vec3<u32>, @builtin(num_workgroups) groups: vec3<u32>) {
    let pair = id.x + id.y * groups.x * 64u;
    if pair >= params.pair_count {
        return;
    }

    // The last instance whose pairs start at or before this one.
    var lo = 0u;
    var hi = params.instance_count;
    while lo + 1u < hi {
        let mid = (lo + hi) / 2u;
        if instances[mid].pair_offset <= pair {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    let instance = instances[lo];
    // A thumbnail's model on its own layer must not appear in the main
    // viewport, and the viewport's scene must not appear in the thumbnail.
    if !any((instance.layers & params.view_layers) != vec2(0u)) {
        return;
    }
    let cluster_index = instance.first_cluster + (pair - instance.pair_offset);
    let cluster = clusters[cluster_index];
    let world_from_local = affine3_to_square(instance.world_from_local);

    // The LOD cut, with streaming (`renzora_rvg::GroupGraph` has the rule
    // and why it never leaves a hole): a cluster is drawn when its own error
    // is small enough and its group's simplified one is not, or, past its own
    // error, as the stand-in for finer clusters whose page is not resident
    // yet. Those it asks for.
    if !is_resident(cluster.group) {
        return;
    }
    let threshold = params.lod_threshold;
    if projected_error(cluster.parent, cluster.parent_error, world_from_local, instance.max_scale) <= threshold {
        return;
    }
    let finer_missing = cluster.refined != VG_NO_GROUP && !is_resident(cluster.refined);
    let too_coarse = projected_error(cluster.lod, cluster.lod_error, world_from_local, instance.max_scale) > threshold;
    if too_coarse && !finer_missing {
        return;
    }

    // Left, right, top, bottom, near. The far plane of an infinite reversed-z
    // projection culls nothing. A skinned cluster's sphere is around its rest
    // pose, and an arm raised over the head leaves it entirely: culling by it
    // would cut the arm off, so skinned clusters are only culled per instance,
    // by Bevy's own visibility test, exactly as an ordinary skinned mesh is.
    if (instance.flags & VG_SKINNED) == 0u {
        let center = (world_from_local * vec4(cluster.culling.xyz, 1.0)).xyz;
        let radius = cluster.culling.w * instance.max_scale;
        for (var i = 0u; i < 5u; i++) {
            let plane = view.frustum[i];
            if dot(plane.xyz, center) + plane.w + radius <= 0.0 {
                return;
            }
        }
    }

    // Only what is on screen asks for detail or counts as used.
    // Written out rather than through a helper: WGSL takes no pointer to a
    // storage buffer as a function argument.
    if too_coarse {
        atomicOr(&requests[cluster.refined >> 5u], 1u << (cluster.refined & 31u));
    }
    atomicOr(&drawn[cluster.group >> 5u], 1u << (cluster.group & 31u));

    var slot: u32;
    if (instance.material & (VG_MASKED | VG_TRANSPARENT)) != 0u {
        // Each bucket was given room for every cluster of every instance in
        // it, so the count can never run into the next bucket. A transparent
        // instance's bucket is its alone, drawn as one sorted item.
        let bucket = instance.masked_bucket;
        slot = params.masked_offset + masked_bases[bucket]
            + atomicAdd(&masked_draws[bucket * 4u + 1u], 1u);
    } else if (instance.material & VG_DOUBLE_SIDED) != 0u {
        slot = params.visible_capacity - 1u - atomicAdd(&draws[5], 1u);
    } else {
        slot = atomicAdd(&draws[1], 1u);
    }
    visible[slot] = vec2(lo, cluster_index);
}
