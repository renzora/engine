// Screen-space indirect lighting. Five entry points share this file, and so
// one set of binding indices; each pass's layout lists only the ones it reads.
// See render.rs for the order they run in.
//
// The gather, the edge detection and the blur are Intel's ASSAO, converted to
// gather light instead of occlusion:
//
// Copyright (c) 2016, Intel Corporation
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to
// deal in the Software without restriction, including without limitation the
// rights to use, copy, modify, merge, publish, distribute, sublicense, and/or
// sell copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
// The above copyright notice and this permission notice shall be included in
// all copies or substantial portions of the Software.
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING
// FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS
// IN THE SOFTWARE.
//
// Positions and normals here are in Bevy's view space, where the camera looks
// down -z. ASSAO's view space looks down +z, so where it reads a depth as `z`
// this reads `-z`.

import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput;

struct SsilUniform {
    clip_from_view: mat4x4<f32>,
    view_from_clip: mat4x4<f32>,
    view_from_world: mat4x4<f32>,
    // This frame's view space to last frame's clip space, for reading the
    // history where each tap was a frame ago.
    reprojection: mat4x4<f32>,
    full_size: vec4<f32>,
    half_size: vec4<f32>,
    // radius (m), intensity (already times pi), 1 - sharpness, taps
    params: vec4<f32>,
    // normal rejection, 1 / near limit, -1 / radius, ambient scale (0: none)
    extra: vec4<f32>,
    // fade multiply, fade add, the mip the taps read, the 1x1 average mip
    fade: vec4<f32>,
    // The inverse of the environment light's rotation, as a quaternion.
    env_rotation: vec4<f32>,
    // x: the debug view (0 off, 1 light, 2 occlusion, 3 history);
    // y: occlusion strength, already times pi; zw unused.
    misc: vec4<f32>,
};

@group(0) @binding(0) var depth_tex: texture_depth_2d;
@group(0) @binding(1) var gbuffer_tex: texture_2d<u32>;
@group(0) @binding(2) var color_tex: texture_2d<f32>;
@group(0) @binding(3) var linear_sampler: sampler;
@group(0) @binding(4) var<uniform> ssil: SsilUniform;
@group(0) @binding(5) var blur_in: texture_2d<f32>;
@group(0) @binding(6) var light_tex: texture_2d<f32>;
@group(0) @binding(7) var history_tex: texture_2d<f32>;
@group(0) @binding(8) var mip_sampler: sampler;
@group(0) @binding(9) var edges_tex: texture_2d<f32>;
@group(0) @binding(10) var mirror_sampler: sampler;
@group(0) @binding(11) var env_tex: texture_cube<f32>;

const PI: f32 = 3.14159265;
// ASSAO's luma, used for its firefly compression.
const LUMA: vec3<f32> = vec3(0.299, 0.587, 0.114);
// The brightest light one tap may carry; an unbounded HDR value can be
// infinite, and one infinity poisons the whole average.
const MAX_LIGHT: f32 = 60000.0;
// The view depth given to the sky, beyond any radius and any depth edge.
const FAR: f32 = 1e6;

// ASSAO's disk: xy the offset, z the tap's weight, w unused here.
var<private> PATTERN: array<vec4<f32>, 32> = array<vec4<f32>, 32>(
    vec4(0.78488064, 0.56661671, 1.500000, -0.126083), vec4(0.26022232, -0.29575172, 1.500000, -1.064030), vec4(0.10459357, 0.08372527, 1.110000, -2.730563), vec4(-0.68286800, 0.04963045, 1.090000, -0.498827),
    vec4(-0.13570161, -0.64190155, 1.250000, -0.532765), vec4(-0.26193795, -0.08205118, 0.670000, -1.783245), vec4(-0.61177456, 0.66664219, 0.710000, -0.044234), vec4(0.43675563, 0.25119025, 0.610000, -1.167283),
    vec4(0.07884444, 0.86618668, 0.640000, -0.459002), vec4(-0.12790935, -0.29869005, 0.600000, -1.729424), vec4(-0.04031125, 0.02413622, 0.600000, -4.792042), vec4(0.16201244, -0.52851415, 0.790000, -1.067055),
    vec4(-0.70991218, 0.47301072, 0.640000, -0.335236), vec4(0.03277707, -0.22349690, 0.600000, -1.982384), vec4(0.68921727, 0.36800742, 0.630000, -0.266718), vec4(0.29251814, 0.37775412, 0.610000, -1.422520),
    vec4(-0.12224089, 0.96582592, 0.600000, -0.426142), vec4(0.11071457, -0.16131058, 0.600000, -2.165947), vec4(0.46562141, -0.59747696, 0.600000, -0.189760), vec4(-0.51548797, 0.11804193, 0.600000, -1.246800),
    vec4(0.89141309, -0.42090443, 0.600000, 0.028192), vec4(-0.32402530, -0.01591529, 0.600000, -1.543018), vec4(0.60771245, 0.41635221, 0.600000, -0.605411), vec4(0.02379565, -0.08239821, 0.600000, -3.809046),
    vec4(0.48951152, -0.23657045, 0.600000, -1.189011), vec4(-0.17611565, -0.81696892, 0.600000, -0.513724), vec4(-0.33930185, -0.20732205, 0.600000, -1.698047), vec4(-0.91974425, 0.05403209, 0.600000, 0.062246),
    vec4(-0.15064627, -0.14949332, 0.600000, -1.896062), vec4(0.53180975, -0.35210401, 0.600000, -0.758838), vec4(0.41487166, 0.81442589, 0.600000, -0.505648), vec4(-0.24106961, -0.32721516, 0.600000, -1.665244)
);

