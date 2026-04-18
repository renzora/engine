#import bevy_ui::ui_vertex_output::UiVertexOutput

@group(1) @binding(0) var<uniform> material_color: vec4<f32>;
// x = thickness (pixels), y = angle (radians)
@group(1) @binding(1) var<uniform> material_params: vec4<f32>;

@fragment
fn fragment(in: UiVertexOutput) -> @location(0) vec4<f32> {
    let thickness = material_params.x;
    let angle = material_params.y;

    // Center UV at origin, scale to pixel space
    let center = in.uv - 0.5;
    let px = center * in.size;

    // Rotate point by -angle to align line with x-axis
    let ca = cos(-angle);
    let sa = sin(-angle);
    let rotated = vec2<f32>(
        px.x * ca - px.y * sa,
        px.x * sa + px.y * ca,
    );

    // Distance from x-axis (the line) in rotated space
    let dist = abs(rotated.y);
    let half_thickness = thickness * 0.5;
    let aa = 1.0;

    let alpha = 1.0 - smoothstep(half_thickness - aa, half_thickness + aa, dist);

    if alpha < 0.001 {
        discard;
    }

    return material_color * alpha;
}
