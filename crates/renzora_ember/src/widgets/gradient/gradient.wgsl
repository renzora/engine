// Gradient editor bar: interpolates between up to 6 color stops (rgb in 0..1,
// position packed in .w). Output is gamma-encoded for the sRGB UI pass.

#import bevy_ui::ui_vertex_output::UiVertexOutput

struct GradientUniforms {
    colors: array<vec4<f32>, 6>, // rgb = color, w = position (0..1)
    params: vec4<f32>,           // x = stop count
};

@group(1) @binding(0)
var<uniform> u: GradientUniforms;

@fragment
fn fragment(in: UiVertexOutput) -> @location(0) vec4<f32> {
    let t = in.uv.x;
    let count = i32(u.params.x);
    var col = u.colors[0].rgb;
    for (var i = 0; i < count - 1; i = i + 1) {
        let a = u.colors[i];
        let b = u.colors[i + 1];
        if (t >= a.w && t <= b.w) {
            col = mix(a.rgb, b.rgb, (t - a.w) / max(b.w - a.w, 1e-4));
        }
    }
    if (t <= u.colors[0].w) {
        col = u.colors[0].rgb;
    }
    if (count > 0 && t >= u.colors[count - 1].w) {
        col = u.colors[count - 1].rgb;
    }
    return vec4<f32>(pow(col, vec3<f32>(2.2)), 1.0);
}
