#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput
#import firefly::types::FireflyConfig

#import firefly::utils::blend

@group(0) @binding(0)
var screen_texture: texture_2d<f32>;

@group(0) @binding(1)
var light_map_texture: texture_2d<f32>;

@group(0) @binding(2)
var texture_sampler: sampler;

@group(0) @binding(3)
var texture_sampler2: sampler;

@group(0) @binding(4)
var<uniform> config: FireflyConfig;

#ifdef IS_COMBINED
@group(0) @binding(5)
var light_map_textures: texture_2d_array<f32>;
#endif

@fragment
fn fragment(vo: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    var light_frag = blend(textureSample(light_map_texture, texture_sampler2, vo.uv), vec4f(config.ambient_color, 0), config.ambient_brightness);

#ifdef IS_COMBINED
    for (var i = 0u; i < config.n_combined_lightmaps; i += 1) {
        let extra_light_frag = textureSample(light_map_textures, texture_sampler, vo.uv, i);
        if config.combination_mode == 0u {
            light_frag *= extra_light_frag;
        }
        else if config.combination_mode == 1u {
            light_frag += extra_light_frag; 
        }
        else if config.combination_mode == 2u {
            light_frag = max(light_frag, extra_light_frag);
        }
        else if config.combination_mode == 3u {
            light_frag = min(light_frag, extra_light_frag);
        }
    }
#endif    

    if config.light_bands > 0 {
        light_frag = floor(light_frag / vec4f(config.light_bands)) * config.light_bands;
    }

    let scene_frag = textureSample(screen_texture, texture_sampler, vo.uv);
    
    return scene_frag * light_frag;
}