// The order ASSAO visits its five sub-rotations in.
var<private> SUB_PASS: array<f32, 5> = array<f32, 5>(0.0, 1.0, 4.0, 3.0, 2.0);

// A view-space position from a pixel centre (full resolution) and its depth.
fn view_position(pixel: vec2<f32>, depth: f32) -> vec3<f32> {
    let uv = pixel * ssil.full_size.zw;
    let ndc = vec2(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0);
    let h = ssil.view_from_clip * vec4(ndc, depth, 1.0);
    return h.xyz / h.w;
}

fn full_max() -> vec2<i32> {
    return vec2<i32>(ssil.full_size.xy) - 1;
}

// Distance in front of the camera at a full-resolution pixel; the sky is FAR.
fn linear_depth(pixel: vec2<i32>) -> f32 {
    let p = clamp(pixel, vec2(0), full_max());
    let depth = textureLoad(depth_tex, p, 0);
    if depth <= 0.0 {
        return FAR;
    }
    return -view_position(vec2<f32>(p) + 0.5, depth).z;
}

// The G-buffer's normal: oct24 in the low 24 bits of the last channel, in
// world space.
fn world_normal(g: vec4<u32>) -> vec3<f32> {
    let v = vec2(f32(g.a & 0xFFFu), f32((g.a >> 12u) & 0xFFFu)) / 4095.0;
    let f = v * 2.0 - 1.0;
    var n = vec3(f.xy, 1.0 - abs(f.x) - abs(f.y));
    let t = saturate(-n.z);
    let w = select(vec2(t), vec2(-t), n.xy >= vec2(0.0));
    n = vec3(n.xy + w, n.z);
    return normalize(n);
}

fn view_normal(g: vec4<u32>) -> vec3<f32> {
    return normalize((ssil.view_from_world * vec4(world_normal(g), 0.0)).xyz);
}

fn is_unlit(g: vec4<u32>) -> bool {
    return ((g.a >> 24u) & 1u) != 0u;
}

// Slope-aware depth edges to the left, right, top and bottom: 1 is no edge.
fn calculate_edges(center: f32, left: f32, right: f32, top: f32, bottom: f32) -> vec4<f32> {
    var edges = vec4(left, right, top, bottom) - center;
    let slope_adjusted = edges + edges.yxwz;
    edges = min(abs(edges), abs(slope_adjusted));
    return clamp(1.3 - edges / (center * 0.040), vec4(0.0), vec4(1.0));
}

// Four edges in two bits each, to fit one 8-bit texel.
fn pack_edges(edges: vec4<f32>) -> f32 {
    let q = round(clamp(edges, vec4(0.0), vec4(1.0)) * 3.05);
    return dot(q, vec4(64.0, 16.0, 4.0, 1.0) / 255.0);
}

fn unpack_edges(packed: f32) -> vec4<f32> {
    let p = u32(packed * 255.5);
    let e = vec4(f32((p >> 6u) & 3u), f32((p >> 4u) & 3u), f32((p >> 2u) & 3u), f32(p & 3u)) / 3.0;
    return clamp(e + ssil.params.z, vec4(0.0), vec4(1.0));
}

