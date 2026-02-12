#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput

@group(0) @binding(0) var screen_texture: texture_2d<f32>;
@group(0) @binding(1) var texture_sampler: sampler;

struct UnderwaterSettings {
    distortion: f32,
    tint_r: f32,
    tint_g: f32,
    tint_b: f32,
    tint_strength: f32,
    wave_speed: f32,
    wave_scale: f32,
    time: f32,
};
@group(0) @binding(2) var<uniform> settings: UnderwaterSettings;

@fragment
fn fragment(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let t = settings.time * settings.wave_speed;
    let offset = vec2(
        sin(in.uv.y * settings.wave_scale * 10.0 + t) * settings.distortion,
        cos(in.uv.x * settings.wave_scale * 8.0 + t * 1.3) * settings.distortion * 0.7
    );
    let uv = clamp(in.uv + offset, vec2(0.0), vec2(1.0));
    var color = textureSample(screen_texture, texture_sampler, uv);

    // Apply tint
    let tint = vec3(settings.tint_r, settings.tint_g, settings.tint_b);
    color = vec4(mix(color.rgb, color.rgb * tint, settings.tint_strength), color.a);

    return color;
}
