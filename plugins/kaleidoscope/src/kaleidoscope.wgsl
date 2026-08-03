@group(0) @binding(0) var screen_texture: texture_2d<f32>;
@group(0) @binding(1) var texture_sampler: sampler;

struct KaleidoscopeSettings {
    segments: f32,
    rotation: f32,
    center_x: f32,
    center_y: f32,
};
@group(0) @binding(2) var<uniform> settings: KaleidoscopeSettings;

const PI: f32 = 3.14159265;

@fragment
fn fragment(@builtin(position) pos: vec4<f32>, @location(0) in_uv: vec2<f32>) -> @location(0) vec4<f32> {
    let color = textureSample(screen_texture, texture_sampler, in_uv);

    let center = vec2(settings.center_x, settings.center_y);
    let uv = in_uv - center;
    var angle = atan2(uv.y, uv.x) + settings.rotation;
    let radius = length(uv);
    let segment_angle = 2.0 * PI / settings.segments;
    angle = abs(((angle % segment_angle) + segment_angle) % segment_angle - segment_angle * 0.5);
    let new_uv = vec2(cos(angle), sin(angle)) * radius + center;
    let clamped = clamp(new_uv, vec2(0.0), vec2(1.0));
    return textureSample(screen_texture, texture_sampler, clamped);
}