// One of ASSAO's twenty disk rotations, as the matrix columns (x, y) and
// (z, w). Its CPU side builds the same table.
fn rotation(pass_index: u32, sub: u32) -> vec4<f32> {
    let a = f32(pass_index);
    let b = SUB_PASS[sub];
    let angle = (a + b / 5.0) * PI * 0.5;
    let scale = 1.0 + (a - 1.5 + (b - 2.0) / 5.0) * 0.07;
    let c = cos(angle);
    let s = sin(angle);
    return vec4(scale * c, scale * -s, -scale * s, -scale * c);
}

struct Tap {
    color: vec3<f32>,
    obscurance: f32,
    weight: f32,
};

// One tap, `offset` half-resolution pixels from the receiver.
fn tap(centre: vec2<f32>, offset: vec2<f32>, p: vec3<f32>, n: vec3<f32>, falloff_sq: f32, weight_mod: f32, key: f32) -> Tap {
    let at = clamp(vec2<i32>(floor(centre + offset * 2.0)), vec2(0), full_max());
    let depth = textureLoad(depth_tex, at, 0);
    // The sky: ASSAO's far plane, beyond any falloff, so it counts toward the
    // average with nothing in it.
    if depth <= 0.0 {
        return Tap(vec3(0.0), 0.0, weight_mod);
    }
    let hit = view_position(vec2<f32>(at) + 0.5, depth);
    let delta = hit - p;
    let length_sq = dot(delta, delta);
    if length_sq < 1e-12 {
        return Tap(vec3(0.0), 0.0, weight_mod);
    }
    let dir = delta * inverseSqrt(length_sq);
    let obscurance = max(0.0, dot(n, dir) - 0.05) * max(0.0, length_sq * falloff_sq + 1.0);

    // Haloing reduction: a tap well in front of the receiver counts for less.
    let reduce = clamp(max(0.0, delta.z) * ssil.extra.z + 2.0, 0.0, 1.0);
    let weight = (0.8 * reduce + 0.2) * weight_mod;

    // Where this tap was last frame, read from a blurred mip of last frame.
    let prev = ssil.reprojection * vec4(hit, 1.0);
    let ndc = prev.xy / prev.w;
    let uv = vec2(ndc.x * 0.5 + 0.5, 0.5 - ndc.y * 0.5);
    var light = textureSampleLevel(history_tex, mip_sampler, uv, ssil.fade.z).rgb;
    // NaN fails every comparison, so this drops it with the negatives.
    if !all(light >= vec3(0.0)) {
        light = vec3(0.0);
    }
    // Tonemapped before averaging so one blown-out tap cannot dominate, in
    // units of the frame's average brightness. ASSAO's `c / (1 + luma)`
    // assumes a frame near 1; a sunlit pixel under physical light units is
    // nearer 15, and compressing that as it stands flattened every bright tap
    // to about 1 and lost nine tenths of the bounce.
    light = min(light, vec3(MAX_LIGHT)) / key;
    light /= 1.0 + dot(light, LUMA);

    let sender = view_normal(textureLoad(gbuffer_tex, at, 0));
    let reject = mix(1.0, smoothstep(0.0, 0.1, -dot(sender, dir)), ssil.extra.x);
    return Tap(light * obscurance * weight * reject, obscurance * weight, weight);
}

struct GatherOutput {
    @location(0) light: vec4<f32>,
    @location(1) edges: vec4<f32>,
};

