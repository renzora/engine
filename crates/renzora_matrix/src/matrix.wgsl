#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput

@group(0) @binding(0) var screen_texture: texture_2d<f32>;
@group(0) @binding(1) var texture_sampler: sampler;

struct MatrixSettings {
    speed: f32,
    density: f32,
    glow: f32,
    trail_length: f32,
    color_r: f32,
    color_g: f32,
    time: f32,
    enabled: f32,
};
@group(0) @binding(2) var<uniform> settings: MatrixSettings;

// Simple hash function for pseudo-random values
fn hash11(p: f32) -> f32 {
    var x = fract(p * 0.1031);
    x *= x + 33.33;
    x *= x + x;
    return fract(x);
}

fn hash21(p: vec2<f32>) -> f32 {
    var p3 = fract(vec3(p.xyx) * 0.1031);
    p3 += dot(p3, p3.yzx + 33.33);
    return fract((p3.x + p3.y) * p3.z);
}

@fragment
fn fragment(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let color = textureSample(screen_texture, texture_sampler, in.uv);
    if settings.enabled < 0.5 {
        return color;
    }

    let uv = in.uv;

    // Scale UV by density to create column grid
    let scaled = uv * vec2(settings.density, settings.density * 2.0);
    let col = floor(scaled.x);
    let row = scaled.y;

    // Each column has a random speed offset and phase
    let col_rand = hash11(col);
    let col_speed = settings.speed * (0.5 + col_rand * 1.5);

    // Animated vertical position of the rain head in this column
    let t = settings.time * col_speed + col_rand * 20.0;

    // The rain head falls downward; wrap with fract
    // rain_pos is in [0,1] representing where the head is within the column height
    let head_pos = fract(t * 0.1);

    // Distance from current pixel (in row units) to the rain head
    // row is in [0, density*2] range; normalize to [0,1]
    let row_norm = fract(row * 0.5); // normalize row to column-relative [0,1]

    // Compute distance from pixel to head, accounting for wrap
    var dist_to_head = head_pos - fract(scaled.y / (settings.density * 2.0));
    if dist_to_head < 0.0 {
        dist_to_head += 1.0;
    }

    // Trail: bright near the head, fading with trail_length
    // dist_to_head == 0 means at the head; increases going "above" (older)
    let trail_fade = 1.0 - dist_to_head / max(settings.trail_length, 0.01);
    let trail_intensity = clamp(trail_fade, 0.0, 1.0);
    // Quadratic falloff for more organic look
    let intensity = trail_intensity * trail_intensity;

    // Randomize which columns are active (not all columns show rain)
    let is_active = step(0.3, hash11(col + 7.77));

    // Character flicker: random brightness per cell
    let cell_row = floor(scaled.y);
    let char_rand = hash21(vec2(col, cell_row + floor(t * 10.0)));
    let char_brightness = 0.6 + 0.4 * char_rand;

    // Head glow: extra brightness at the head position
    let head_glow = exp(-dist_to_head * 30.0) * 2.0;

    let rain_value = is_active * intensity * char_brightness + is_active * head_glow;

    // Rain color: green by default (color_r, color_g tint)
    // color_r/color_g are extra tint multipliers (0 = pure green, etc.)
    let rain_color = vec3(
        settings.color_r * rain_value,
        settings.color_g * rain_value,
        0.0
    );

    // Mix rain over scene based on glow factor
    // glow=0: rain barely visible, glow=1: strong overlay
    let scene = color.rgb;
    let result = mix(scene, scene * 0.3 + rain_color, settings.glow * clamp(rain_value, 0.0, 1.0));

    return vec4(result, color.a);
}
