// Gizmo Material Shader — unlit, always-on-top
// Renders gizmo meshes with a flat solid color, ignoring scene depth.

#import bevy_pbr::forward_io::VertexOutput

struct GizmoMaterialUniform {
    base_color: vec4<f32>,
    emissive: vec4<f32>,
};

@group(3) @binding(0) var<uniform> material: GizmoMaterialUniform;

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    return vec4<f32>(material.base_color.rgb, 1.0);
}
