#import bevy_ui::ui_vertex_output::UiVertexOutput

@group(1) @binding(0) var<uniform> material_color: vec4<f32>;
@group(1) @binding(1) var<uniform> material_stroke_color: vec4<f32>;
// x = stroke_width (pixels), y = rotation (radians)
@group(1) @binding(2) var<uniform> material_params: vec4<f32>;

const SQRT3: f32 = 1.7320508;

// Equilateral triangle SDF centered at origin, pointing up
fn sdf_triangle(p: vec2<f32>) -> f32 {
    var q = p;
    q.x = abs(q.x) - 0.5;
    q.y = q.y + 0.5 / SQRT3;
    if q.x + SQRT3 * q.y > 0.0 {
        q = vec2<f32>(q.x - SQRT3 * q.y, -SQRT3 * q.x - q.y) / 2.0;
    }
    q.x -= clamp(q.x, -1.0, 0.0);
    return -length(q) * sign(q.y);
}

@fragment
fn fragment(in: UiVertexOutput) -> @location(0) vec4<f32> {
    let uv = in.uv * 2.0 - 1.0;
    let aa = 2.0 / min(in.size.x, in.size.y);

    let angle = material_params.y;
    let ca = cos(angle);
    let sa = sin(angle);
    let rotated = vec2<f32>(
        uv.x * ca - uv.y * sa,
        uv.x * sa + uv.y * ca,
    );

    let d = sdf_triangle(rotated * 1.15); // Scale to fit the node

    let stroke_px = material_params.x;
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

    let outer_alpha = 1.0 - smoothstep(-aa, aa, d);
    if fill_alpha < 0.001 && (stroke_norm <= 0.0 || outer_alpha < 0.001) {
        discard;
    }

    return result;
}
