// @effect(name = "Fog Overlay", icon = "cloud-fog")
//
// A cheap height-fade over the finished picture. It is a screen-space overlay,
// not a depth-aware fog: it knows how far up the frame a pixel is, not how far
// away it is.
//
// For fog that respects distance, use the engine's own **Distance Fog** on the
// World Environment instead.

@group(0) @binding(0) var screen_texture: texture_2d<f32>;
@group(0) @binding(1) var texture_sampler: sampler;

struct Settings {
    density: f32,  // @slider(0, 1, default = 0.3, step = 0.01)
    height: f32,   // @slider(0, 1, default = 0.3, step = 0.01)
    color_r: f32,  // @hidden(default = 0.7)
    color_g: f32,  // @hidden(default = 0.75)
    color_b: f32,  // @hidden(default = 0.8)
};
@group(0) @binding(2) var<uniform> settings: Settings;

@fragment
fn fragment(@builtin(position) pos: vec4<f32>, @location(0) in_uv: vec2<f32>) -> @location(0) vec4<f32> {
    let color = textureSample(screen_texture, texture_sampler, in_uv);

    let fog_color = vec3(settings.color_r, settings.color_g, settings.color_b);
    // Fog increases toward bottom of screen (simulating ground fog)
    let fog_amount = smoothstep(1.0 - settings.height, 1.0, in_uv.y) * settings.density;
    let result = mix(color.rgb, fog_color, fog_amount);
    return vec4(result, color.a);
}
