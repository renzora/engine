#import bevy_ui::ui_vertex_output::UiVertexOutput

@group(1) @binding(0) var<uniform> material_color: vec4<f32>;
// x = start_angle, y = end_angle, z = inner_radius
@group(1) @binding(1) var<uniform> material_params: vec4<f32>;

const PI: f32 = 3.14159265359;
const TAU: f32 = 6.28318530718;

@fragment
fn fragment(in: UiVertexOutput) -> @location(0) vec4<f32> {
    let uv = in.uv * 2.0 - 1.0;
    let dist = length(uv);
    let aa = 2.0 / min(in.size.x, in.size.y);

    let start = material_params.x;
    let end = material_params.y;
    let inner = material_params.z;

    // Outer circle clip
    let outer_alpha = 1.0 - smoothstep(1.0 - aa, 1.0, dist);

    // Inner hole
    let inner_alpha = smoothstep(inner - aa, inner, dist);

    // Angle: clockwise from top
    var angle = atan2(uv.x, -uv.y);
    if angle < 0.0 {
        angle += TAU;
    }

    // Handle wrapping
    var arc_start = start;
    var arc_end = end;
    if arc_end < arc_start {
        arc_end += TAU;
    }

    var a = angle;
    if a < arc_start {
        a += TAU;
    }

    let in_wedge = select(0.0, 1.0, a >= arc_start && a <= arc_end);

    let alpha = outer_alpha * inner_alpha * in_wedge;

    if alpha < 0.001 {
        discard;
    }

    return material_color * alpha;
}
