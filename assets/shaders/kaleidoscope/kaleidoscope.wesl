// @effect(name = "Kaleidoscope", icon = "flower-lotus")
//
// Mirrors one wedge of the frame around the centre, so the picture repeats
// into a kaleidoscope. `segments` is how many wedges, `rotation` spins the
// whole pattern.
//
// The centre is fixed at the middle of the screen, which is what a
// kaleidoscope wants. Swirl and Radial Blur are the effects in this set with
// a movable centre.

@group(0) @binding(0) var screen_texture: texture_2d<f32>;
@group(0) @binding(1) var texture_sampler: sampler;

struct KaleidoscopeSettings {
    segments: f32,  // @slider(2, 32, default = 6, step = 0.1)
    rotation: f32,  // @slider(0, 6.283, default = 0, step = 0.01)
    center_x: f32,  // @hidden(default = 0.5)
    center_y: f32,  // @hidden(default = 0.5)
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
