// Ocean surface shader — a port of GodotOceanWaves' `water.gdshader` (MIT).
//
// The waves themselves are not computed here: `sim.rs` runs an inverse FFT of a
// JONSWAP/TMA spectrum into a stack of cascade maps, and this shader displaces
// vertices by them and shades the result. The lighting model follows the 2019
// GDC talk "Wakes, Explosions and Lighting: Interactive Water Simulation in
// Atlas".
//
// One deliberate deviation from the original. In Godot, `SPECULAR_LIGHT`
// implicitly picks up sky reflection from the environment, so the shader only
// had to add its own sun highlight. Bevy has no such implicit term, so the
// environment, shadows and every other light come from `apply_pbr_lighting`
// with this shader's fresnel-derived roughness — and the reference's
// subsurface-scattering term is layered on top of it. The result responds like
// the reference while still sitting inside whatever atmosphere or environment
// map the scene uses.

#import bevy_pbr::mesh_functions
#import bevy_pbr::view_transformations::position_world_to_clip
#import bevy_pbr::mesh_view_bindings as view_bindings
#import bevy_pbr::pbr_functions
#import bevy_pbr::pbr_types::{PbrInput, pbr_input_new}

const PI: f32 = 3.14159265359;
/// Reflectance from air to water (eta = 1.33).
const REFLECTANCE: f32 = 0.02;
/// Bevy remaps `F0 = 0.16 * reflectance²`, so this is `REFLECTANCE` expressed
/// the way the PBR material wants it.
const PBR_REFLECTANCE: f32 = 0.3536;

struct WaterUniforms {
    water_color: vec4<f32>,
    foam_color: vec4<f32>,
    // xyz = direction the sun points, w = normalised intensity.
    sun_direction: vec4<f32>,
    // (displacement_range, displacement_falloff, normal_falloff, foam_falloff),
    // scaled CPU-side by how far this water actually reaches. The reference's
    // literal constants assume a +-256 m ocean; on a bigger one they flatten
    // everything past a quarter of the view into a dead plane.
    distance_scales: vec4<f32>,
    // Per cascade: (1/tile.x, 1/tile.y, displacement_scale, normal_scale).
    map_scales: array<vec4<f32>, 8>,
    num_cascades: u32,
    roughness: f32,
    normal_strength: f32,
    pad: f32,
}

@group(3) @binding(0) var<uniform> water: WaterUniforms;
@group(3) @binding(1) var displacement_maps: texture_2d_array<f32>;
@group(3) @binding(2) var displacement_sampler: sampler;
@group(3) @binding(3) var normal_maps: texture_2d_array<f32>;
@group(3) @binding(4) var normal_sampler: sampler;

struct WaterVertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_position: vec4<f32>,
    @location(1) wave_height: f32,
}

// ── Vertex ─────────────────────────────────────────────────────────────────

@vertex
fn vertex(
    @builtin(instance_index) instance_index: u32,
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
) -> WaterVertexOutput {
    let world_from_local = mesh_functions::get_world_from_local(instance_index);
    let flat_position = mesh_functions::mesh_position_local_to_world(
        world_from_local,
        vec4<f32>(position, 1.0),
    );

    let camera = view_bindings::view.world_position.xyz;
    // Displacement falls off with distance: far vertices sit on coarse clipmap
    // rings, where displacing them buys nothing but aliasing that crawls as the
    // mesh slides under the camera.
    let camera_distance = length(flat_position.xz - camera.xz);
    let distance_factor = min(
        exp(-(camera_distance - water.distance_scales.x) * water.distance_scales.y),
        1.0,
    );

    // Cascade UVs come from world XZ, so the waves stay put when the mesh
    // (a camera-following clipmap) moves under them.
    var displacement = vec3<f32>(0.0);
    for (var i = 0u; i < water.num_cascades; i += 1u) {
        let scales = water.map_scales[i];
        displacement += textureSampleLevel(
            displacement_maps,
            displacement_sampler,
            flat_position.xz * scales.xy,
            i32(i),
            0.0,
        ).xyz * scales.z;
    }
    displacement *= distance_factor;

    var out: WaterVertexOutput;
    let displaced = flat_position.xyz + displacement;
    out.clip_position = position_world_to_clip(displaced);
    out.world_position = vec4<f32>(displaced, 1.0);
    out.wave_height = displacement.y;
    return out;
}

// ── Bicubic filtering ──────────────────────────────────────────────────────

/// Filter weights for a cubic B-spline.
fn cubic_weights(a: f32) -> vec4<f32> {
    let a2 = a * a;
    let a3 = a2 * a;

    let w0 = -a3 + a2 * 3.0 - a * 3.0 + 1.0;
    let w1 = a3 * 3.0 - a2 * 6.0 + 4.0;
    let w2 = -a3 * 3.0 + a2 * 3.0 + a * 3.0 + 1.0;
    let w3 = a3;
    return vec4<f32>(w0, w1, w2, w3) / 6.0;
}

/// Bicubic B-spline filtering built from four bilinear taps.
/// Source: GPU Gems 2, chapter 20 — Fast Third-Order Texture Filtering.
fn texture_bicubic(uv: vec2<f32>, layer: i32) -> vec4<f32> {
    let dims = vec2<f32>(textureDimensions(normal_maps).xy);
    let dims_inv = 1.0 / dims;
    let scaled = uv * dims + 0.5;

    let fuv = fract(scaled);
    let wx = cubic_weights(fuv.x);
    let wy = cubic_weights(fuv.y);

    let g = vec4<f32>(wx.xz + wx.yw, wy.xz + wy.yw);
    let h = (vec4<f32>(wx.yw, wy.yw) / g + vec4<f32>(-1.5, 0.5, -1.5, 0.5)
        + floor(scaled).xxyy) * dims_inv.xxyy;
    let w = g.xz / (g.xz + g.yw);

    let top = mix(
        textureSample(normal_maps, normal_sampler, h.yw, layer),
        textureSample(normal_maps, normal_sampler, h.xw, layer),
        w.x,
    );
    let bottom = mix(
        textureSample(normal_maps, normal_sampler, h.yz, layer),
        textureSample(normal_maps, normal_sampler, h.xz, layer),
        w.x,
    );
    return mix(top, bottom, w.y);
}

