// @effect(name = "Toon", icon = "paint-brush")
//
// Cel shading: luminance is quantized into bands and an edge is drawn over
// the result.
//
// It is Posterize and Outline in one pass rather than two, which is the
// point: doing it together keeps the edge aligned to the bands it is
// outlining.

@group(0) @binding(0) var screen_texture: texture_2d<f32>;
@group(0) @binding(1) var texture_sampler: sampler;

struct ToonSettings {
    levels: f32,            // @slider(2, 16, default = 4.0, step = 0.1)
    edge_threshold: f32,    // @slider(0, 1, default = 0.1, step = 0.005)
    edge_thickness: f32,    // @slider(0.5, 5, default = 1.0, step = 0.05)
    saturation_boost: f32,  // @slider(0, 3, default = 1.2, step = 0.02)
};
@group(0) @binding(2) var<uniform> settings: ToonSettings;

fn luminance(c: vec3<f32>) -> f32 {
    return dot(c, vec3(0.2126, 0.7152, 0.0722));
}

// Boost saturation: lerp between grayscale and color
fn boost_saturation(c: vec3<f32>, factor: f32) -> vec3<f32> {
    let lum = luminance(c);
    return mix(vec3(lum), c, factor);
}

// Quantize a value into N discrete levels
fn quantize(v: f32, levels: f32) -> f32 {
    return floor(v * levels) / levels;
}

@fragment
fn fragment(@builtin(position) pos: vec4<f32>, @location(0) in_uv: vec2<f32>) -> @location(0) vec4<f32> {
    let color = textureSample(screen_texture, texture_sampler, in_uv);

    // --- Cel shading: quantize color channels ---
    let levels = max(settings.levels, 2.0);
    var cel = vec3(
        quantize(color.r, levels),
        quantize(color.g, levels),
        quantize(color.b, levels),
    );

    // Boost saturation on the quantized color
    cel = boost_saturation(cel, settings.saturation_boost);

    // --- Sobel edge detection on luminance ---
    let tex_size = vec2<f32>(textureDimensions(screen_texture));
    let texel = settings.edge_thickness / tex_size;

    let tl = luminance(textureSample(screen_texture, texture_sampler, in_uv + vec2(-texel.x,  texel.y)).rgb);
    let tc = luminance(textureSample(screen_texture, texture_sampler, in_uv + vec2( 0.0,      texel.y)).rgb);
    let tr = luminance(textureSample(screen_texture, texture_sampler, in_uv + vec2( texel.x,  texel.y)).rgb);
    let ml = luminance(textureSample(screen_texture, texture_sampler, in_uv + vec2(-texel.x,  0.0    )).rgb);
    let mr = luminance(textureSample(screen_texture, texture_sampler, in_uv + vec2( texel.x,  0.0    )).rgb);
    let bl = luminance(textureSample(screen_texture, texture_sampler, in_uv + vec2(-texel.x, -texel.y)).rgb);
    let bc = luminance(textureSample(screen_texture, texture_sampler, in_uv + vec2( 0.0,     -texel.y)).rgb);
    let br = luminance(textureSample(screen_texture, texture_sampler, in_uv + vec2( texel.x, -texel.y)).rgb);

    let gx = (-1.0 * tl) + (1.0 * tr)
           + (-2.0 * ml) + (2.0 * mr)
           + (-1.0 * bl) + (1.0 * br);

    let gy = ( 1.0 * tl) + (2.0 * tc) + ( 1.0 * tr)
           + (-1.0 * bl) + (-2.0 * bc) + (-1.0 * br);

    let edge_magnitude = sqrt(gx * gx + gy * gy);
    let is_edge = step(settings.edge_threshold, edge_magnitude);

    // Overlay black edges on top of cel-shaded color
    let result = mix(cel, vec3(0.0), is_edge);

    return vec4(result, color.a);
}
