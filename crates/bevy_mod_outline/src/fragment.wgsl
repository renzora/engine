#import bevy_mod_outline::common::VertexOutput

struct FragmentOutput {
    @location(0) colour: vec4<f32>,
#ifdef FLAT_DEPTH
#ifndef FLOOD_INIT
    @builtin(frag_depth) frag_depth: f32,
#endif
#endif
};

#ifdef ALPHA_MASK_TEXTURE
@group(3) @binding(0) var alpha_mask_texture: texture_2d<f32>;
@group(3) @binding(1) var alpha_mask_sampler: sampler;
#endif

@fragment
fn fragment(vertex: VertexOutput) -> FragmentOutput {
    var out: FragmentOutput;

#ifdef ALPHA_MASK_TEXTURE
    let alpha_mask = textureSample(alpha_mask_texture, alpha_mask_sampler, vertex.uv)[#{ALPHA_MASK_CHANNEL}];
    if (alpha_mask < vertex.alpha_mask_threshold) {
        discard;
    }
#endif

#ifdef FLAT_DEPTH
#ifndef FLOOD_INIT
    out.frag_depth = vertex.flat_depth; 
#endif
#endif
#ifdef VOLUME
    out.colour = vertex.volume_colour;
#endif
#ifdef FLOOD_INIT
    out.colour = vec4<f32>(vertex.position.xy, vertex.flat_depth, 0.0);
#endif
    return out;
}