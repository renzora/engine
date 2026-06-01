// HSV color picker surfaces: mode 0 = saturation/value square for a given hue,
// mode 1 = vertical hue strip. Output is gamma-encoded to linear for the sRGB UI
// pass so the displayed gradient reads as standard sRGB color.

#import bevy_ui::ui_vertex_output::UiVertexOutput

struct PickerUniforms {
    // x = mode (0 = SV square, 1 = hue strip), y = hue (0..1)
    params: vec4<f32>,
};

@group(1) @binding(0)
var<uniform> u: PickerUniforms;

fn hsv2rgb(h: f32, s: f32, v: f32) -> vec3<f32> {
    let c = v * s;
    let hp = h * 6.0;
    let x = c * (1.0 - abs(hp % 2.0 - 1.0));
    let m = v - c;
    var rgb = vec3<f32>(0.0, 0.0, 0.0);
    if (hp < 1.0) { rgb = vec3<f32>(c, x, 0.0); }
    else if (hp < 2.0) { rgb = vec3<f32>(x, c, 0.0); }
    else if (hp < 3.0) { rgb = vec3<f32>(0.0, c, x); }
    else if (hp < 4.0) { rgb = vec3<f32>(0.0, x, c); }
    else if (hp < 5.0) { rgb = vec3<f32>(x, 0.0, c); }
    else { rgb = vec3<f32>(c, 0.0, x); }
    return rgb + m;
}

@fragment
fn fragment(in: UiVertexOutput) -> @location(0) vec4<f32> {
    var srgb: vec3<f32>;
    if (u.params.x < 0.5) {
        srgb = hsv2rgb(u.params.y, in.uv.x, 1.0 - in.uv.y);
    } else {
        srgb = hsv2rgb(in.uv.y, 1.0, 1.0);
    }
    return vec4<f32>(pow(srgb, vec3<f32>(2.2)), 1.0);
}
