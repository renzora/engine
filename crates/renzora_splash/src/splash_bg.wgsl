// Splash animated background: a synthwave perspective grid + drifting starfield
// over a vertical gradient, with a hue-cycled grid colour and a bottom vignette.
// A bevy_ui-space port of the egui `draw_background` painter art (the 3D
// wireframes and O(n^2) constellation links don't map to a fragment shader, so
// the grid + stars carry the look). Output is sRGB-encoded for the UI pass.

#import bevy_ui::ui_vertex_output::UiVertexOutput

struct BgUniforms {
    params: vec4<f32>, // x = time (s), y = aspect (w/h)
};

@group(1) @binding(0)
var<uniform> u: BgUniforms;

fn hash21(p: vec2<f32>) -> f32 {
    var p3 = fract(vec3<f32>(p.x, p.y, p.x) * 0.1031);
    p3 = p3 + dot(p3, p3.yzx + 33.33);
    return fract((p3.x + p3.y) * p3.z);
}

fn hsv2rgb(h: f32, s: f32, v: f32) -> vec3<f32> {
    let k = vec3<f32>(5.0, 3.0, 1.0);
    let p = abs(fract(vec3<f32>(h, h, h) + k / 6.0) * 6.0 - 3.0);
    return v * mix(vec3<f32>(1.0, 1.0, 1.0), clamp(p - 1.0, vec3<f32>(0.0), vec3<f32>(1.0)), s);
}

@fragment
fn fragment(in: UiVertexOutput) -> @location(0) vec4<f32> {
    let uv = in.uv;                       // 0..1, y down
    let t = u.params.x;
    let aspect = max(u.params.y, 0.0001);

    // Base vertical gradient (sRGB space).
    let top = vec3<f32>(8.0, 7.0, 16.0) / 255.0;
    let bot = vec3<f32>(3.0, 2.0, 8.0) / 255.0;
    var col = mix(top, bot, uv.y);

    // Hue-cycled grid colour (slow, so the scene reads as alive but not busy).
    let hue = fract(t * 0.05);
    let grid_col = hsv2rgb(hue, 0.5, 1.0) * 0.5;

    let horizon = 0.45;

    // ── Floor grid (below the horizon) ──
    if (uv.y > horizon) {
        let fy = (uv.y - horizon) / (1.0 - horizon);     // 0 at horizon → 1 at bottom
        let depth = 1.0 / max(1.0 - fy, 0.02);           // large near the horizon

        // Horizontal lines scrolling toward the viewer.
        let hl = fract(depth * 0.6 - t * 0.6);
        let hline = 1.0 - smoothstep(0.0, 0.06, min(hl, 1.0 - hl));

        // Vertical lines fanning out with perspective.
        let px = (uv.x - 0.5) * aspect * depth;
        let vl = fract(px * 1.2);
        let vw = clamp(0.04 * depth, 0.02, 0.4);
        let vline = 1.0 - smoothstep(0.0, vw, min(vl, 1.0 - vl));

        let g = clamp(hline + vline, 0.0, 1.0) * clamp(fy * 1.5, 0.0, 1.0);
        col = col + grid_col * g * 0.6;
    }

    // ── Drifting starfield ──
    let cells = vec2<f32>(28.0 * aspect, 28.0);
    let gp = uv * cells + vec2<f32>(t * 0.4, 0.0);
    let cell = floor(gp);
    let f = fract(gp);
    let rnd = hash21(cell);
    if (rnd > 0.86) {
        let center = vec2<f32>(hash21(cell + 1.7), hash21(cell + 4.3));
        let d = length(f - center);
        let twinkle = 0.6 + 0.4 * sin(t * 2.0 + rnd * 40.0);
        let star = (1.0 - smoothstep(0.0, 0.08, d)) * twinkle;
        col = col + vec3<f32>(0.82, 0.86, 0.96) * star * 0.85;
    }

    // ── Bottom vignette for bottom-bar legibility ──
    let vig = smoothstep(0.82, 1.0, uv.y);
    col = mix(col, vec3<f32>(0.02, 0.024, 0.047), vig * 0.6);

    return vec4<f32>(pow(col, vec3<f32>(2.2)), 1.0);
}
