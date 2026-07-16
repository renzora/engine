#define_import_path bevy_gaussian_splatting::depth

fn depth_to_rgb(depth: f32, min_depth: f32, max_depth: f32) -> vec3<f32> {
    let normalized_depth = clamp((depth - min_depth) / (max_depth - min_depth), 0.0, 1.0);

    let r = smoothstep(0.5, 1.0, normalized_depth);
    let g = 1.0 - abs(normalized_depth - 0.5) * 2.0;
    let b = 1.0 - smoothstep(0.0, 0.5, normalized_depth);

    return vec3<f32>(r, g, b);
}
