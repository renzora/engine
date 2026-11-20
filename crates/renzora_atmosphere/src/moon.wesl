// The moon and its glow. One shader for both: `params.x` picks.
//
// The moon is a sphere surfaced procedurally, so there is no image to ship or
// license: dark maria from low-frequency noise, bright highlands between, and
// craters with raised rims and a few ray systems, all in the sphere's own
// space so they stay put as the moon moves. It is shaded by `moon.light`,
// which `moon.rs` builds from the phase, not from the scene's sun.
//
// The glow is a larger sphere around it, adding a halo that falls off with
// angular distance from the moon's centre.

import bevy_pbr::render::forward_io::VertexOutput;
import bevy_pbr::render::mesh_view_bindings::view;

struct MoonUniform {
    // xyz = toward the light shading the face, w = lit luminance in nits.
    light: vec4<f32>,
    // rgb = face tint, a = fade (0 hidden, 1 risen).
    tint: vec4<f32>,
    // xyz = the moon's right on screen, w = eclipse 0 to 1.
    right: vec4<f32>,
    // xyz = the moon's up on screen, w = glow strength.
    up: vec4<f32>,
    // xyz = centre in world space, w = angular radius in radians.
    centre: vec4<f32>,
    // x = 1 for the glow, y = lit fraction, z = glow luminance in nits.
    params: vec4<f32>,
}

@group(3) @binding(0) var<uniform> moon: MoonUniform;

// Copper red of a totally eclipsed moon, and how much light reaches it.
const BLOOD: vec3<f32> = vec3<f32>(1.0, 0.32, 0.14);
const UMBRA_LIGHT: f32 = 0.06;
// The Earth's shadow, in moon radii: the dark umbra and its soft penumbra.
const UMBRA_RADIUS: f32 = 1.6;
const PENUMBRA_WIDTH: f32 = 0.9;

fn hash31(p: vec3<f32>) -> f32 {
    var h = fract(p * 0.1031);
    h += dot(h, h.yzx + 33.33);
    return fract((h.x + h.y) * h.z);
}

fn hash33(p: vec3<f32>) -> vec3<f32> {
    return vec3<f32>(
        hash31(p),
        hash31(p + vec3<f32>(13.71, 5.37, 9.11)),
        hash31(p + vec3<f32>(31.30, 17.10, 3.71)),
    );
}

fn noise3(p: vec3<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    return mix(
        mix(mix(hash31(i), hash31(i + vec3<f32>(1.0, 0.0, 0.0)), u.x),
            mix(hash31(i + vec3<f32>(0.0, 1.0, 0.0)), hash31(i + vec3<f32>(1.0, 1.0, 0.0)), u.x), u.y),
        mix(mix(hash31(i + vec3<f32>(0.0, 0.0, 1.0)), hash31(i + vec3<f32>(1.0, 0.0, 1.0)), u.x),
            mix(hash31(i + vec3<f32>(0.0, 1.0, 1.0)), hash31(i + vec3<f32>(1.0, 1.0, 1.0)), u.x), u.y),
        u.z,
    );
}

fn fbm(p: vec3<f32>) -> f32 {
    var sum = 0.0;
    var amp = 0.5;
    var q = p;
    for (var i = 0; i < 5; i += 1) {
        sum += amp * noise3(q);
        q = q * 2.07 + vec3<f32>(1.7, 9.2, 3.1);
        amp *= 0.5;
    }
    return sum;
}

// Craters at one scale: the nearest crater centre in a 3D cell grid on the
// sphere. Returns (darkening of the floor, brightening of the rim and rays).
fn craters(n: vec3<f32>, scale: f32, rays: bool) -> vec2<f32> {
    let p = n * scale;
    let base = floor(p);
    var floor_dark = 0.0;
    var bright = 0.0;
    for (var x = -1; x <= 1; x += 1) {
        for (var y = -1; y <= 1; y += 1) {
            for (var z = -1; z <= 1; z += 1) {
                let cell = base + vec3<f32>(f32(x), f32(y), f32(z));
                if hash31(cell + vec3<f32>(4.1, 7.3, 1.9)) > 0.45 {
                    continue;
                }
                let centre = normalize(cell + hash33(cell));
                let radius = (0.25 + 0.55 * hash31(cell + vec3<f32>(9.4, 2.2, 6.6))) / scale;
                let d = acos(clamp(dot(n, centre), -1.0, 1.0)) / radius;
                if d < 1.0 {
                    floor_dark = max(floor_dark, (1.0 - d * d) * 0.35);
                }
                // A raised, bright rim just outside the floor.
                bright = max(bright, exp(-pow((d - 1.0) * 6.0, 2.0)) * 0.5);
                // Young craters throw bright rays.
                if rays && hash31(cell + vec3<f32>(3.3, 8.8, 0.5)) > 0.7 && d < 6.0 {
                    let tangent = normalize(cross(centre, vec3<f32>(0.0, 1.0, 0.0)) + 1e-4);
                    let bitangent = cross(centre, tangent);
                    let offset = n - centre;
                    let angle = atan2(dot(offset, bitangent), dot(offset, tangent));
                    let streak = pow(abs(sin(angle * 9.0 + hash31(cell) * 6.28)), 18.0);
                    bright = max(bright, streak * (1.0 - d / 6.0) * 0.35);
                }
            }
        }
    }
    return vec2<f32>(floor_dark, bright);
}