// Half resolution. Writes the light in `rgb`, the obscurance in `a`, and the
// packed depth edges the blur keeps to.
@fragment
fn gather(in: FullscreenVertexOutput) -> GatherOutput {
    let gp = vec2<i32>(floor(in.position.xy));
    let pixel = min(gp * 2, full_max());
    let depth = textureLoad(depth_tex, pixel, 0);
    if depth <= 0.0 {
        return GatherOutput(vec4(0.0), vec4(pack_edges(vec4(0.0))));
    }
    let centre = vec2<f32>(pixel) + 0.5;
    var p = view_position(centre, depth);
    let n = view_normal(textureLoad(gbuffer_tex, pixel, 0));
    let pix_z = -p.z;

    let edges = calculate_edges(
        pix_z,
        linear_depth(pixel + vec2(-2, 0)),
        linear_depth(pixel + vec2(2, 0)),
        linear_depth(pixel + vec2(0, -2)),
        linear_depth(pixel + vec2(0, 2)),
    );

    // The radius on screen, in half-resolution pixels, shrunk up close so the
    // disk cannot outgrow the screen.
    let pixel_size = max(abs(view_position(centre + vec2(2.0, 0.0), depth).x - p.x), 1e-6);
    let too_close = clamp(length(p) * ssil.extra.y, 0.0, 1.0) * 0.8 + 0.2;
    let radius = ssil.params.x * too_close;
    var lookup = 0.85 * radius / pixel_size;
    let falloff_sq = -1.0 / (radius * radius);
    // Smaller near the screen border, where half the disk would be off it.
    let uv = centre * ssil.full_size.zw;
    let border = min(min(uv.x, 1.0 - uv.x), min(uv.y, 1.0 - uv.y));
    lookup *= clamp(10.0 * border + 0.6, 0.0, 1.0);

    // ASSAO splits the screen into four interleaved passes with five
    // rotations each; the same twenty, picked by the same pixel pattern.
    let pass_index = u32(gp.x & 1) + u32(gp.y & 1) * 2u;
    let slice = vec2<u32>(gp / 2);
    let rot = rotation(pass_index, (slice.y * 2u + slice.x) % 5u) * lookup;

    // Nudged toward the camera against depth imprecision.
    p *= 0.99;

    // Last frame's average brightness, from the history's 1x1 mip.
    let average = textureSampleLevel(history_tex, mip_sampler, vec2(0.5), ssil.fade.w).rgb;
    var key = dot(average, LUMA);
    if !(key > 1e-6) || !(key < MAX_LIGHT) {
        key = 1.0;
    }

    var color_sum = vec3(0.0);
    var obscurance_sum = 0.0;
    var weight_sum = 0.0;
    let taps = u32(ssil.params.w);
    for (var i = 0u; i < taps; i++) {
        let s = PATTERN[i];
        let offset = round(vec2(dot(s.xy, rot.xy), dot(s.xy, rot.zw)));
        let a = tap(centre, offset, p, n, falloff_sq, s.z, key);
        let b = tap(centre, -offset, p, n, falloff_sq, s.z, key);
        color_sum += a.color + b.color;
        obscurance_sum += a.obscurance + b.obscurance;
        weight_sum += a.weight + b.weight;
    }
    if weight_sum <= 0.0 {
        return GatherOutput(vec4(0.0), vec4(pack_edges(edges)));
    }

    var color = color_sum / weight_sum;
    color /= max(1.0 - dot(color, LUMA), 1e-3);
    color *= key;

    var fade = clamp(-p.z * ssil.fade.x + ssil.fade.y, 0.0, 1.0);
    // Fade out on thin features with edges on opposite sides, which alias.
    let edge_fade = clamp((1.0 - edges.x - edges.y) * 0.35, 0.0, 1.0)
        + clamp((1.0 - edges.z - edges.w) * 0.35, 0.0, 1.0);
    fade *= clamp(1.0 - edge_fade, 0.0, 1.0);

    // ASSAO scales both by one intensity; here the bounce and the dimming each
    // have their own (see `SsilSettings::occlusion`).
    color *= ssil.params.y * fade;
    let obscurance = clamp(obscurance_sum / weight_sum * ssil.misc.y, 0.0, 1.0);
    return GatherOutput(vec4(color, obscurance), vec4(pack_edges(edges)));
}

fn half_max() -> vec2<i32> {
    return vec2<i32>(ssil.half_size.xy) - 1;
}

fn edges_at(pixel: vec2<i32>) -> vec4<f32> {
    return unpack_edges(textureLoad(edges_tex, clamp(pixel, vec2(0), half_max()), 0).r);
}

// ASSAO's wide blur: two half-resolution pixels to each side.
@fragment
fn blur_wide(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let gp = vec2<i32>(floor(in.position.xy));
    let uv = in.position.xy * ssil.half_size.zw;
    let c = textureSampleLevel(blur_in, mirror_sampler, uv, 0.0);
    let l = textureSampleLevel(blur_in, mirror_sampler, uv, 0.0, vec2(-2, 0));
    let t = textureSampleLevel(blur_in, mirror_sampler, uv, 0.0, vec2(0, -2));
    let r = textureSampleLevel(blur_in, mirror_sampler, uv, 0.0, vec2(2, 0));
    let b = textureSampleLevel(blur_in, mirror_sampler, uv, 0.0, vec2(0, 2));

    var e = edges_at(gp);
    e.x *= edges_at(gp + vec2(-2, 0)).y;
    e.z *= edges_at(gp + vec2(0, -2)).w;
    e.y *= edges_at(gp + vec2(2, 0)).x;
    e.w *= edges_at(gp + vec2(0, 2)).z;

    let sum = c * 0.8 + l * e.x + r * e.y + t * e.z + b * e.w;
    return sum / (0.8 + e.x + e.y + e.z + e.w);
}

