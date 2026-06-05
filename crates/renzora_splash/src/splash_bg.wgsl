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

// A constellation node: one drifting point per grid cell (grid-space coords).
fn cell_point(cell: vec2<f32>, t: f32) -> vec2<f32> {
    let h1 = hash21(cell);
    let h2 = hash21(cell + 7.3);
    return cell + vec2<f32>(0.5, 0.5)
        + 0.4 * vec2<f32>(sin(t * 0.4 + h1 * 6.2832), cos(t * 0.4 + h2 * 6.2832));
}

// Distance from point `p` to segment `a`–`b`.
fn seg_dist(p: vec2<f32>, a: vec2<f32>, b: vec2<f32>) -> f32 {
    let pa = p - a;
    let ba = b - a;
    let h = clamp(dot(pa, ba) / max(dot(ba, ba), 1e-5), 0.0, 1.0);
    return length(pa - ba * h);
}

// Whether a constellation cell carries a node (sparse, so it reads as scattered).
fn node_exists(cell: vec2<f32>) -> bool {
    return hash21(cell + 11.0) > 0.6;
}

// Antialiased grid line at integer values of `coord` (~1px wide via screen-space
// derivatives), so perspective-warped lines stay thin and fade at the horizon.
fn grid_line(coord: f32) -> f32 {
    let d = max(fwidth(coord), 1e-5);
    let g = abs(fract(coord - 0.5) - 0.5) / d;
    return 1.0 - min(g, 1.0);
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

    // ── Floor grid (below the horizon, receding upward into the distance) ──
    if (uv.y > horizon) {
        let fy = (uv.y - horizon) / (1.0 - horizon);     // 0 at horizon → 1 at the viewer (bottom)
        let depth = 1.0 / max(fy, 0.02);                 // far (large) at the horizon, near (~1) at the viewer

        // Horizontal lines scroll toward the viewer; verticals converge to the horizon.
        let hline = grid_line(depth * 0.5 + t * 0.6);
        let vline = grid_line((uv.x - 0.5) * aspect * depth * 2.5);

        // Fade in toward the viewer (the original grid is subtle, not a bright band).
        let fade = clamp(fy * 1.3, 0.0, 1.0) * 0.4;
        col = col + grid_col * max(hline, vline) * fade;
    }

    // ── Constellation: sparse drifting nodes with faint links to neighbours ──
    let net = vec2<f32>(11.0 * aspect, 11.0);
    let np = uv * net;
    let nc = floor(np);
    if (node_exists(nc)) {
        let center_pt = cell_point(nc, t);
        var link = 0.0;
        for (var dy = -1; dy <= 1; dy = dy + 1) {
            for (var dx = -1; dx <= 1; dx = dx + 1) {
                if (dx == 0 && dy == 0) {
                    continue;
                }
                let ncell = nc + vec2<f32>(f32(dx), f32(dy));
                if (node_exists(ncell)) {
                    let d = seg_dist(np, center_pt, cell_point(ncell, t));
                    link = max(link, 1.0 - smoothstep(0.0, 0.022, d));
                }
            }
        }
        col = col + vec3<f32>(0.5, 0.58, 0.82) * link * 0.10;
        let twinkle = 0.7 + 0.3 * sin(t * 2.0 + hash21(nc) * 40.0);
        let pglow = 1.0 - smoothstep(0.0, 0.05, length(np - center_pt));
        col = col + vec3<f32>(0.78, 0.83, 0.95) * pglow * twinkle * 0.5;
    }

    // ── Bottom vignette for bottom-bar legibility ──
    let vig = smoothstep(0.82, 1.0, uv.y);
    col = mix(col, vec3<f32>(0.02, 0.024, 0.047), vig * 0.6);

    return vec4<f32>(pow(col, vec3<f32>(2.2)), 1.0);
}
