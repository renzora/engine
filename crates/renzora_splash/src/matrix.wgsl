// Matrix-style digital rain for the loading screen background. Columns of fake
// glyphs fall at varying speeds; the head of each stream is bright/white and the
// trail fades green up the column. Glyphs are faked with a 5×5 per-cell on/off
// hash that flickers over time. Opaque (green rain on black). params.x = time,
// params.y = width(px), params.z = height(px).

#import bevy_ui::ui_vertex_output::UiVertexOutput

struct MatrixUniforms {
    params: vec4<f32>,
};

@group(1) @binding(0) var<uniform> u: MatrixUniforms;

fn hash(p: vec2<f32>) -> f32 {
    return fract(sin(dot(p, vec2<f32>(127.1, 311.7))) * 43758.5453);
}

@fragment
fn fragment(in: UiVertexOutput) -> @location(0) vec4<f32> {
    let t = u.params.x;
    let res = vec2<f32>(max(u.params.y, 1.0), max(u.params.z, 1.0));
    let aspect = res.x / res.y;
    let uv = in.uv;

    let cols = 70.0;
    let rows = max(floor(cols / aspect), 1.0); // roughly square cells

    let cx = floor(uv.x * cols);
    let speed = 0.10 + hash(vec2<f32>(cx, 1.0)) * 0.45;
    let offset = hash(vec2<f32>(cx, 7.3));

    // Falling head position for this column (wraps).
    let fall = fract(offset + t * speed);
    var q = fall - uv.y;
    q = q - floor(q);                         // 0..1, small = just above the head
    let trail = exp(-q * 11.0);               // short trails (less coverage)
    let head = smoothstep(0.03, 0.0, q);      // bright head

    // Faked glyph: a 5×5 on/off pattern per cell that flickers over time. Sparse.
    let cell = vec2<f32>(cx, floor(uv.y * rows));
    let gframe = floor(t * (6.0 + hash(cell) * 8.0));
    let sub = floor(fract(vec2<f32>(uv.x * cols, uv.y * rows)) * 5.0);
    let glyph = step(0.72, hash(cell * 1.7 + sub * 9.1 + vec2<f32>(gframe, gframe)));

    let green = vec3<f32>(0.07, 0.42, 0.17);
    let white = vec3<f32>(0.30, 0.55, 0.42);
    var col = green * trail * glyph;
    col = col + white * head * glyph * 0.7;
    col = col * 0.4;                           // faint backdrop, not the main event

    return vec4<f32>(col, 1.0);
}
