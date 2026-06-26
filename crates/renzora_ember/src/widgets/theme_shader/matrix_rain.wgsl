// Built-in themeable-UI effect: "matrix rain". This is also the contract example
// a theme's own `shaders/*.wgsl` follows — bind group at @group(1) @binding(0)
// laid out exactly as `ThemeUniforms`, entry point `fragment`, output decoded to
// linear (`pow(c, 2.2)`) for the sRGB UI pass.
//
// Per-column falling streams: the trail uses the theme accent and the leading
// cell the primary text color, over the surface background. The node's pixel size
// arrives as `in.size`, so no resolution uniform is needed.

#import bevy_ui::ui_vertex_output::UiVertexOutput

struct ThemeUniforms {
    params: vec4<f32>, // x = time (s), y/z/w reserved
    bg: vec4<f32>,
    accent: vec4<f32>,
    fg: vec4<f32>,
};

@group(1) @binding(0)
var<uniform> u: ThemeUniforms;

fn hash11(n: f32) -> f32 {
    return fract(sin(n) * 43758.5453123);
}

fn matrix_rain(uv: vec2<f32>, size: vec2<f32>, t: f32) -> vec3<f32> {
    // Fine cell grid in physical px. Small cells give the short top bar enough
    // vertical rows for a readable trail gradient instead of solid columns.
    let cell = vec2<f32>(8.0, 9.0);
    let p = uv * size;
    let col = floor(p.x / cell.x);
    let row = floor(p.y / cell.y);
    let rows = max(floor(size.y / cell.y), 1.0);

    // Sparse: only some columns rain at once, so it reads as ambient flecks
    // rather than a wall of green.
    let col_active = step(0.55, hash11(col * 12.9));

    // Slow fall (rows/sec) with long gaps between drops, so the bar is mostly
    // calm with the odd streak drifting down.
    let speed = 0.8 + hash11(col) * 1.6;
    let span = rows + 22.0;
    let offset = hash11(col * 1.73) * span;
    let head = (t * speed + offset) % span;

    // Distance of this cell behind the head, in rows (>= 0 means lit trail).
    let d = head - row;
    // Short trail (~3 cells) so streaks stay small.
    let trail = clamp(1.0 - d / 3.0, 0.0, 1.0) * step(0.0, d);

    // Slow per-cell flicker (~6 fps) so cells read as changing glyphs.
    let glyph_seed = col * 31.3 + row * 7.1 + floor(t * 6.0) * 0.137;
    let lit = step(0.30, hash11(glyph_seed));

    let intensity = trail * lit * col_active;
    let head_glow = clamp(1.0 - d, 0.0, 1.0) * lit * col_active;

    // Dim: keep the streaks close to the background so light UI text on top stays
    // legible. Accent (not fg) even at the head, so nothing competes with text.
    var color = u.bg.rgb;
    color = mix(color, u.accent.rgb, intensity * 0.28);
    color = mix(color, u.accent.rgb, head_glow * 0.22);
    return color;
}

@fragment
fn fragment(in: UiVertexOutput) -> @location(0) vec4<f32> {
    let color = matrix_rain(in.uv, in.size, u.params.x);
    return vec4<f32>(pow(color, vec3<f32>(2.2)), 1.0);
}
