// Terrain Checkerboard Material — procedural world-space checkerboard with PBR lighting
//
// Uses world position (not UVs) for infinite-resolution tiling.

#import bevy_pbr::forward_io::VertexOutput
#import bevy_pbr::pbr_functions
#import bevy_pbr::pbr_types::{PbrInput, pbr_input_new}

struct TerrainCheckerboardUniform {
    color_a: vec4<f32>,
    color_b: vec4<f32>,
    // x = scale (squares per world unit), y = metallic, z = roughness
    properties: vec4<f32>,
};

@group(3) @binding(0) var<uniform> material: TerrainCheckerboardUniform;

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let scale = material.properties.x;

    // Dominant-axis triplanar projection so the perimeter walls and floor
    // also show the checker. world.xz alone collapses to a single stripe on
    // any vertical face — pick whichever pair of world axes is parallel to
    // the face by looking at the largest component of the world normal.
    let world_pos = in.world_position.xyz;
    let an = abs(normalize(in.world_normal));
    var proj: vec2<f32>;
    if an.y >= an.x && an.y >= an.z {
        proj = world_pos.xz;
    } else if an.x >= an.z {
        proj = world_pos.yz;
    } else {
        proj = world_pos.xy;
    }

    // Checkerboard pattern from the projected coords
    let checker = floor(proj.x * scale) + floor(proj.y * scale);
    let t = fract(checker * 0.5) * 2.0;

    let base_color = vec4<f32>(mix(material.color_a.rgb, material.color_b.rgb, t), 1.0);

    let N = normalize(in.world_normal);
    let V = pbr_functions::calculate_view(in.world_position, false);

    // PBR lighting
    var pbr_input: PbrInput = pbr_input_new();
    pbr_input.material.base_color = base_color;
    pbr_input.material.metallic = material.properties.y;
    pbr_input.material.perceptual_roughness = material.properties.z;
    pbr_input.world_normal = N;
    pbr_input.world_position = in.world_position;
    pbr_input.N = N;
    pbr_input.V = V;

    var color = pbr_functions::apply_pbr_lighting(pbr_input);
    color.a = 1.0;
    return color;
}
