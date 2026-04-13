#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput

@group(0) @binding(0) var screen_texture: texture_2d<f32>;
@group(0) @binding(1) var texture_sampler: sampler;

struct MosaicSettings {
    tile_size: f32,
    edge_thickness: f32,
    roundness: f32,
    _padding0: f32,
    _padding1: f32,
    _padding2: f32,
    _padding3: f32,
    enabled: f32,
};
@group(0) @binding(2) var<uniform> settings: MosaicSettings;

@fragment
fn fragment(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let color = textureSample(screen_texture, texture_sampler, in.uv);
    if settings.enabled < 0.5 {
        return color;
    }

    let dims = vec2<f32>(textureDimensions(screen_texture));
    let tile = vec2(settings.tile_size) / dims;
    let tile_center = (floor(in.uv / tile) + 0.5) * tile;
    let tile_color = textureSample(screen_texture, texture_sampler, tile_center);

    // Distance from center of tile for edge/roundness effect
    let local = abs(in.uv - tile_center) / tile;
    let dist = length(max(local - vec2(0.5 - settings.roundness), vec2(0.0)));
    let edge = smoothstep(0.5 - settings.edge_thickness, 0.5, max(local.x, local.y));

    let result = mix(tile_color.rgb, vec3(0.1), edge);
    return vec4(result, color.a);
}
