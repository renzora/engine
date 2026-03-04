#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput

@group(0) @binding(0) var screen_texture: texture_2d<f32>;
@group(0) @binding(1) var texture_sampler: sampler;

struct HexPixelateSettings {
    hex_size: f32,
    _p1: f32,
    _p2: f32,
    _p3: f32,
    _p4: f32,
    _p5: f32,
    _p6: f32,
    enabled: f32,
};
@group(0) @binding(2) var<uniform> settings: HexPixelateSettings;

// Maps a UV coordinate to the nearest hexagonal grid center UV.
// Uses axial hex grid math with flat-top hexagons.
fn hex_center(uv: vec2<f32>, size: f32, tex_size: vec2<f32>) -> vec2<f32> {
    // Work in pixel space
    let px = uv * tex_size;

    // Hex grid spacing: width = size * 2, height = size * sqrt(3)
    let w = size * 2.0;
    let h = size * 1.7320508; // sqrt(3)

    // Axial grid coordinates
    let col = px.x / (w * 0.75);
    let row_offset = select(0.0, h * 0.5, (i32(floor(col)) % 2) != 0);
    let row = (px.y - row_offset) / h;

    let col_floor = floor(col);
    let row_floor = floor(row);

    // Candidate hex centers (two per column pair)
    var best_center = vec2<f32>(0.0);
    var best_dist = 1e9;

    for (var dc: i32 = 0; dc <= 1; dc++) {
        for (var dr: i32 = 0; dr <= 1; dr++) {
            let c = col_floor + f32(dc);
            let r = row_floor + f32(dr);
            let cx = c * w * 0.75;
            let row_off = select(0.0, h * 0.5, (i32(c) % 2) != 0);
            let cy = r * h + row_off;
            let candidate = vec2<f32>(cx, cy);
            let d = distance(px, candidate);
            if d < best_dist {
                best_dist = d;
                best_center = candidate;
            }
        }
    }

    return best_center / tex_size;
}

@fragment
fn fragment(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let color = textureSample(screen_texture, texture_sampler, in.uv);
    if settings.enabled < 0.5 {
        return color;
    }

    let tex_size = vec2<f32>(textureDimensions(screen_texture));
    let center_uv = hex_center(in.uv, settings.hex_size, tex_size);
    return textureSample(screen_texture, texture_sampler, center_uv);
}
