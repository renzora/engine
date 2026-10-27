//! Rasterize the visible clusters into the visibility buffer and the view's
//! own depth buffer. One draw instance per cluster, 128 triangles' worth of
//! vertices each; the corners of triangles a cluster does not have collapse
//! to a point and rasterize nothing.

import super::types::{VgCullParams, VG_SIGN_DETERMINANT};
import super::bindings::{vg_visible, vg_instances, vg_clusters, vg_triangle_vertex, vg_world_position};
import bevy_render::view::View;

@group(0) @binding(0) var<uniform> view: View;
@group(0) @binding(1) var<uniform> params: VgCullParams;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) @interpolate(flat) packed: u32,
}

@vertex
fn vertex(@builtin(instance_index) instance_index: u32, @builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    @if(DOUBLE_SIDED)
    let slot = params.visible_capacity - 1u - instance_index;
    @else
    let slot = instance_index;

    let entry = vg_visible[slot];
    let instance = vg_instances[entry.x];
    let cluster = vg_clusters[entry.y];

    let triangle = vertex_index / 3u;
    if triangle >= cluster.triangle_count {
        return VertexOutput(vec4(0.0), 0u);
    }
    // A mirroring transform turns every triangle inside out; swapping two
    // corners puts the winding, and so back-face culling, right again.
    var corner = vertex_index % 3u;
    if (instance.flags & VG_SIGN_DETERMINANT) == 0u && corner != 0u {
        corner = 3u - corner;
    }
    let vertex = vg_triangle_vertex(cluster, triangle, corner);
    let world = vg_world_position(instance, vertex);
    return VertexOutput(view.clip_from_world * vec4(world, 1.0), ((slot << 7u) | triangle) + 1u);
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) u32 {
    return in.packed;
}
