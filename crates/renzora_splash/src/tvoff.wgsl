// CRT "turn-off" overlay for the splash → loading transition. A fullscreen node
// that, while active, collapses the screen the way an old TV powers down: the
// image squashes vertically into a bright horizontal line, the line collapses
// horizontally into a centre dot, then the dot fades to black. params.x =
// progress 0..1, params.y = active (0 = idle → fully transparent).

#import bevy_ui::ui_vertex_output::UiVertexOutput

struct TvOffUniforms {
    params: vec4<f32>,
};

@group(1) @binding(0) var<uniform> u: TvOffUniforms;

@fragment
fn fragment(in: UiVertexOutput) -> @location(0) vec4<f32> {
    if (u.params.y < 0.5) {
        return vec4<f32>(0.0); // idle: invisible, lets the splash through
    }

    let uv = in.uv;
    let p = clamp(u.params.x, 0.0, 1.0);
    let v_scale = 1.0 - smoothstep(0.0, 0.5, p);   // vertical collapse (0..0.5)
    let h_scale = 1.0 - smoothstep(0.5, 0.92, p);  // horizontal collapse (0.5..0.92)
    let dy = abs(uv.y - 0.5);
    let dx = abs(uv.x - 0.5);

    // Everything outside the shrinking band is black (hides the splash).
    var col = vec3<f32>(0.0);
    var a = 1.0;

    // Inside the band the splash shows, washing to white as it squeezes shut.
    if (dy < 0.5 * v_scale && dx < 0.5 * h_scale) {
        col = vec3<f32>(1.0);
        a = (1.0 - v_scale) * 0.9;
    }

    // The bright collapsing scan line (only once it's thinned down).
    let line_h = 0.0015 + 0.0015 * (1.0 - v_scale);
    if (v_scale < 0.35 && dy < line_h && dx < 0.5 * h_scale) {
        col = vec3<f32>(1.0);
        a = 1.0;
    }

    // The final centre dot, fading out.
    let dot_fade = 1.0 - smoothstep(0.9, 1.0, p);
    if (p > 0.8 && length(vec2<f32>(dx, dy)) < 0.012 * dot_fade) {
        col = vec3<f32>(1.0);
        a = 1.0;
    }

    return vec4<f32>(col, a);
}
