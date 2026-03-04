#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput

@group(0) @binding(0) var screen_texture: texture_2d<f32>;
@group(0) @binding(1) var texture_sampler: sampler;

struct SharpenSettings {
    strength: f32,
    _padding1: f32,
    _padding2: f32,
    _padding3: f32,
    _padding4: f32,
    _padding5: f32,
    _padding6: f32,
    enabled: f32,
};
@group(0) @binding(2) var<uniform> settings: SharpenSettings;

@fragment
fn fragment(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let color = textureSample(screen_texture, texture_sampler, in.uv);
    if settings.enabled < 0.5 {
        return color;
    }

    let tex_size = vec2<f32>(textureDimensions(screen_texture));
    let texel = 1.0 / tex_size;

    let top    = textureSample(screen_texture, texture_sampler, in.uv + vec2( 0.0, -texel.y)).rgb;
    let bottom = textureSample(screen_texture, texture_sampler, in.uv + vec2( 0.0,  texel.y)).rgb;
    let left   = textureSample(screen_texture, texture_sampler, in.uv + vec2(-texel.x,  0.0)).rgb;
    let right  = textureSample(screen_texture, texture_sampler, in.uv + vec2( texel.x,  0.0)).rgb;

    let sharpened = color.rgb * (1.0 + 4.0 * settings.strength) - (top + bottom + left + right) * settings.strength;
    return vec4(clamp(sharpened, vec3(0.0), vec3(1.0)), color.a);
}
