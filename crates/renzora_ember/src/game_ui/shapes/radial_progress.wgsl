#import bevy_ui::ui_vertex_output::UiVertexOutput

@group(1) @binding(0) var<uniform> material_color: vec4<f32>;
@group(1) @binding(1) var<uniform> material_bg_color: vec4<f32>;
// x = value (0-1), y = thickness (0-1)
@group(1) @binding(2) var<uniform> material_params: vec4<f32>;

const PI: f32 = 3.14159265359;
const TAU: f32 = 6.28318530718;

@fragment
fn fragment(in: UiVertexOutput) -> @location(0) vec4<f32> {
    let uv = in.uv * 2.0 - 1.0;
    let dist = length(uv);
    let aa = 2.0 / min(in.size.x, in.size.y);

    let value = material_params.x;
    let thickness = material_params.y;

    let outer = 1.0;
    let inner = 1.0 - thickness;

    // Ring mask
    let ring_alpha = (1.0 - smoothstep(outer - aa, outer, dist)) *
                     smoothstep(inner - aa, inner, dist);

    if ring_alpha < 0.001 {
        discard;
    }

    // Angle: clockwise from top (12 o'clock)
    // atan2 gives -PI..PI with 0 at right. Rotate so 0 = top.
    var angle = atan2(uv.x, -uv.y); // 0 = top, clockwise positive
    if angle < 0.0 {
        angle += TAU;
    }

    let threshold = value * TAU;
    let in_fill = select(0.0, 1.0, angle <= threshold);

    let fill = mix(material_bg_color, material_color, in_fill);

    return fill * ring_alpha;
}
