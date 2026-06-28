#import bevy_ui::ui_vertex_output::UiVertexOutput

@group(1) @binding(0) var<uniform> material_color: vec4<f32>;
@group(1) @binding(1) var<uniform> material_stroke_color: vec4<f32>;
// x = stroke_width (pixels)
@group(1) @binding(2) var<uniform> material_params: vec4<f32>;
// IQ-order corner radii (BR, TR, BL, TL) in pixels
@group(1) @binding(3) var<uniform> material_corners: vec4<f32>;

// Signed distance to a box with 4 individually-radiused corners.
// Based on https://iquilezles.org/articles/distfunctions2d/
fn sdf_rounded_box(p: vec2<f32>, b: vec2<f32>, r: vec4<f32>) -> f32 {
    var rr: vec2<f32>;
    if p.x > 0.0 {
        rr = r.xy;
    } else {
        rr = r.zw;
    }
    var ri: f32;
    if p.y > 0.0 {
        ri = rr.x;
    } else {
        ri = rr.y;
    }
    let q = abs(p) - b + vec2<f32>(ri, ri);
    return min(max(q.x, q.y), 0.0) + length(max(q, vec2<f32>(0.0))) - ri;
}

@fragment
fn fragment(in: UiVertexOutput) -> @location(0) vec4<f32> {
    // Work in pixel space centered on 0.
    let size = in.size * 0.5;
    let p = (in.uv * 2.0 - 1.0) * size;

    let stroke_px = material_params.x;

    // Clamp per-corner radii to a sensible max (half the shortest side).
    let max_r = min(size.x, size.y);
    let r = clamp(material_corners, vec4<f32>(0.0), vec4<f32>(max_r));

    let d = sdf_rounded_box(p, size, r);

    // 1 pixel antialiasing
    let aa = 1.0;

    // Fill: inside the interior (past the stroke)
    let fill_alpha = 1.0 - smoothstep(-aa, aa, d + stroke_px);
    var result = material_color * fill_alpha;

    // Stroke: the band of width `stroke_px` inside the edge
    if stroke_px > 0.0 {
        let stroke_alpha = (1.0 - smoothstep(-aa, aa, d)) *
                           smoothstep(-aa, aa, d + stroke_px);
        result = mix(result, material_stroke_color, stroke_alpha * material_stroke_color.a);
    }

    if result.a < 0.001 {
        discard;
    }

    return result;
}
