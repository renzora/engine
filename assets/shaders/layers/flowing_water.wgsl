// Flowing water layer shader
// Produces an animated blue water effect with wave patterns.

fn layer_main(uv: vec2<f32>, world_pos: vec3<f32>, world_normal: vec3<f32>, time: f32) -> vec4<f32> {
    let flow_uv = uv * 4.0 + vec2<f32>(time * 0.15, time * 0.08);
    let wave = sin(flow_uv.x * 6.0) * 0.5 + sin(flow_uv.y * 8.0 + time) * 0.3;
    let depth = 0.4 + wave * 0.1;
    return vec4<f32>(0.05, 0.15 + depth, 0.4 + depth * 0.3, 1.0);
}

fn layer_pbr(uv: vec2<f32>, world_pos: vec3<f32>) -> vec2<f32> {
    return vec2<f32>(0.8, 0.1); // metallic, roughness (shiny water)
}
