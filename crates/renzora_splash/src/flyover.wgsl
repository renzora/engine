// Finishing pass for the splash terrain-flyover render. Samples the offscreen
// terrain image and applies a gentle lens vignette + faint film grain — no hard
// cuts or chromatic split (the flyover is one continuous shot, unlike the old
// shot-cut city). Alpha is preserved so the transparent sky above the ridgeline
// keeps showing the night shader behind it.

#import bevy_ui::ui_vertex_output::UiVertexOutput

struct FlyoverUniforms {
    params: vec4<f32>, // x = time (s)
};

@group(1) @binding(0) var<uniform> u: FlyoverUniforms;
@group(1) @binding(1) var tex: texture_2d<f32>;
@group(1) @binding(2) var tex_sampler: sampler;

fn rand(p: f32) -> f32 {
    return fract(sin(p * 12.9898) * 43758.5453);
}

@fragment
fn fragment(in: UiVertexOutput) -> @location(0) vec4<f32> {
    let t = u.params.x;
    let c = textureSample(tex, tex_sampler, in.uv);

    var col = c.rgb;

    // ── Hue grade (brightness-neutral) ──
    // Shift the warm-brown ground toward a cooler, moonlit tone to match the night
    // sky. HUE_TINT is the single dial: more blue = cooler, raise .r/.g for warmer
    // or greener.
    let lum = dot(col, vec3<f32>(0.299, 0.587, 0.114));
    col = mix(vec3<f32>(lum), col, 0.85);                  // trim saturation slightly
    let HUE_TINT = vec3<f32>(0.84, 0.93, 1.16);            // warm brown → cool moonlit blue
    col = col * HUE_TINT;
    col = (col - 0.42) * 1.08 + 0.42;                      // gentle contrast (pivot keeps brightness)
    col = max(col, vec3<f32>(0.0));

    // Faint grain for a filmic, alive feel.
    col = col + (rand(in.uv.x * 53.7 + in.uv.y * 91.3 + t * 7.0) - 0.5) * 0.020;

    // Soft lens vignette toward the corners.
    let vig = smoothstep(1.05, 0.35, length(in.uv - vec2<f32>(0.5, 0.5)));
    col = col * mix(0.82, 1.0, vig);

    return vec4<f32>(col, c.a);
}
