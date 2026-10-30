// @effect(name = "Emboss", icon = "stamp")
//
// Emboss: the picture is turned into a relief of light and shadow along its
// edges, as if stamped into metal or paper.

@group(0) @binding(0) var screen_texture: texture_2d<f32>;
@group(0) @binding(1) var texture_sampler: sampler;

struct Settings {
    strength: f32,    // @slider(0, 3, default = 1, step = 0.01)
    mix_amount: f32,  // @slider(0, 1, default = 0.5, step = 0.01)
};
@group(0) @binding(2) var<uniform> settings: Settings;

@fragment
fn fragment(@builtin(position) pos: vec4<f32>, @location(0) in_uv: vec2<f32>) -> @location(0) vec4<f32> {
    let color = textureSample(screen_texture, texture_sampler, in_uv);

    let tex_size = vec2<f32>(textureDimensions(screen_texture));
    let texel = 1.0 / tex_size;

    // 3x3 emboss kernel: top-left (+strength) minus bottom-right (-strength)
    let tl = textureSample(screen_texture, texture_sampler, in_uv + vec2(-texel.x, -texel.y)).rgb;
    let tr = textureSample(screen_texture, texture_sampler, in_uv + vec2( texel.x, -texel.y)).rgb;
    let bl = textureSample(screen_texture, texture_sampler, in_uv + vec2(-texel.x,  texel.y)).rgb;
    let br = textureSample(screen_texture, texture_sampler, in_uv + vec2( texel.x,  texel.y)).rgb;
    let tc = textureSample(screen_texture, texture_sampler, in_uv + vec2( 0.0,     -texel.y)).rgb;
    let bc = textureSample(screen_texture, texture_sampler, in_uv + vec2( 0.0,      texel.y)).rgb;
    let cl = textureSample(screen_texture, texture_sampler, in_uv + vec2(-texel.x,  0.0    )).rgb;
    let cr = textureSample(screen_texture, texture_sampler, in_uv + vec2( texel.x,  0.0    )).rgb;

    // Emboss: directional kernel (top-left positive, bottom-right negative)
    let embossed = (tl * 2.0 + tc * 1.0 + cl * 1.0)
                 - (br * 2.0 + bc * 1.0 + cr * 1.0);

    let grey = vec3(dot(embossed * settings.strength, vec3(0.299, 0.587, 0.114)) + 0.5);
    let result = mix(color.rgb, clamp(grey, vec3(0.0), vec3(1.0)), settings.mix_amount);
    return vec4(result, color.a);
}
