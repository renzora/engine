#import bevy_ui::ui_vertex_output::UiVertexOutput

struct CircleMaterial {
    color: vec4<f32>,
    stroke_color: vec4<f32>,
    // x = stroke_width in pixels
    params: vec4<f32>,
};

@group(1) @binding(0) var<uniform> material_color: vec4<f32>;
@group(1) @binding(1) var<uniform> material_stroke_color: vec4<f32>;
@group(1) @binding(2) var<uniform> material_params: vec4<f32>;

@fragment
fn fragment(in: UiVertexOutput) -> @location(0) vec4<f32> {
    let uv = in.uv * 2.0 - 1.0; // -1..1
    let dist = length(uv);

    let aa = 2.0 / min(in.size.x, in.size.y); // anti-alias width

    // Stroke width normalized to radius
    let stroke_px = material_params.x;
    let stroke_norm = stroke_px / (min(in.size.x, in.size.y) * 0.5);

    let outer = 1.0;
    let inner = outer - stroke_norm;

    // Fill: inside the inner edge
    let fill_alpha = 1.0 - smoothstep(inner - aa, inner, dist);
    var result = material_color * fill_alpha;

    // Stroke: between inner and outer edge
    if stroke_norm > 0.0 {
        let stroke_alpha = (1.0 - smoothstep(outer - aa, outer, dist)) *
                           smoothstep(inner - aa, inner, dist);
        result = mix(result, material_stroke_color, stroke_alpha * material_stroke_color.a);
    }

    // Outer clip
    let outer_alpha = 1.0 - smoothstep(outer - aa, outer, dist);
    result.a *= outer_alpha;

    if result.a < 0.001 {
        discard;
    }

    return result;
}
