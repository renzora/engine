// Rocky ground layer shader
// Produces a procedural rock texture with pseudo-random noise.

fn layer_main(uv: vec2<f32>, world_pos: vec3<f32>, world_normal: vec3<f32>, time: f32) -> vec4<f32> {
    let scale = 8.0;
    let n = fract(sin(dot(floor(uv * scale), vec2<f32>(12.9898, 78.233))) * 43758.5453);
    let base = vec3<f32>(0.35, 0.3, 0.25);
    return vec4<f32>(base + vec3<f32>(n * 0.15 - 0.075), 1.0);
}
