// Gizmo Material Shader — unlit, always-on-top
// Renders gizmo meshes with a flat color, ignoring scene depth.

#import bevy_pbr::forward_io::VertexOutput

struct GizmoMaterialUniform {
    base_color: vec4<f32>,
    emissive: vec4<f32>,
};

// Bevy 0.18 bind groups: 0=view, 1=globals, 2=mesh, 3=material
@group(3) @binding(0) var<uniform> material: GizmoMaterialUniform;

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    return material.base_color + material.emissive;
}
