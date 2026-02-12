#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput

@group(0) @binding(0) var screen_texture: texture_2d<f32>;
@group(0) @binding(1) var texture_sampler: sampler;

struct PaletteQuantizationSettings {
    num_colors: u32,
    dithering: f32,
    _padding0: f32,
    _padding1: f32,
    _padding2: f32,
    _padding3: f32,
    _padding4: f32,
    _padding5: f32,
};
@group(0) @binding(2) var<uniform> settings: PaletteQuantizationSettings;

fn bayer4(pos: vec2<f32>) -> f32 {
    let x = u32(pos.x) % 4u;
    let y = u32(pos.y) % 4u;
    let bayer = array<f32, 16>(
         0.0/16.0,  8.0/16.0,  2.0/16.0, 10.0/16.0,
        12.0/16.0,  4.0/16.0, 14.0/16.0,  6.0/16.0,
         3.0/16.0, 11.0/16.0,  1.0/16.0,  9.0/16.0,
        15.0/16.0,  7.0/16.0, 13.0/16.0,  5.0/16.0
    );
    return bayer[y * 4u + x];
}

@fragment
fn fragment(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let color = textureSample(screen_texture, texture_sampler, in.uv);
    let dims = vec2<f32>(textureDimensions(screen_texture));
    let levels = max(f32(settings.num_colors), 2.0);

    // Ordered dithering
    let dither = (bayer4(in.uv * dims) - 0.5) * settings.dithering / levels;
    let quantized = floor((color.rgb + dither) * levels) / levels;

    return vec4(clamp(quantized, vec3(0.0), vec3(1.0)), color.a);
}
