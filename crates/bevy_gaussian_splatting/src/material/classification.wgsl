#define_import_path bevy_gaussian_splatting::classification

#import bevy_render::color_operations::{
    hsv_to_rgb,
    rgb_to_hsv,
}
#import bevy_gaussian_splatting::bindings::gaussian_uniforms

fn class_to_rgb(
    visualization: f32,
    sh_color: vec3<f32>,
) -> vec3<f32> {
    if visualization < 2.0 {
        return sh_color;
    }

    let class_idx = visualization - 2.0;
    let hue = (class_idx / f32(gaussian_uniforms.num_classes)) * 6.283185307;

    return mix(
        sh_color,
        hsv_to_rgb(
            vec3<f32>(hue, 1.0, 1.0)
        ),
        0.5
    );
}
