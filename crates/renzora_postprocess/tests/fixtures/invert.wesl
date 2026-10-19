// @effect(name = "Invert", icon = "circle-half")

@group(0) @binding(0) var screen_texture: texture_2d<f32>;
@group(0) @binding(1) var screen_sampler: sampler;

struct Settings {
    amount: f32,    // @slider(0, 1, default = 1)
};
@group(0) @binding(2) var<uniform> settings: Settings;

@fragment
fn fragment(@location(0) uv: vec2<f32>) -> @location(0) vec4<f32> {
    let c = textureSample(screen_texture, screen_sampler, uv);
    return vec4(mix(c.rgb, 1.0 - c.rgb, settings.amount), c.a);
}
