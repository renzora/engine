// The fragment stage only. The vertex stage is Bevy's own mesh shader, which is
// why `VertexOutput` can be imported rather than declared: it is the struct that
// stage hands over, and skinning, morph targets and the model transform have all
// already happened by the time this runs.
//
// The import works at all because a material is compiled through Bevy's pipeline,
// with naga_oil in the path. A post-process shader has no such luxury — it goes
// straight to naga and must be self-contained.
#import bevy_pbr::forward_io::VertexOutput

// `@group(3)`, not 2: Bevy 0.19 binds view data at 0 and mesh data at 1 and 2.
//
// Two `vec4`s rather than six scalars because the uniform address space requires
// a struct aligned to 16, and a run of `f32` is aligned to 4. The Rust side is
// eight plain `f32` fields, which is the same 32 bytes — it is only WGSL that
// insists on saying so in `vec4`s.
struct RippleSettings {
    // time, speed, amplitude, unused
    params: vec4<f32>,
    // rgb, unused
    tint: vec4<f32>,
};

@group(3) @binding(0) var<uniform> settings: RippleSettings;

// Bound from binding 1 upward, each texture followed by its sampler. This one is
// written from the CPU every frame, so it is a plasma rather than a still image —
// the point is to show the upload path working, not the pattern.
@group(3) @binding(1) var noise_texture: texture_2d<f32>;
@group(3) @binding(2) var noise_sampler: sampler;

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let time = settings.params.x;
    let speed = settings.params.y;
    let amplitude = settings.params.z;

    // Concentric rings travelling out from the middle of the quad. Reading UVs
    // rather than world position keeps this correct wherever the entity is moved
    // to, which makes it a better check that the vertex stage is really Bevy's.
    let centered = in.uv - vec2(0.5, 0.5);
    let distance = length(centered);
    let wave = sin(distance * 40.0 - time * speed) * 0.5 + 0.5;

    let noise = textureSample(noise_texture, noise_sampler, in.uv).rgb;

    // Deliberately obvious when something is wrong: an unbound uniform reads as
    // zero, which is flat black, and an unbound texture reads as the fallback
    // white. Neither can be mistaken for the intended result.
    let lit = settings.tint.rgb * (1.0 - amplitude + wave * amplitude);
    return vec4(lit * (0.4 + noise * 0.6), 1.0);
}
