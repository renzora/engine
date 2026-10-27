//! Deform every skinned instance's vertices for this frame.
//!
//! One thread per output vertex. Each skinned instance is one job: a run of its
//! mesh's vertices, the mesh's joints and weights, and where its skin's joint
//! matrices start in Bevy's own joint buffers. Bevy's joint matrices already
//! carry the model's place in the world (`skinning.wesl`'s `skin_model`), so the
//! result is in world space and the raster, resolve and debug passes read it
//! instead of transforming the rest pose.
//!
//! Done in one pass ahead of raster, rather than in the raster vertex shader,
//! because a vertex is shared by every triangle touching it and the resolve pass
//! reads three of them per pixel: skinning there would repeat the same 4 matrix
//! reads many times over.

import super::types::VgSkinnedVertex;
import bevy_render::utils::octahedral_decode_signed;

struct SkinJob {
    /// The mesh's first vertex in `positions` and `attributes`.
    vertex_base: u32,
    /// Its first vertex in `skin`.
    skin_base: u32,
    /// Its skin's first joint matrix in `joints` and `previous_joints`.
    joint_base: u32,
    /// Its first vertex in `skinned`, and so the first thread of the job.
    out_base: u32,
}

struct SkinParams {
    vertex_count: u32,
    job_count: u32,
    _pad0: u32,
    _pad1: u32,
}

@group(0) @binding(0) var<uniform> params: SkinParams;
@group(0) @binding(1) var<storage, read> jobs: array<SkinJob>;
/// 3 words per vertex: joints 0 and 1 as two u16, joints 2 and 3 likewise,
/// then the four weights as unorm8.
@group(0) @binding(2) var<storage, read> skin: array<u32>;
@group(0) @binding(3) var<storage, read> positions: array<f32>;
@group(0) @binding(4) var<storage, read> attributes: array<u32>;
@group(0) @binding(5) var<storage, read> joints: array<mat4x4<f32>>;
@group(0) @binding(6) var<storage, read> previous_joints: array<mat4x4<f32>>;
@group(0) @binding(7) var<storage, read_write> skinned: array<VgSkinnedVertex>;

fn inverse_transpose_3x3(m: mat3x3<f32>) -> mat3x3<f32> {
    let x = cross(m[1], m[2]);
    let y = cross(m[2], m[0]);
    let z = cross(m[0], m[1]);
    let det = dot(m[2], z);
    return mat3x3(x / det, y / det, z / det);
}

@compute @workgroup_size(64)
fn skin_vertices(@builtin(global_invocation_id) id: vec3<u32>, @builtin(num_workgroups) groups: vec3<u32>) {
    let out = id.x + id.y * groups.x * 64u;
    if out >= params.vertex_count {
        return;
    }

    // The last job starting at or before this vertex.
    var lo = 0u;
    var hi = params.job_count;
    while lo + 1u < hi {
        let mid = (lo + hi) / 2u;
        if jobs[mid].out_base <= out {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    let job = jobs[lo];
    let local = out - job.out_base;
    let vertex = job.vertex_base + local;

    let s = (job.skin_base + local) * 3u;
    let indexes = vec4(
        extractBits(skin[s], 0u, 16u),
        extractBits(skin[s], 16u, 16u),
        extractBits(skin[s + 1u], 0u, 16u),
        extractBits(skin[s + 1u], 16u, 16u),
    ) + vec4(job.joint_base);
    // Eight bits each no longer sum to exactly one; renormalized, or the
    // mesh would shrink or swell by up to a percent.
    let packed = unpack4x8unorm(skin[s + 2u]);
    let weights = packed / max(dot(packed, vec4(1.0)), 1e-6);

    let model = weights.x * joints[indexes.x] + weights.y * joints[indexes.y]
        + weights.z * joints[indexes.z] + weights.w * joints[indexes.w];
    let previous_model = weights.x * previous_joints[indexes.x] + weights.y * previous_joints[indexes.y]
        + weights.z * previous_joints[indexes.z] + weights.w * previous_joints[indexes.w];

    let b = vertex * 3u;
    let position = vec4(positions[b], positions[b + 1u], positions[b + 2u], 1.0);
    let normal = octahedral_decode_signed(unpack2x16snorm(attributes[vertex * 4u]));
    let tangent_word = attributes[vertex * 4u + 1u];
    let tangent = octahedral_decode_signed(unpack2x16snorm(tangent_word & ~1u));
    let model3 = mat3x3(model[0].xyz, model[1].xyz, model[2].xyz);
    // A mirroring joint flips the bitangent, as a mirroring transform does for
    // an ordinary mesh.
    var sign = select(1.0, -1.0, (tangent_word & 1u) != 0u);
    if determinant(model3) < 0.0 {
        sign = -sign;
    }

    var v: VgSkinnedVertex;
    v.position = vec4((model * position).xyz, 1.0);
    v.previous = vec4((previous_model * position).xyz, 1.0);
    v.normal = vec4(normalize(inverse_transpose_3x3(model3) * normal), 0.0);
    // Without tangents of its own the mesh's tangent words are zero, and the
    // resolve derives a frame from UV gradients instead of reading this.
    v.tangent = vec4(model3 * tangent, sign);
    skinned[out] = v;
}