// ── Lighting helpers ───────────────────────────────────────────────────────

/// Smith masking-shadowing term. Only the subsurface term uses it here; the
/// specular lobe comes from Bevy's PBR. (The reference passes its two arguments
/// the other way round — kept correct here, since the value only softens the
/// scattering falloff.)
fn smith_masking_shadowing(cos_theta: f32, alpha: f32) -> f32 {
    let a = cos_theta / (alpha * sqrt(max(1.0 - cos_theta * cos_theta, 1e-5)));
    let a_sq = a * a;
    if a < 1.6 {
        return (1.0 - 1.259 * a + 0.396 * a_sq) / (3.535 * a + 2.181 * a_sq);
    }
    return 0.0;
}

// ── Fragment ───────────────────────────────────────────────────────────────

@fragment
fn fragment(in: WaterVertexOutput) -> @location(0) vec4<f32> {
    let world_xz = in.world_position.xz;
    let camera = view_bindings::view.world_position.xyz;
    let dist = length(in.world_position.xyz - camera);
    let map_size = f32(textureDimensions(normal_maps).x);

    // Gradients and foam, summed across cascades.
    var gradient = vec3<f32>(0.0);
    for (var i = 0u; i < water.num_cascades; i += 1u) {
        let scales = water.map_scales[i];
        let coords = world_xz * scales.xy;
        // World-space pixels per metre for this cascade. Bicubic filtering is
        // worth it only while the map is stretched over the surface; once a
        // texel is smaller than a pixel, plain bilinear is both cheaper and
        // less prone to ringing.
        let ppm = map_size * min(scales.x, scales.y);
        let sample = mix(
            texture_bicubic(coords, i32(i)),
            textureSample(normal_maps, normal_sampler, coords, i32(i)),
            min(1.0, ppm * 0.1),
        );
        gradient += sample.xyw * vec3<f32>(scales.w, scales.w, 1.0);
    }

    // Foam sits in the third channel (the maps' alpha), already accumulated
    // over time by the simulation's Jacobian test.
    let foam_factor = smoothstep(0.0, 1.0, gradient.z * 0.75)
        * exp(-dist * water.distance_scales.w);
    let albedo = mix(water.water_color.rgb, water.foam_color.rgb, foam_factor);

    // Flatten the normal with distance: at range the per-pixel detail turns
    // into shimmer, so it is faded toward a flat surface.
    let slope = gradient.xy
        * mix(0.015, water.normal_strength, exp(-dist * water.distance_scales.z));
    let N = normalize(vec3<f32>(-slope.x, 1.0, -slope.y));
    let V = pbr_functions::calculate_view(in.world_position, false);
    let dot_nv = max(dot(N, V), 2e-5);

    let fresnel = mix(
        pow(1.0 - dot_nv, 5.0 * exp(-2.69 * water.roughness))
            / (1.0 + 22.7 * pow(water.roughness, 1.5)),
        1.0,
        REFLECTANCE,
    );
    // Rougher where there is foam, and where the surface faces the viewer.
    let shading_roughness = clamp((1.0 - fresnel) * foam_factor + 0.4, 0.0, 1.0);

    var pbr = pbr_input_new();
    // Diffuse is weighted by (1 - fresnel), as the reference's `light()` does:
    // light that reflected off the surface never entered the water, so it
    // cannot come back out as body colour. Scaling base_color is how to express
    // that here, because Bevy takes a dielectric's specular from `reflectance`
    // and only uses base_color for the diffuse lobe. Without it the water keeps
    // full diffuse at grazing angles and reads as flat plastic instead of going
    // dark and mirror-like toward the horizon.
    pbr.material.base_color = vec4<f32>(albedo * (1.0 - fresnel), 1.0);
    pbr.material.metallic = 0.0;
    pbr.material.perceptual_roughness = shading_roughness;
    pbr.material.reflectance = vec3<f32>(PBR_REFLECTANCE);
    pbr.frag_coord = in.clip_position;
    pbr.world_position = in.world_position;
    pbr.world_normal = N;
    pbr.N = N;
    pbr.V = V;
    var color = pbr_functions::apply_pbr_lighting(pbr).rgb;

    // Subsurface scattering: light passing through the crest of a wave toward
    // the viewer. Strongest looking into the sun, through a tall wave, at a
    // grazing angle — which is exactly when real water glows green.
    let L = normalize(-water.sun_direction.xyz);
    let sun_intensity = water.sun_direction.w;
    let sss_modifier = vec3<f32>(0.9, 1.15, 0.85);
    let sss_height = max(0.0, in.wave_height + 2.5)
        * pow(max(dot(L, -V), 0.0), 4.0)
        * pow(0.5 - 0.5 * dot(L, N), 3.0);
    let sss_near = 0.5 * pow(dot_nv, 2.0);
    let light_mask = smith_masking_shadowing(dot_nv, water.roughness);
    let sss = (sss_height + sss_near) * sss_modifier / (1.0 + light_mask);
    color += sss * albedo * (1.0 - fresnel) * sun_intensity;

    return vec4<f32>(color, 1.0);
}
