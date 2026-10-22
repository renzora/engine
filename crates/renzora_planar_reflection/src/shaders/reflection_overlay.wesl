// Planar reflection overlay: the reflecting surface drawn a second time, over
// its own material, showing what the mirror camera saw.
//
// The mirror camera renders the reflected scene into its own HDR image from a
// pose that is the viewing camera reflected across the plane. Every point on
// the surface therefore finds its reflection at the SAME screen position in
// that image, mirrored horizontally (the mirror camera keeps a proper rotation,
// so its right axis is the reflection's left). Sampling is a screen-space
// lookup: fragment position over the viewport, x flipped.
//
// The image is linear HDR with tonemapping off. This pass runs inside the
// viewing camera's HDR main pass, which tonemaps once at the end; tonemapping
// the capture as well would compress it twice and wash it out. Only a view that
// tonemaps in the shader (an LDR camera) does it here.

import bevy_pbr::render::mesh_functions;
import bevy_pbr::render::mesh_view_bindings::view;
@if(TONEMAP_IN_SHADER) import bevy_core_pipeline::tonemapping::tone_mapping;

struct OverlayParams {
    // rgb multiplied into the reflection
    tint: vec4<f32>,
    // x strength, y fresnel, z blur, w unused
    shading: vec4<f32>,
    // xyz world-space normal of the reflecting side
    normal: vec4<f32>,
    // xyz world position of the one camera this overlay belongs to, w the
    // distance within which another view counts as that camera
    view_origin: vec4<f32>,
    // xyz forward direction of that camera
    view_forward: vec4<f32>,
};

@group(3) @binding(0) var<uniform> params: OverlayParams;
@group(3) @binding(1) var reflection_texture: texture_2d<f32>;
@group(3) @binding(2) var reflection_sampler: sampler;

struct Vertex {
    @builtin(instance_index) instance_index: u32,
    @location(0) position: vec3<f32>,
};

struct VertexOut {
    @builtin(position) position: vec4<f32>,
    @location(0) world_position: vec3<f32>,
};

// Relative pull toward the camera, so the overlay wins the depth test against
// the coplanar surface it covers. Relative rather than a fixed offset because
// a fixed NDC offset is centimetres up close and metres at a distance under a
// reverse-Z projection; scaling clip z keeps it a constant fraction of the
// view distance (0.02%, 2 cm at 100 m), far above f32 depth precision.
const DEPTH_NUDGE: f32 = 0.0002;

// Disc blur: taps on a Vogel spiral, one pass. Twelve is enough that a rough
// surface reads as soft rather than as twelve ghost images.
const BLUR_TAPS: u32 = 12u;
const GOLDEN_ANGLE: f32 = 2.39996323;
// Blur radius at `blur = 1`, as a fraction of the viewport height.
const MAX_BLUR_RADIUS: f32 = 0.03;

// The Schlick reflectance of a dielectric seen head on (water and glass sit
// near 0.02 to 0.04). `fresnel = 1` follows this curve; `fresnel = 0` ignores
// the angle altogether, which is what a mirror wants.
const F0: f32 = 0.04;

@vertex
fn vertex(v: Vertex) -> VertexOut {
    let world_from_local = mesh_functions::get_world_from_local(v.instance_index);
    let world = (world_from_local * vec4<f32>(v.position, 1.0)).xyz;
    var clip = view.clip_from_world * vec4<f32>(world, 1.0);
    clip.z = clip.z * (1.0 + DEPTH_NUDGE);
    var out: VertexOut;
    out.position = clip;
    out.world_position = world;
    return out;
}

fn sample_reflection(uv: vec2<f32>, radius: f32, aspect: f32) -> vec3<f32> {
    // `textureSampleLevel` throughout: `fragment` may discard before calling
    // this, which leaves the control flow non-uniform where implicit-derivative
    // sampling is not allowed, and the capture has no mips to choose between.
    var sum = textureSampleLevel(reflection_texture, reflection_sampler, uv, 0.0).rgb;
    if radius <= 0.0 {
        return sum;
    }
    for (var i = 0u; i < BLUR_TAPS; i = i + 1u) {
        let fi = f32(i) + 0.5;
        let r = sqrt(fi / f32(BLUR_TAPS)) * radius;
        let a = fi * GOLDEN_ANGLE;
        let offset = vec2<f32>(cos(a) / aspect, sin(a)) * r;
        sum = sum + textureSampleLevel(reflection_texture, reflection_sampler, uv + offset, 0.0).rgb;
    }
    return sum / f32(BLUR_TAPS + 1u);
}

@fragment
fn fragment(in: VertexOut) -> @location(0) vec4<f32> {
    // Draw only in the view this overlay was captured for. Every other view
    // that can see the surface (another viewport, a preview, and above all any
    // mirror camera, including this mirror's own) would otherwise sample an
    // image taken from somewhere else; in a mirror camera that is also how
    // reflections would recurse.
    let camera_forward = -view.world_from_view[2].xyz;
    if distance(view.world_position, params.view_origin.xyz) > params.view_origin.w
        || dot(camera_forward, params.view_forward.xyz) < 0.9999 {
        discard;
    }

    let size = max(view.viewport.zw, vec2<f32>(1.0));
    let screen = (in.position.xy - view.viewport.xy) / size;
    let uv = vec2<f32>(1.0 - screen.x, screen.y);

    let strength = params.shading.x;
    let fresnel = params.shading.y;
    let blur = params.shading.z;

    let reflected = sample_reflection(uv, blur * MAX_BLUR_RADIUS, size.x / size.y) * params.tint.rgb;

    let to_camera = normalize(view.world_position - in.world_position);
    let cos_theta = clamp(dot(normalize(params.normal.xyz), to_camera), 0.0, 1.0);
    let schlick = F0 + (1.0 - F0) * pow(1.0 - cos_theta, 5.0);
    let weight = clamp(strength * mix(1.0, schlick, fresnel), 0.0, 1.0);

    var out = vec4<f32>(reflected, weight);
    @if(TONEMAP_IN_SHADER) {
        out = tone_mapping(out, view.color_grading);
    }
    return out;
}
