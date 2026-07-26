// SDF text — sample a signed-distance-field glyph atlas and reconstruct a crisp
// edge at ANY magnification. The atlas stores distance in the red channel with
// 0.5 == the glyph outline (>0.5 inside, <0.5 outside). `fwidth` gives the
// screen-space rate of change of that distance, so the anti-alias band is ~1
// pixel wide however close the camera gets — the whole point of SDF text.

#import bevy_pbr::forward_io::VertexOutput

@group(3) @binding(0) var<uniform> text_color: vec4<f32>;
@group(3) @binding(1) var sdf_atlas: texture_2d<f32>;
@group(3) @binding(2) var sdf_sampler: sampler;

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let d = textureSample(sdf_atlas, sdf_sampler, in.uv).r;
    // Half-pixel anti-alias width from the screen-space derivative of the field.
    let aa = max(fwidth(d), 0.0001);
    let alpha = smoothstep(0.5 - aa, 0.5 + aa, d);
    if (alpha <= 0.001) {
        discard;
    }
    return vec4<f32>(text_color.rgb, text_color.a * alpha);
}
