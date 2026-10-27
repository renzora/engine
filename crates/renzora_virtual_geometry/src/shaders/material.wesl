//! The vertex half of every material pass: one fullscreen triangle per
//! material, placed at that material's id as depth. The fragment half is the
//! material's own shader, compiled with `MESHLET_MESH_MATERIAL_PASS` so it
//! reads its surface from `resolve_vertex_output` instead of a vertex stage.

import bevy_pbr::render::view_transformations::uv_to_ndc;

@vertex
fn vertex(@builtin(vertex_index) vertex_input: u32) -> @builtin(position) vec4<f32> {
    let vertex_index = vertex_input % 3u;
    let material_id = vertex_input / 3u;
    let material_depth = f32(material_id) / 65535.0;
    let uv = vec2<f32>(vec2(vertex_index >> 1u, vertex_index & 1u)) * 2.0;
    return vec4(uv_to_ndc(uv), material_depth, 1.0);
}
