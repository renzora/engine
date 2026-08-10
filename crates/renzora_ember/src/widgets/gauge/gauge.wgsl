// Arc dial: a ring band swept from `a0` over `sweep` radians. The fill color
// spans from `pivot` to `value`, the rest is track. Used by gauges and knobs.
//
// `pivot` is 0.0 for a gauge — a level filling from the low end — and 0.5 for a
// bipolar control like a pan knob, where the meaningful thing is the distance
// either side of centre and a fill from one end reads as "mostly left" when the
// value is dead centre.

#import bevy_ui::ui_vertex_output::UiVertexOutput

struct ArcUniforms {
    track: vec4<f32>,
    fill: vec4<f32>,
    // x = value (0..1), y = start angle, z = sweep, w = thickness fraction of radius
    params: vec4<f32>,
    // x = pivot (0..1) the fill spans from; y/z/w unused
    extra: vec4<f32>,
};

@group(1) @binding(0)
var<uniform> u: ArcUniforms;

const TAU: f32 = 6.2831853;

@fragment
fn fragment(in: UiVertexOutput) -> @location(0) vec4<f32> {
    let value = clamp(u.params.x, 0.0, 1.0);
    let a0 = u.params.y;
    let sweep = u.params.z;
    let thick_frac = u.params.w;

    let c = in.size * 0.5;
    let p = in.uv * in.size - c;
    let r = length(p);
    let radius = min(in.size.x, in.size.y) * 0.5 - 1.5;
    let thick = thick_frac * radius;
    let mid = radius - thick * 0.5;

    let ring = abs(r - mid);
    let aa = max(fwidth(r), 0.75);
    let band = 1.0 - smoothstep(thick * 0.5 - aa, thick * 0.5 + aa, ring);

    var rel = atan2(p.y, p.x) - a0;
    rel = rel - floor(rel / TAU) * TAU;
    let t = rel / sweep;
    let on_arc = step(0.0, t) * step(t, 1.0);

    let alpha = band * on_arc;
    if (alpha <= 0.0) {
        discard;
    }
    let pivot = clamp(u.extra.x, 0.0, 1.0);
    let lo = min(pivot, value);
    let hi = max(pivot, value);
    let filled = t >= lo && t <= hi;
    let rgb = select(u.track.rgb, u.fill.rgb, filled);
    return vec4<f32>(rgb, alpha);
}
