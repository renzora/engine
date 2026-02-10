// Default layer shader — procedural checkerboard (matches terrain material)

fn layer_main(uv: vec2<f32>, world_pos: vec3<f32>, world_normal: vec3<f32>, time: f32) -> vec4<f32> {
    let scale = 0.5;
    let checker = floor(world_pos.x * scale) + floor(world_pos.z * scale);
    let checker_value = fract(checker * 0.5) * 2.0;

    let color_a = vec3<f32>(0.85, 0.85, 0.85);
    let color_b = vec3<f32>(0.65, 0.65, 0.65);
    let base_color = mix(color_a, color_b, checker_value);

    return vec4<f32>(base_color, 1.0);
}
