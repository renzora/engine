//! Rasterize alpha-masked clusters: leaves, grass cards, fences. The same as
//! `raster.wesl`, except that each fragment samples the material's base colour
//! alpha and is discarded below the cutoff, so a card only covers the shape cut
//! into its texture. What is left writes the visibility buffer and depth
//! exactly as an opaque cluster would, and is shaded the same way.
//!
//! The alpha test follows `StandardMaterial`'s own (`pbr_fragment.wesl`):
//! base colour times the base colour texture, UVs through `uv_transform`. One
//! draw per material bind group, which with bindless materials is one draw for
//! many materials; the material comes from the instance's slot.

import super::types::{VgCullParams, VG_SIGN_DETERMINANT};
import super::bindings::{vg_visible, vg_instances, vg_clusters, vg_triangle_vertex, vg_world_position, vg_uv};
import bevy_render::view::View;
import bevy_render::bindless::{bindless_samplers_filtering, bindless_textures_2d};
import bevy_pbr::render::{pbr_bindings, pbr_types};

@if(BINDLESS)
import bevy_pbr::render::pbr_bindings::material_indices;

@group(0) @binding(0) var<uniform> view: View;
@group(0) @binding(1) var<uniform> params: VgCullParams;

/// Where this draw's slots start in the visible list.
struct Bucket {
    base: u32,
}
@group(3) @binding(0) var<uniform> bucket: Bucket;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) @interpolate(flat) packed: u32,
    @location(1) @interpolate(flat) material_slot: u32,
    @location(2) uv: vec2<f32>,
}

@vertex
fn vertex(@builtin(instance_index) instance_index: u32, @builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    let slot = bucket.base + instance_index;
    let entry = vg_visible[slot];
    let instance = vg_instances[entry.x];
    let cluster = vg_clusters[entry.y];

    let triangle = vertex_index / 3u;
    if triangle >= cluster.triangle_count {
        return VertexOutput(vec4(0.0), 0u, 0u, vec2(0.0));
    }
    var corner = vertex_index % 3u;
    if (instance.flags & VG_SIGN_DETERMINANT) == 0u && corner != 0u {
        corner = 3u - corner;
    }
    let vertex = vg_triangle_vertex(cluster, triangle, corner);
    let world = vg_world_position(instance, vertex);
    return VertexOutput(
        view.clip_from_world * vec4(world, 1.0),
        ((slot << 7u) | triangle) + 1u,
        instance.material_slot,
        vg_uv(vertex),
    );
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) u32 {
    // Derivatives first, in uniform control flow, so the texture can be
    // sampled at the right mip inside the branch below. A distant leaf
    // sampled at full resolution would flicker in and out.
    let ddx_uv = dpdx(in.uv);
    let ddy_uv = dpdy(in.uv);

    let slot = in.material_slot;
@if(BINDLESS)
    let material = pbr_bindings::material_array[material_indices[slot].material];
@else
    let material = pbr_bindings::material;

    var alpha = material.base_color.a;
    if (material.flags & pbr_types::STANDARD_MATERIAL_FLAGS_BASE_COLOR_TEXTURE_BIT) != 0u {
        let transform = mat2x2(material.uv_transform[0].xy, material.uv_transform[1].xy);
        let uv = (material.uv_transform * vec3(in.uv, 1.0)).xy;
@if(BINDLESS)
        let texture = bindless_textures_2d[material_indices[slot].base_color_texture];
@else
        let texture = pbr_bindings::base_color_texture;
@if(BINDLESS)
        let sampler_ = bindless_samplers_filtering[material_indices[slot].base_color_sampler];
@else
        let sampler_ = pbr_bindings::base_color_sampler;
        alpha *= textureSampleGrad(texture, sampler_, uv, transform * ddx_uv, transform * ddy_uv).a;
    }
    if alpha < material.alpha_cutoff {
        discard;
    }
    return in.packed;
}
