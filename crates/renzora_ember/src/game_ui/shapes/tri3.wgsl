// Filled arbitrary triangle via a signed-distance field (Inigo Quilez's triangle
// SDF). Three vertices come in as node-local pixels; the SDF is negative inside,
// so a 1px smoothstep gives a clean anti-aliased edge at any triangle shape.

#import bevy_ui::ui_vertex_output::UiVertexOutput

@group(1) @binding(0) var<uniform> material_color: vec4<f32>;
@group(1) @binding(1) var<uniform> pts_ab: vec4<f32>;  // a.xy, b.xy
@group(1) @binding(2) var<uniform> pts_c: vec4<f32>;   // c.xy

fn sd_triangle(p: vec2<f32>, p0: vec2<f32>, p1: vec2<f32>, p2: vec2<f32>) -> f32 {
    let e0 = p1 - p0;
    let e1 = p2 - p1;
    let e2 = p0 - p2;
    let v0 = p - p0;
    let v1 = p - p1;
    let v2 = p - p2;
    let pq0 = v0 - e0 * clamp(dot(v0, e0) / dot(e0, e0), 0.0, 1.0);
    let pq1 = v1 - e1 * clamp(dot(v1, e1) / dot(e1, e1), 0.0, 1.0);
    let pq2 = v2 - e2 * clamp(dot(v2, e2) / dot(e2, e2), 0.0, 1.0);
    let s = sign(e0.x * e2.y - e0.y * e2.x);
    let d = min(
        min(
            vec2<f32>(dot(pq0, pq0), s * (v0.x * e0.y - v0.y * e0.x)),
            vec2<f32>(dot(pq1, pq1), s * (v1.x * e1.y - v1.y * e1.x)),
        ),
        vec2<f32>(dot(pq2, pq2), s * (v2.x * e2.y - v2.y * e2.x)),
    );
    return -sqrt(d.x) * sign(d.y);
}

@fragment
fn fragment(in: UiVertexOutput) -> @location(0) vec4<f32> {
    let p = in.uv * in.size;
    let d = sd_triangle(p, pts_ab.xy, pts_ab.zw, pts_c.xy);
    let alpha = 1.0 - smoothstep(-1.0, 1.0, d);
    if alpha < 0.001 {
        discard;
    }
    return material_color * alpha;
}
