// Default layer shader — dark grey surface.

fn layer_main(uv: vec2<f32>, world_pos: vec3<f32>, world_normal: vec3<f32>, time: f32) -> vec4<f32> {
    return vec4<f32>(0.35, 0.35, 0.35, 1.0);
}