// ASSAO's smart blur: the four direct neighbours, each cut off at an edge.
@fragment
fn blur_smart(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let gp = vec2<i32>(floor(in.position.xy));
    let uv = in.position.xy * ssil.half_size.zw;
    let c = textureSampleLevel(blur_in, mirror_sampler, uv, 0.0);
    let l = textureSampleLevel(blur_in, mirror_sampler, uv, 0.0, vec2(-1, 0));
    let t = textureSampleLevel(blur_in, mirror_sampler, uv, 0.0, vec2(0, -1));
    let r = textureSampleLevel(blur_in, mirror_sampler, uv, 0.0, vec2(1, 0));
    let b = textureSampleLevel(blur_in, mirror_sampler, uv, 0.0, vec2(0, 1));
    let e = edges_at(gp);
    let sum = c * 0.5 + l * e.x + r * e.y + t * e.z + b * e.w;
    return sum / (0.5 + e.x + e.y + e.z + e.w);
}

// A bilinear copy into the next mip of the history: an exact copy at the
// same size, a 2x2 average at half.
@fragment
fn downsample(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    return textureSampleLevel(blur_in, linear_sampler, in.uv, 0.0);
}

fn quat_rotate(q: vec4<f32>, v: vec3<f32>) -> vec3<f32> {
    let t = 2.0 * cross(q.xyz, v);
    return v + q.w * t + cross(q.xyz, t);
}

// Full resolution: the ambient light the occlusion says is blocked comes off,
// the bounce that replaces it goes on, both on the pixel's diffuse colour.
@fragment
fn composite(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let pixel = vec2<i32>(in.position.xy);
    let frame = textureLoad(color_tex, pixel, 0);
    let depth = textureLoad(depth_tex, pixel, 0);
    let uv = (vec2<f32>(pixel) + 0.5) * ssil.full_size.zw;
    let debug_view = u32(ssil.misc.x + 0.5);
    // Last frame as the taps read it: reprojected and from the same mip.
    if debug_view == 3u {
        var at = uv;
        if depth > 0.0 {
            let prev = ssil.reprojection * vec4(view_position(vec2<f32>(pixel) + 0.5, depth), 1.0);
            let ndc = prev.xy / prev.w;
            at = vec2(ndc.x * 0.5 + 0.5, 0.5 - ndc.y * 0.5);
        }
        return vec4(textureSampleLevel(history_tex, mip_sampler, at, ssil.fade.z).rgb, frame.a);
    }
    if depth <= 0.0 {
        if debug_view == 1u {
            return vec4(0.0, 0.0, 0.0, frame.a);
        }
        if debug_view == 2u {
            return vec4(1.0, 1.0, 1.0, frame.a);
        }
        return frame;
    }
    let g = textureLoad(gbuffer_tex, pixel, 0);
    // An unlit surface shows its own colour and receives no light.
    if is_unlit(g) {
        return frame;
    }
    let base = unpack4x8unorm(g.r);
    let metallic = unpack4x8unorm(g.b).g;
    // Metal has no diffuse colour to bounce light off.
    let diffuse = pow(base.rgb, vec3(2.2)) * (1.0 - metallic);

    // The half-resolution result read bilinearly, as a material would.
    let s = textureSampleLevel(light_tex, linear_sampler, uv, 0.0);
    if !all(s >= vec4(0.0)) {
        return frame;
    }
    if debug_view == 1u {
        return vec4(s.rgb, frame.a);
    }
    if debug_view == 2u {
        return vec4(vec3(1.0 - s.a), frame.a);
    }

    // The ambient light this pixel received, as Bevy's environment light
    // gave it. The pass runs after lighting, so it cannot scale only the
    // ambient term the way a material can; it takes off what that term was.
    var ambient = vec3(0.0);
    if ssil.extra.w > 0.0 {
        var dir = quat_rotate(ssil.env_rotation, world_normal(g));
        dir.z = -dir.z;
        ambient = textureSampleLevel(env_tex, linear_sampler, dir, 0.0).rgb * ssil.extra.w * diffuse;
    }
    let dimmed = max(frame.rgb - ambient * s.a, vec3(0.0));
    let lit = dimmed + min(s.rgb * diffuse, vec3(MAX_LIGHT));
    return vec4(lit, frame.a);
}
