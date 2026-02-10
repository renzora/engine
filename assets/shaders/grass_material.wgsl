// Grass Fragment Shader
// Renders grass blades with a root-to-tip color gradient,
// simple subsurface scattering, and PBR lighting.

#import bevy_pbr::{
    pbr_functions::pbr,
    pbr_types::PbrInput,
    pbr_types::pbr_input_new,
    mesh_view_bindings::view,
    mesh_view_bindings::globals,
    forward_io::VertexOutput,
}

struct GrassUniforms {
    base_color: vec4<f32>,
    tip_color: vec4<f32>,
    // x = wind dir X, y = wind dir Z, z = strength, w = speed
    wind_params: vec4<f32>,
};

@group(3) @binding(0) var<uniform> material: GrassUniforms;

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    // Height along the blade (0 = root, 1 = tip)
    let height = clamp(in.uv.y, 0.0, 1.0);

    // Blend root → tip color with a slight ease-in curve
    let blend = height * height;
    var base_color = mix(material.base_color, material.tip_color, blend);

    // Subtle per-blade color variation based on world position
    let variation = sin(dot(floor(in.world_position.xz * 2.0), vec2<f32>(12.9898, 78.233)));
    let variation_norm = fract(variation * 43758.5453) * 0.15 - 0.075;
    base_color = vec4<f32>(
        clamp(base_color.r + variation_norm, 0.0, 1.0),
        clamp(base_color.g + variation_norm * 0.5, 0.0, 1.0),
        clamp(base_color.b + variation_norm * 0.3, 0.0, 1.0),
        base_color.a,
    );

    // Darken at the root to fake ambient occlusion
    let ao = mix(0.4, 1.0, height);
    base_color = vec4<f32>(base_color.rgb * ao, base_color.a);

    // Subsurface scattering approximation: brighten when lit from behind
    let view_dir = normalize(view.world_position.xyz - in.world_position.xyz);
    let normal = normalize(in.world_normal);
    let backlight = max(dot(-normal, view_dir), 0.0) * 0.3;
    let sss_color = vec3<f32>(0.2, 0.5, 0.05) * backlight * height;

    // PBR lighting
    var pbr_input: PbrInput = pbr_input_new();
    pbr_input.material.base_color = base_color;
    pbr_input.material.metallic = 0.0;
    pbr_input.material.perceptual_roughness = 0.8;
    pbr_input.material.reflectance = 0.2;
    pbr_input.material.emissive = vec4<f32>(sss_color, 0.0);
    pbr_input.occlusion = vec3<f32>(ao);
    pbr_input.world_normal = normal;
    pbr_input.world_position = vec4<f32>(in.world_position.xyz, 1.0);
    pbr_input.frag_coord = in.position;

    var color = pbr(pbr_input);

    // No alpha cutoff — use vertex colors or a separate alpha if needed
    color.a = base_color.a;

    return color;
}
