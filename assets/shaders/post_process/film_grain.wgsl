#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput

@group(0) @binding(0) var screen_texture: texture_2d<f32>;
@group(0) @binding(1) var texture_sampler: sampler;

struct FilmGrainSettings {
    intensity: f32,
    grain_size: f32,
    time: f32,
    _padding0: f32,
    _padding1: f32,
    _padding2: f32,
    _padding3: f32,
    _padding4: f32,
};
@group(0) @binding(2) var<uniform> settings: FilmGrainSettings;

fn hash(p: vec2<f32>) -> f32 {
    let p2 = fract(p * vec2(443.897, 441.423));
    let p3 = dot(p2, p2 + vec2(19.19));
    return fract(p3 * p3);
}

@fragment
fn fragment(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let color = textureSample(screen_texture, texture_sampler, in.uv);
    let grain = hash(in.uv * settings.grain_size * 1000.0 + settings.time) * 2.0 - 1.0;
    return vec4(color.rgb + vec3(grain * settings.intensity), color.a);
}
