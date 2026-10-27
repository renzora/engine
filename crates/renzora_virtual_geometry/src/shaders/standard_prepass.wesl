//! `StandardMaterial`'s prepass fragment shader for virtual geometry.
//!
//! Bevy's `pbr_prepass.wesl` (MIT OR Apache-2.0), cut down to its
//! `MESHLET_MESH_MATERIAL_PASS` branch, with the one thing that branch is
//! missing: Bevy declares the material's `flags` and `uv_transform` only for
//! ordinary meshes, so the visibility buffer variant does not compile. Here they
//! are read through the instance's material slot, as `pbr_fragment.wesl`
//! already does for the forward pass. Everything else is unchanged, so normal
//! maps reach the prepass normals exactly as they do on ordinary geometry.

import bevy_pbr::{
    render::{
        pbr_bindings,
        pbr_types,
        pbr_functions,
        pbr_functions::SampleBias,
        mesh_view_bindings::view,
    },
    prepass::io as prepass_io,
};
import bevy_pbr::meshlet::visibility_buffer_resolve::resolve_vertex_output;
import bevy_render::bindless::{bindless_samplers_filtering, bindless_textures_2d};

@if(BINDLESS)
import bevy_pbr::render::pbr_bindings::material_indices;

@fragment
fn fragment(@builtin(position) frag_coord: vec4<f32>) -> prepass_io::FragmentOutput {
    let in = resolve_vertex_output(frag_coord);
    let is_front = true;

    let slot = in.material_bind_group_slot;
@if(BINDLESS)
    let flags = pbr_bindings::material_array[material_indices[slot].material].flags;
@if(BINDLESS)
    let uv_transform = pbr_bindings::material_array[material_indices[slot].material].uv_transform;
@if(!BINDLESS)
    let flags = pbr_bindings::material.flags;
@if(!BINDLESS)
    let uv_transform = pbr_bindings::material.uv_transform;

    var out: prepass_io::FragmentOutput;

@if(NORMAL_PREPASS)
    if (flags & pbr_types::STANDARD_MATERIAL_FLAGS_UNLIT_BIT) == 0u {
        let double_sided = (flags & pbr_types::STANDARD_MATERIAL_FLAGS_DOUBLE_SIDED_BIT) != 0u;
        let is_logically_front = pbr_functions::winding_corrected_front_facing(in.mesh_flags, is_front);

        let world_normal = pbr_functions::prepare_world_normal(
            in.world_normal,
            double_sided,
            is_logically_front,
        );

        var normal = world_normal;

@if(VERTEX_UVS) {
@if(VERTEX_TANGENTS) {
@if(STANDARD_MATERIAL_NORMAL_MAP) {
        let uv = (uv_transform * vec3(in.uv, 1.0)).xy;

        var bias: SampleBias;
        let mip_scale = exp2(view.mip_bias);
        bias.ddx_uv = in.ddx_uv * mip_scale;
        bias.ddy_uv = in.ddy_uv * mip_scale;

@if(BINDLESS)
        let texture = bindless_textures_2d[material_indices[slot].normal_map_texture];
@else
        let texture = pbr_bindings::normal_map_texture;
@if(BINDLESS)
        let sampler_ = bindless_samplers_filtering[material_indices[slot].normal_map_sampler];
@else
        let sampler_ = pbr_bindings::normal_map_sampler;
        let Nt = textureSampleGrad(texture, sampler_, uv, bias.ddx_uv, bias.ddy_uv).rgb;
        let TBN = pbr_functions::calculate_tbn_mikktspace(normal, in.world_tangent);

        normal = pbr_functions::apply_normal_mapping(
            flags,
            TBN,
            double_sided,
            is_logically_front,
            Nt,
        );
}
}
}

        out.normal = vec4(normal * 0.5 + vec3(0.5), 1.0);
    } else {
        out.normal = vec4(in.world_normal * 0.5 + vec3(0.5), 1.0);
    }

@if(MOTION_VECTOR_PREPASS)
    out.motion_vector = in.motion_vector;

    return out;
}
