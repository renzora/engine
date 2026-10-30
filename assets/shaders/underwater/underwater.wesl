// @effect(name = "Underwater", icon = "wave-triangle")
//
// Submerged look: a blue-green tint, a rolling refraction, and caustic light
// playing over the frame. Switch it on when the camera goes below the
// surface.

@group(0) @binding(0) var screen_texture: texture_2d<f32>;
@group(0) @binding(1) var texture_sampler: sampler;

struct UnderwaterSettings {
    distortion: f32,     // @slider(0, 2, default = 0.02, step = 0.01)
    tint_r: f32,         // @hidden(default = 0.0)
    tint_g: f32,         // @hidden(default = 0.3)
    tint_b: f32,         // @hidden(default = 0.5)
    tint_strength: f32,  // @slider(0, 1, default = 0.3, step = 0.01)
    wave_speed: f32,     // @slider(0, 10, default = 1.0, step = 0.01)
    wave_scale: f32,     // @slider(0.1, 50, default = 10.0, step = 0.1)
    time: f32,
};
@group(0) @binding(2) var<uniform> settings: UnderwaterSettings;

@fragment
fn fragment(@builtin(position) pos: vec4<f32>, @location(0) in_uv: vec2<f32>) -> @location(0) vec4<f32> {
    let color = textureSample(screen_texture, texture_sampler, in_uv);
    let t = settings.time * settings.wave_speed;
    let offset = vec2(
        sin(in_uv.y * settings.wave_scale * 10.0 + t) * settings.distortion,
        cos(in_uv.x * settings.wave_scale * 8.0 + t * 1.3) * settings.distortion * 0.7
    );
    let uv = clamp(in_uv + offset, vec2(0.0), vec2(1.0));
    var result = textureSample(screen_texture, texture_sampler, uv);

    // Apply tint
    let tint = vec3(settings.tint_r, settings.tint_g, settings.tint_b);
    result = vec4(mix(result.rgb, result.rgb * tint, settings.tint_strength), result.a);

    return result;
}
