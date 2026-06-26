// Built-in image display — cover-fits the theme image into the surface and
// composites it over the theme background using the image's alpha. Used when a
// theme sets an image for a surface but no shader of its own. A custom theme
// shader can sample the same image at @binding(1)/(2).
#import bevy_ui::ui_vertex_output::UiVertexOutput

struct ThemeUniforms { params: vec4<f32>, bg: vec4<f32>, accent: vec4<f32>, fg: vec4<f32>, };
@group(1) @binding(0) var<uniform> u: ThemeUniforms;
@group(1) @binding(1) var theme_tex: texture_2d<f32>;
@group(1) @binding(2) var theme_samp: sampler;

@fragment
fn fragment(in: UiVertexOutput) -> @location(0) vec4<f32> {
    let tex_size = vec2<f32>(textureDimensions(theme_tex));
    let node = in.size;
    // Cover-fit: scale so the image fills the node, cropping the overflow.
    let scale = max(node.x / max(tex_size.x, 1.0), node.y / max(tex_size.y, 1.0));
    let disp = tex_size * scale;
    let uv = (in.uv * node - (node - disp) * 0.5) / disp;
    let c = textureSample(theme_tex, theme_samp, uv);
    // sRGB textures are auto-decoded to linear on sample, so output directly.
    let color = mix(u.bg.rgb, c.rgb, c.a);
    return vec4<f32>(color, 1.0);
}
