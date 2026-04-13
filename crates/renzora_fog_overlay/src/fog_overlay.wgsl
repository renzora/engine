#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput

@group(0) @binding(0) var screen_texture: texture_2d<f32>;
@group(0) @binding(1) var texture_sampler: sampler;

struct FogOverlaySettings {
    density: f32,
    height: f32,
    color_r: f32,
    color_g: f32,
    color_b: f32,
    _padding1: f32,
    _padding2: f32,
    enabled: f32,
};
@group(0) @binding(2) var<uniform> settings: FogOverlaySettings;

@fragment
fn fragment(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let color = textureSample(screen_texture, texture_sampler, in.uv);
    if settings.enabled < 0.5 {
        return color;
    }

    let fog_color = vec3(settings.color_r, settings.color_g, settings.color_b);
    // Fog increases toward bottom of screen (simulating ground fog)
    let fog_amount = smoothstep(1.0 - settings.height, 1.0, in.uv.y) * settings.density;
    let result = mix(color.rgb, fog_color, fog_amount);
    return vec4(result, color.a);
}
