// Grid Material Shader — unlit per-vertex line color with distance fade.
//
// All grid lines (major, minor, axes) share a single mesh and material
// so the GPU sees one draw call. Per-line color is baked into the
// vertex COLOR attribute; the material only carries the fade range.
//
// Fades alpha toward zero as the fragment's horizontal distance from the
// camera approaches `fade_end`, so the grid softly dissolves into the
// background rather than ending abruptly (Blender-style).

#import bevy_pbr::forward_io::VertexOutput
#import bevy_pbr::mesh_view_bindings::view

struct GridMaterialUniform {
    // fade_start: distance at which fade begins (full alpha below this).
    // fade_end:   distance at which alpha reaches zero.
    fade_start: f32,
    fade_end: f32,
    _pad0: f32,
    _pad1: f32,
};

@group(3) @binding(0) var<uniform> material: GridMaterialUniform;

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let cam_xz = vec2<f32>(view.world_position.x, view.world_position.z);
    let frag_xz = vec2<f32>(in.world_position.x, in.world_position.z);
    let d = distance(cam_xz, frag_xz);
    let fade = 1.0 - smoothstep(material.fade_start, material.fade_end, d);
    return vec4<f32>(in.color.rgb, in.color.a * fade);
}
