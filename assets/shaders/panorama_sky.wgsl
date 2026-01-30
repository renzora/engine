// Panorama Sky Material Shader
// Renders an equirectangular HDR image on a sky sphere

#import bevy_pbr::forward_io::VertexOutput

@group(2) @binding(0) var panorama_texture: texture_2d<f32>;
@group(2) @binding(1) var panorama_sampler: sampler;
@group(2) @binding(2) var<uniform> brightness: f32;
@group(2) @binding(3) var<uniform> rotation: f32;

const PI: f32 = 3.14159265359;
const TWO_PI: f32 = 6.28318530718;

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    // Get the world position direction (normalized because it's on a sphere)
    let dir = normalize(in.world_position.xyz);

    // Apply rotation around Y axis
    let cos_rot = cos(rotation);
    let sin_rot = sin(rotation);
    let rotated_dir = vec3<f32>(
        dir.x * cos_rot - dir.z * sin_rot,
        dir.y,
        dir.x * sin_rot + dir.z * cos_rot
    );

    // Convert direction to equirectangular UV coordinates
    // Longitude (u): atan2(z, x) mapped to [0, 1]
    // Latitude (v): asin(y) mapped to [0, 1]
    let u = (atan2(rotated_dir.z, rotated_dir.x) + PI) / TWO_PI;
    let v = (asin(clamp(rotated_dir.y, -1.0, 1.0)) + PI * 0.5) / PI;

    // Flip V because texture coordinates typically have Y going down
    let uv = vec2<f32>(u, 1.0 - v);

    // Sample the panorama texture
    let color = textureSample(panorama_texture, panorama_sampler, uv);

    // Apply brightness multiplier
    return vec4<f32>(color.rgb * brightness, 1.0);
}
