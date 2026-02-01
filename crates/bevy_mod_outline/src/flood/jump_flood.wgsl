#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput

struct JumpFloodUniform {
    @align(16)
    step_length: u32,
}

@group(0) @binding(0) var screen_texture: texture_2d<f32>;
@group(0) @binding(1) var texture_sampler: sampler;
@group(0) @binding(2) var<uniform> instance: JumpFloodUniform;

@fragment
fn fragment(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let dims = vec2<f32>(textureDimensions(screen_texture));
    let step = i32(instance.step_length);

    let current = textureSample(screen_texture, texture_sampler, in.uv);
    var closest_dist = distance(vec2<f32>(in.position.xy), vec2<f32>(current.xy));
    var result = current;

    // Check all 8 neighbouring pixels at current step distance
    for (var dy = -1; dy <= 1; dy++) {
        for (var dx = -1; dx <= 1; dx++) {
            if (dx == 0 && dy == 0) {
                continue;
            }

            let neighbour_coord = in.position.xy + vec2<f32>(f32(dx * step), f32(dy * step));
            let neighbour = textureSample(screen_texture, texture_sampler, neighbour_coord / dims);
            let dist = distance(vec2<f32>(in.position.xy), vec2<f32>(neighbour.xy));

            if (dist < closest_dist) {
                closest_dist = dist;
                result = neighbour;
            }
        }
    }
    return result;
}