// How the Earth's shadow covers a point on the face, as (umbra, penumbra),
// each 0 to 1. `p` is the point on the disc in moon radii, as seen from the
// viewer. The shadow enters from the lower left and is centred at totality.
fn eclipse_shadow(p: vec2<f32>, eclipse: f32) -> vec2<f32> {
    if eclipse <= 0.0 {
        return vec2<f32>(0.0);
    }
    let reach = UMBRA_RADIUS + PENUMBRA_WIDTH + 1.0;
    let c = normalize(vec2<f32>(-1.0, -0.35)) * (1.0 - eclipse) * reach;
    let d = length(p - c);
    let umbra = 1.0 - smoothstep(UMBRA_RADIUS - 0.12, UMBRA_RADIUS + 0.04, d);
    let penumbra = 1.0 - smoothstep(UMBRA_RADIUS, UMBRA_RADIUS + PENUMBRA_WIDTH, d);
    return vec2<f32>(umbra, penumbra);
}

// Colour the eclipse leaves at a point: grey in the penumbra, copper inside.
fn eclipse_tint(shadow: vec2<f32>) -> vec3<f32> {
    let grey = vec3<f32>(1.0 - 0.55 * shadow.y);
    return mix(grey, BLOOD * UMBRA_LIGHT, shadow.x);
}

fn glow(in: VertexOutput) -> vec4<f32> {
    let ray = normalize(in.world_position.xyz - view.world_position);
    let to_centre = normalize(moon.centre.xyz - view.world_position);
    let d = acos(clamp(dot(ray, to_centre), -1.0, 1.0));
    let r = moon.centre.w;
    // Distance beyond the limb, in moon radii.
    let x = max(d - r, 0.0) / r;
    // A bright, tight aureole and a wide faint haze, as moonlight scattered
    // by the air and the eye. Over the face itself only a little is added.
    var halo = exp(-x * 1.4) * 0.22 + exp(-x * 0.3) * 0.035;
    if d < r {
        halo *= 0.35;
    }
    // The glow is light the moon gives off: a crescent glows less, and an
    // eclipsed moon glows dim and red.
    let eclipse = moon.right.w;
    let colour = moon.tint.rgb * eclipse_tint(vec2<f32>(eclipse, eclipse));
    let strength = moon.up.w * moon.tint.a * (0.08 + 0.92 * moon.params.y);
    let rgb = colour * halo * strength * moon.params.z * view.exposure;
    if max(max(rgb.r, rgb.g), rgb.b) < 1e-7 {
        discard;
    }
    // Additive (`AlphaMode::Add`): alpha 0 leaves the sky behind undimmed.
    return vec4<f32>(rgb, 0.0);
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    if moon.params.x > 0.5 {
        return glow(in);
    }

    // The sphere is never rotated, so its world normal is also its own
    // surface coordinate: the features stay fixed on it.
    let n = normalize(in.world_normal);

    // Albedo: highlands, darkened by the maria and the crater floors.
    let maria = smoothstep(0.48, 0.58, fbm(n * 2.2));
    let mottle = fbm(n * 14.0);
    var albedo = mix(0.17, 0.075, maria) * (0.8 + 0.4 * mottle);
    let big = craters(n, 6.0, true);
    let small = craters(n, 22.0, false);
    albedo *= 1.0 - big.x - small.x * 0.6;
    albedo += (big.y + small.y * 0.5) * 0.12 * (1.0 - maria * 0.6);

    // The moon's surface scatters light back towards its source, so the
    // terminator is softened and the lit side stays fairly even rather than
    // shading like a matte ball.
    let lambert = dot(n, moon.light.xyz);
    let lit = smoothstep(-0.04, 0.12, lambert) * (0.75 + 0.25 * max(lambert, 0.0));
    // Earthshine: the dark side faintly lit by the Earth.
    let earthshine = 0.012;
    var colour = vec3<f32>(albedo / 0.12 * (lit + earthshine));

    // The Earth's shadow, placed on the face as the viewer sees it.
    let p = vec2<f32>(dot(n, moon.right.xyz), dot(n, moon.up.xyz));
    colour *= eclipse_tint(eclipse_shadow(p, moon.right.w));

    let rgb = moon.tint.rgb * colour * moon.light.w * view.exposure * moon.tint.a;
    // Opaque: the moon hides the stars behind it.
    return vec4<f32>(rgb, 1.0);
}
