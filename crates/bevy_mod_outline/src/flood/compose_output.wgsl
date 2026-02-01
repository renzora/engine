#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput
#import bevy_mod_outline::common::OutlineViewUniform

struct ComposeOutputUniform {
    @align(16)
    volume_offset: f32,
    volume_colour: vec4<f32>,
}

@group(0) @binding(0) var screen_texture: texture_2d<f32>;
@group(0) @binding(1) var texture_sampler: sampler;
@group(0) @binding(2) var<uniform> view: OutlineViewUniform;
@group(0) @binding(3) var<uniform> instance: ComposeOutputUniform;

struct FragmentOutput {
    @location(0) colour: vec4<f32>,
    @builtin(frag_depth) frag_depth: f32,
};

@fragment
fn fragment(in: FullscreenVertexOutput) -> FragmentOutput {
    let tex = textureSample(screen_texture, texture_sampler, in.uv);
    var out: FragmentOutput;
    if distance(in.position.xy, tex.xy) <= view.scale_physical_from_logical * instance.volume_offset {
        out.colour = instance.volume_colour;
        out.frag_depth = tex.z;
    }
    else {
        discard;
    }
    return out;
}
