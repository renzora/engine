#import bevy_ui::ui_vertex_output::UiVertexOutput

@group(1) @binding(0) var<uniform> material_color: vec4<f32>;
@group(1) @binding(1) var<uniform> material_stroke_color: vec4<f32>;
// x = stroke_width (pixels), y = sides, z = rotation (radians)
@group(1) @binding(2) var<uniform> material_params: vec4<f32>;

const PI: f32 = 3.14159265359;
const TAU: f32 = 6.28318530718;

// Regular polygon SDF
fn sdf_polygon(p: vec2<f32>, n: f32, r: f32) -> f32 {
    let an = TAU / n;
    let he = r * tan(an * 0.5);

    // Sector angle
    var a = atan2(p.y, p.x);
    a = a - an * floor(a / an + 0.5);

    // Rotated point within sector
    let q = vec2<f32>(cos(a), abs(sin(a)));

    return dot(q, vec2<f32>(he, r)) - he;
}

@fragment
fn fragment(in: UiVertexOutput) -> @location(0) vec4<f32> {
    let uv = in.uv * 2.0 - 1.0;
    let aa = 2.0 / min(in.size.x, in.size.y);

    let stroke_px = material_params.x;
    let sides = material_params.y;
    let rotation = material_params.z;

    // Rotate
    let ca = cos(rotation);
    let sa = sin(rotation);
    let rotated = vec2<f32>(
        uv.x * ca - uv.y * sa,
        uv.x * sa + uv.y * ca,
    );

    let d = sdf_polygon(rotated, sides, 0.8);
    let stroke_norm = stroke_px / (min(in.size.x, in.size.y) * 0.5);

    // Fill
    let fill_alpha = 1.0 - smoothstep(-aa, aa, d + stroke_norm);
    var result = material_color * fill_alpha;

    // Stroke
    if stroke_norm > 0.0 {
        let stroke_alpha = (1.0 - smoothstep(-aa, aa, d)) *
                           smoothstep(-aa, aa, d + stroke_norm);
        result = mix(result, material_stroke_color, stroke_alpha * material_stroke_color.a);
    }

    if result.a < 0.001 {
        discard;
    }

    return result;
}
