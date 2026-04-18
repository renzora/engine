#import bevy_ui::ui_vertex_output::UiVertexOutput

@group(1) @binding(0) var<uniform> material_color: vec4<f32>;
// x = start_angle, y = end_angle, z = thickness (0-1)
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
    let thickness = material_params.z;

    let outer = 1.0;
    let inner = 1.0 - thickness;

    // Ring SDF
    let ring_alpha = (1.0 - smoothstep(outer - aa, outer, dist)) *
                     smoothstep(inner - aa, inner, dist);

    // Angle mask
    var angle = atan2(uv.y, uv.x); // -PI..PI
    // Normalize start/end to handle wrapping
    var arc_start = start;
    var arc_end = end;
    // Ensure arc_end > arc_start
    if arc_end < arc_start {
        arc_end += TAU;
    }

    // Normalize angle to be >= arc_start
    if angle < arc_start - PI {
        angle += TAU;
    }
    if angle < arc_start {
        angle += TAU;
    }

    let in_arc = select(0.0, 1.0, angle >= arc_start && angle <= arc_end);

    let alpha = ring_alpha * in_arc;

    if alpha < 0.001 {
        discard;
    }

    return material_color * alpha;
}
