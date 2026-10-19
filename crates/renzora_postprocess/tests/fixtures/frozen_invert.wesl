// @effect(name = "Frozen Invert", freeze_while = "hold > 0.5")
//
// Shows the inverted frozen copy of the frame, never the live one, so a bright
// result can only come from the snapshot having been taken and bound.

@group(0) @binding(0) var screen_texture: texture_2d<f32>;
@group(0) @binding(1) var screen_sampler: sampler;

struct Settings {
    hold: f32,    // @slider(0, 1, default = 0)
};
@group(0) @binding(2) var<uniform> settings: Settings;

@group(0) @binding(3) var frozen_texture: texture_2d<f32>;
@group(0) @binding(4) var frozen_sampler: sampler;

@fragment
fn fragment(@location(0) uv: vec2<f32>) -> @location(0) vec4<f32> {
    let live = textureSample(screen_texture, screen_sampler, uv);
    let frozen = textureSample(frozen_texture, frozen_sampler, uv);
    return vec4(1.0 - frozen.rgb, live.a);
}
