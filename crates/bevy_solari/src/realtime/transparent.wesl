// RENZORA PATCH: ray-traced see-through surfaces: glass, a car's clear-coat
// shell, a shop window.
//
// Solari lights the G-buffer, which holds the nearest opaque surface only. A
// blended surface was left to Bevy's forward transparent pass, which runs after
// Solari and has no ray tracing, so its only source of reflection was the sky's
// environment map: a car's paint and windows reflected sky where they stood on a
// road in a street of neon. And Renzora kept blended surfaces out of the traced
// scene altogether, since Solari traced every instance as solid.
//
// Now a blended surface is in the traced scene under its own ray mask
// (`RAY_MASK_SEE_THROUGH`), which every other ray ignores, and this pass lights
// it, per pixel, after the opaque image is finished:
//
//   1. trace from the camera to the opaque surface for each see-through
//      surface in front of it, nearest first, up to `MAX_LAYERS`;
//   2. light each: its reflection, traced into the scene and lit from Solari's
//      world cache (the environment map where it leaves the scene), the sun and
//      other lights' highlights, its own diffuse tint, and a clear coat's
//      sharp reflection over all of that;
//   3. blend them over the opaque image the way each one's alpha mode says, as
//      Bevy's forward pass would have.
//
// Bevy's forward pass no longer draws these surfaces on a Solari camera
// (renzora_solari's `SolariShadedTransparent`). It once drew only the nearest
// layer here, which left a headlight (clear lens, tinted lens, reflector) a
// black hole: what showed through the outer lens was the dark housing.

import bevy_pbr::render::pbr_functions::calculate_F0_dielectric;
import bevy_pbr::render::utils::rand_f;
import bevy_render::maths::{orthonormalize, PI};
import package::realtime::bindings::{constants, depth_buffer, view, view_output};
import package::realtime::gbuffer_utils::reconstruct_world_position;
import package::realtime::world_cache_query::{query_world_cache_cell, WORLD_CACHE_CELL_LIFETIME};
import package::realtime::debug_layout::{flush_ray_counts, SLOT_RAYS_INITIAL};
import package::scene::brdf::{evaluate_specular_brdf, fresnel, lobe_reflectances, F_AB};
import package::scene::sampling::{
    calculate_resolved_light_contribution, generate_random_light_sample, ggx_vndf_sample_invalid,
    resolve_light_sample, sample_environment_map_light, sample_ggx_vndf, sample_random_light,
    trace_visibility, LightSample,
};
import package::scene::bindings::{
    directional_lights, light_sources, material_ids, materials, resolve_ray_hit_full, samplers, textures,
    trace_ray_masked, LightSource, LIGHT_SOURCE_KIND_DIRECTIONAL, RAY_MASK_OPAQUE, RAY_MASK_SEE_THROUGH,
    RAY_T_MAX, RAY_T_MIN, TEXTURE_MAP_NONE,
};
import bevy_pbr::render::utils::rand_u;

enable wgpu_ray_query;

// `Material::blend`. Must match `BLEND_*` in scene/binder/assets.rs.
const BLEND_ALPHA: u32 = 1u;
const BLEND_PREMULTIPLIED: u32 = 2u;
const BLEND_ADD: u32 = 3u;
const BLEND_MULTIPLY: u32 = 4u;
const BLEND_MODE_MASK: u32 = 0xFFu;
const BLEND_CULL_BACK: u32 = 0x100u;
const BLEND_CULL_FRONT: u32 = 0x200u;

/// Above this perceptual roughness a reflection is blurred, and is averaged
/// from several rays rather than one.
const SHARP_PERCEPTUAL_ROUGHNESS: f32 = 0.15;
const BLURRED_REFLECTION_RAYS: u32 = 4u;

/// How far past the opaque surface a see-through one may lie and still count
/// as in front of it, as a fraction of the distance: a clear-coat shell can sit
/// exactly on the paint it covers.
const COPLANAR_TOLERANCE: f32 = 0.002;

/// Light arriving along `direction` at `origin`: the surface it reaches, lit
/// from the world cache, or the environment where it leaves the scene. Returned
/// in two parts, as Solari shades: what exposure applies to (x), and visible
/// emission, which it does not (see the emissive patch in restir.wesl).
struct Incoming {
    exposed: vec3<f32>,
    emissive: vec3<f32>,
}

fn incoming(origin: vec3<f32>, direction: vec3<f32>, rng: ptr<function, u32>) -> Incoming {
    let ray = trace_ray_masked(origin, direction, RAY_T_MIN, RAY_T_MAX, RAY_FLAG_NONE, RAY_MASK_OPAQUE);
    if ray.kind == RAY_QUERY_INTERSECTION_NONE {
        return Incoming(sample_environment_map_light(direction), vec3(0.0));
    }
    let hit = resolve_ray_hit_full(ray);
    // The world cache holds the light arriving at a surface (irradiance), so
    // what leaves it towards us is that times its colour over pi: a diffuse
    // approximation, which is what the cache can say about any surface, and
    // exactly how initial_path.wesl reads the cache at a path's last hit.
    // Without the pi, every reflection was over three times too bright and
    // glass and clear coat read as mirrors.
    let arriving = light_arriving(hit.world_position, hit.geometric_world_normal, ray.t, rng);
    return Incoming(hit.material.base_color / PI * arriving, hit.material.emissive);
}

/// Lookups [`cached_light`] averages.
const CACHE_LOOKUPS: u32 = 3u;

/// The light arriving at a point (irradiance): from the world cache where it
/// has settled cells there, and otherwise from one sample of the scene's
/// lights, which leaves out bounce light but needs no cell.
fn light_arriving(world_position: vec3<f32>, world_normal: vec3<f32>, ray_t: f32, rng: ptr<function, u32>) -> vec3<f32> {
    let cached = cached_light(world_position, world_normal, ray_t, rng);
    if cached.a > 0.0 {
        return cached.rgb;
    }
    if arrayLength(&light_sources) == 0u {
        return vec3(0.0);
    }
    let origin = world_position + world_normal * RAY_T_MIN;
    let light = sample_random_light(origin, world_normal, rng);
    return light.radiance * light.inverse_pdf * saturate(dot(world_normal, light.wi));
}

/// The world cache's light at a point in rgb, averaged over a few of its
/// jittered lookups, each weighed by how many samples its cell has gathered;
/// the total weight in a, zero where no cell has any.
///
/// The lookups only read: this pass never creates a cell, nor keeps one alive.
/// It once did both, several times at every point it lit, and filled the
/// cache to 95% of its capacity in the Bistro; lookups then collided and
/// failed, and the cache's per-frame update budget spread so thin that the
/// opaque image's bounce light went blotchy everywhere, a pavement nowhere
/// near any glass included. Read-only, glass cannot change what the rest of
/// the image gets from the cache.
///
/// Weighed by samples because the cache's other readers sit under the
/// denoiser, which hides a young cell (one random bounce ray), and this pass
/// does not: read evenly, young cells showed as splotches on a clear coat.
fn cached_light(world_position: vec3<f32>, world_normal: vec3<f32>, ray_t: f32, rng: ptr<function, u32>) -> vec4<f32> {
    var sum = vec3(0.0);
    var weight = 0.0;
    for (var i = 0u; i < CACHE_LOOKUPS; i++) {
        let cell = query_world_cache_cell(world_position, world_normal, view.world_position, ray_t, WORLD_CACHE_CELL_LIFETIME, false, rng);
        let w = min(cell.a, SETTLED_CELL_SAMPLES);
        sum += cell.rgb * w;
        weight += w;
    }
    return vec4(sum / max(weight, 1e-6), weight);
}

/// Samples past which a cache cell counts as settled, and is weighed no more
/// than any other settled one.
const SETTLED_CELL_SAMPLES: f32 = 16.0;

@compute @workgroup_size(8, 8, 1)
fn shade_see_through(@builtin(global_invocation_id) global_id: vec3<u32>) {
    shade_pixel(global_id.xy, false);
}

/// RENZORA PATCH: see-through surfaces in front of the sky, run after the sky
/// is drawn (see `solari_see_through_sky` in node.rs). Solari leaves pixels
/// with no opaque surface for the skybox or atmosphere to fill, and they fill
/// them after this crate's lighting, so a window or a sheer curtain seen
/// against the sky was drawn and then painted over: every curtain in front of
/// a window vanished.
@compute @workgroup_size(8, 8, 1)
fn shade_see_through_sky(@builtin(global_invocation_id) global_id: vec3<u32>) {
    shade_pixel(global_id.xy, true);
}

/// Composites the see-through surfaces along one pixel's ray over what is
/// already there: only the pixels with an opaque surface when `sky` is false,
/// only those without one when it is true.
fn shade_pixel(pixel_id: vec2<u32>, sky: bool) {
    let size = vec2<u32>(view.main_pass_viewport.zw);
    if any(pixel_id >= size) { return; }
    let pixel_index = pixel_id.x + pixel_id.y * size.x;
    var rng = pixel_index + constants.frame_rng + 0x3C6EF372u;

    // The camera ray through this pixel, as far as the opaque surface there.
    let pixel_uv = (vec2<f32>(pixel_id) + 0.5) / view.main_pass_viewport.zw;
    let pixel_ndc = pixel_uv * 2.0 - 1.0;
    let near = view.world_from_clip * vec4(pixel_ndc.x, -pixel_ndc.y, 1.0, 1.0);
    let direction = normalize(near.xyz / near.w - view.world_position);
    let depth = textureLoad(depth_buffer, pixel_id, 0);
    if sky != (depth == 0.0) { return; }
    var t_max = RAY_T_MAX;
    if depth > 0.0 {
        let opaque = reconstruct_world_position(pixel_id, depth, view.main_pass_viewport.zw, view.world_from_clip);
        t_max = distance(opaque, view.world_position) * (1.0 + COPLANAR_TOLERANCE);
    }

    // Every see-through surface along the ray, nearest first, up to
    // MAX_LAYERS: a headlight is a clear outer lens over a tinted inner one
    // over the reflector, and each sheet has a front face and a back face. Lit
    // and blended front to back: `light` gathers what each layer adds, and
    // `through` how much of what lies behind it still shows. That is the same
    // image as blending them back to front over the opaque one, without having
    // to keep every hit to shade in reverse.
    var light = vec3(0.0);
    var through = vec3(1.0);
    var t_min = 0.0;
    for (var layer = 0u; layer < MAX_LAYERS; layer++) {
        let hit = trace_ray_masked(view.world_position, direction, t_min, t_max, RAY_FLAG_NONE, RAY_MASK_SEE_THROUGH);
        if hit.kind == RAY_QUERY_INTERSECTION_NONE { break; }
        let shaded = shade_layer(hit, direction, &rng);
        light += through * shaded.light;
        through *= shaded.through;
        // Just past this hit, so the next search finds the next surface and
        // not this one again; relative, so it holds at any distance.
        t_min = hit.t * (1.0 + LAYER_SEPARATION) + RAY_T_MIN;
        if all(through < vec3(MIN_THROUGH)) { break; }
    }
    if any(through < vec3(1.0)) || any(light > vec3(0.0)) {
        let behind = textureLoad(view_output, pixel_id).rgb;
        textureStore(view_output, pixel_id, vec4(light + through * behind, 1.0));
    }

    flush_ray_counts(SLOT_RAYS_INITIAL, pixel_index);
}

/// Rays [`gather_diffuse`] spreads over a surface's facing hemisphere.
const DIFFUSE_GATHER_RAYS: u32 = 8u;

/// Below this much diffuse colour (times alpha) a layer's tint is too faint
/// to gather light for, and clear or metallic glass costs nothing extra.
const DIFFUSE_GATHER_MIN: f32 = 0.02;

/// How much of the light behind a diffuse see-through surface its threads pass
/// on toward the camera: half, as a thin fibre scatters about as much light
/// forward as back.
const DIFFUSE_TRANSLUCENCY: f32 = 0.5;

/// Samples of the scene's lights [`gather_diffuse`] adds for direct light.
const DIFFUSE_LIGHT_SAMPLES: u32 = 4u;

/// The light arriving at a see-through surface over its facing hemisphere,
/// as the mean radiance of cosine-weighted rays (irradiance over pi), in the
/// two parts [`Incoming`] keeps apart, plus the direct light of the scene's
/// lights, which a ray finds only by chance (a directional light never). The
/// directions and light samples are drawn from a hash of the surface position
/// rather than of the frame, so a still point on the surface gathers the same
/// light every frame: nothing denoises this pass, and fresh samples each frame
/// made it flicker. Tied to the surface, not the screen, the pattern does not
/// crawl as the camera moves.
fn gather_diffuse(origin: vec3<f32>, normal: vec3<f32>, world_position: vec3<f32>, rng: ptr<function, u32>) -> Incoming {
    let cell = vec3<i32>(floor(world_position * 200.0));
    var seed = (bitcast<u32>(cell.x) * 73856093u) ^ (bitcast<u32>(cell.y) * 19349663u) ^ (bitcast<u32>(cell.z) * 83492791u);
    let rotation = rand_f(&seed);
    // Every directional light is lit explicitly, and the random samples are
    // spent on emissive meshes alone. Drawn from the whole list, each sample
    // found the sun one time in as many lights as the scene held (every point
    // and spot light is an emissive stand-in), weighted up to match: with a
    // few lamps in a room, the points whose fixed samples happened to find
    // the sun shone at several times the rest, and a sunlit sheer curtain
    // was covered in white specks. A sample that lands on a directional light
    // counts as nothing, which keeps the emissive estimate unbiased.
    var directional = vec3(0.0);
    for (var d = 0u; d < arrayLength(&directional_lights); d++) {
        if all(directional_lights[d].luminance == vec3(0.0)) { continue; }
        let sample = resolve_light_sample(LightSample(0u, rand_u(&seed)), LightSource(LIGHT_SOURCE_KIND_DIRECTIONAL, d));
        let light = calculate_resolved_light_contribution(sample, origin, normal);
        directional += light.radiance * light.inverse_pdf * saturate(dot(normal, light.wi))
            * trace_visibility(origin, sample.world_position);
    }
    var emissive = vec3(0.0);
    if arrayLength(&light_sources) > 0u {
        for (var i = 0u; i < DIFFUSE_LIGHT_SAMPLES; i++) {
            let sample = generate_random_light_sample(&seed);
            if light_sources[sample.light_sample.light_id >> 16u].kind == LIGHT_SOURCE_KIND_DIRECTIONAL { continue; }
            let light = calculate_resolved_light_contribution(sample.resolved_light_sample, origin, normal);
            emissive += light.radiance * light.inverse_pdf * saturate(dot(normal, light.wi))
                * trace_visibility(origin, sample.resolved_light_sample.world_position);
        }
        emissive /= f32(DIFFUSE_LIGHT_SAMPLES);
    }
    let direct = (directional + emissive) / PI;
    let TBN = orthonormalize(normal);
    var sum = Incoming(vec3(0.0), vec3(0.0));
    for (var i = 0u; i < DIFFUSE_GATHER_RAYS; i++) {
        // Stratified in the polar coordinate, golden-angle spaced in the
        // azimuth, cosine-weighted.
        let u = (f32(i) + 0.5) / f32(DIFFUSE_GATHER_RAYS);
        let phi = 2.0 * PI * fract(f32(i) * 0.618034 + rotation);
        let r = sqrt(u);
        let local = vec3(r * cos(phi), r * sin(phi), sqrt(max(1.0 - u, 0.0)));
        let wi = local.x * TBN[0] + local.y * TBN[1] + local.z * TBN[2];
        let light = incoming(origin, wi, rng);
        sum.exposed += light.exposed;
        // Not `light.emissive`: an emissive surface is one of the scene's
        // lights, and the light samples above already count it. Added here
        // too, a lamp counted twice, and the rare ray that met a small bright
        // one (a light panel, a point light's stand-in sphere) lit its point
        // many times over. The rays are fixed per point, so those points
        // stayed lit: still, bright specks over sheer curtains and tinted
        // glass that no denoiser sees, since this pass runs after it.
    }
    return Incoming(sum.exposed / f32(DIFFUSE_GATHER_RAYS) + direct, sum.emissive / f32(DIFFUSE_GATHER_RAYS));
}

/// See-through surfaces drawn along one pixel's ray, at most.
const MAX_LAYERS: u32 = 4u;

/// How far past a layer the search for the next starts, as a fraction of its
/// distance.
const LAYER_SEPARATION: f32 = 0.0001;

/// Below this much of what is behind still showing, the layers further back
/// are not looked for: nothing they add could be seen.
const MIN_THROUGH: f32 = 0.002;

/// What one see-through layer does to the pixel: the light it adds, and how
/// much of what lies behind it shows through, per channel.
struct Layer {
    light: vec3<f32>,
    through: vec3<f32>,
}

fn shade_layer(front: RayIntersection, direction: vec3<f32>, rng: ptr<function, u32>) -> Layer {
    let surface = resolve_ray_hit_full(front);
    let record = materials[material_ids[front.instance_custom_data]];
    var alpha = record.alpha;
    if record.base_color_texture_id != TEXTURE_MAP_NONE {
        alpha *= textureSampleLevel(textures[record.base_color_texture_id], samplers[record.base_color_texture_id], surface.uv, 0.0).a;
    }
    alpha = saturate(alpha);

    // Glass is seen from either side: light the side facing the camera.
    let wo = -direction;
    let facing = select(-1.0, 1.0, dot(surface.geometric_world_normal, wo) >= 0.0);
    // A face the material's raster draw culls is not drawn here either: a ray
    // meets every face, so a single-sided pane's back face would otherwise be
    // a second layer of tint that Bevy never draws.
    let culled = ((record.blend & BLEND_CULL_BACK) != 0u && facing < 0.0)
        || ((record.blend & BLEND_CULL_FRONT) != 0u && facing > 0.0);
    if culled {
        return Layer(vec3(0.0), vec3(1.0));
    }
    let geometric_normal = surface.geometric_world_normal * facing;
    var normal = surface.world_normal * facing;
    if dot(normal, wo) <= 0.0 {
        normal = geometric_normal;
    }
    let origin = surface.world_position + geometric_normal * RAY_T_MIN;
    let material = surface.material;
    let NdotV = max(dot(normal, wo), 0.0001);
    let F_ab = F_AB(material.perceptual_roughness, NdotV);
    let rho = lobe_reflectances(material.base_color, calculate_F0_dielectric(vec3(material.reflectance)), material, F_ab);

    // The reflection: one ray along the mirror direction, or several spread
    // over the specular lobe for a blurred one. Sampling the lobe's visible
    // normals weighs each by the lobe already, so their average times the
    // lobe's reflectance is the reflection.
    let TBN = orthonormalize(normal);
    let wo_tangent = vec3(dot(wo, TBN[0]), dot(wo, TBN[1]), dot(wo, TBN[2]));
    let rays = select(1u, BLURRED_REFLECTION_RAYS, material.perceptual_roughness > SHARP_PERCEPTUAL_ROUGHNESS);
    var reflected = Incoming(vec3(0.0), vec3(0.0));
    for (var i = 0u; i < rays; i++) {
        var wi = reflect(direction, normal);
        if rays > 1u {
            let wi_tangent = sample_ggx_vndf(wo_tangent, material.roughness, rng);
            if !ggx_vndf_sample_invalid(wi_tangent) {
                wi = wi_tangent.x * TBN[0] + wi_tangent.y * TBN[1] + wi_tangent.z * TBN[2];
            }
        }
        let light = incoming(origin, wi, rng);
        reflected.exposed += light.exposed;
        reflected.emissive += light.emissive;
    }
    reflected.exposed /= f32(rays);
    reflected.emissive /= f32(rays);

    // Highlights of the sun and the other lights, which a reflection ray
    // rarely finds on its own.
    var highlight = vec3(0.0);
    if arrayLength(&light_sources) > 0u {
        let light = sample_random_light(origin, normal, rng);
        highlight = evaluate_specular_brdf(wo, light.wi, normal, material, F_ab) * light.radiance * light.inverse_pdf;
    }

    // Its own tint, lit by what arrives at it. A surface with a real diffuse
    // colour (a sheer curtain, frosted glass) gathers that light with rays of
    // its own: read from the world cache at its own position, it found nothing,
    // since opaque rays pass through see-through surfaces and leave no cells
    // there, and its fallback was one random light sample a frame, which left
    // out the sky. A sky-lit curtain came out near black and flickered in and
    // out, with no denoiser after this pass to steady it.
    var own_light = Incoming(light_arriving(surface.world_position, geometric_normal, front.t, rng) / PI, vec3(0.0));
    if max(rho.diffuse.r, max(rho.diffuse.g, rho.diffuse.b)) * alpha > DIFFUSE_GATHER_MIN {
        own_light = gather_diffuse(origin, geometric_normal, surface.world_position, rng);
        // A sheer fabric's threads also scatter the light reaching them from
        // behind, which is most of what a curtain in a window shows: lit from
        // the room side alone it only dimmed the sky by its alpha.
        let back_origin = surface.world_position - geometric_normal * RAY_T_MIN;
        let back = gather_diffuse(back_origin, -geometric_normal, surface.world_position, rng);
        own_light.exposed += DIFFUSE_TRANSLUCENCY * back.exposed;
        own_light.emissive += DIFFUSE_TRANSLUCENCY * back.emissive;
    }

    var lit = (rho.diffuse * own_light.exposed + rho.specular * reflected.exposed + highlight) * view.exposure
        + rho.diffuse * own_light.emissive
        + rho.specular * reflected.emissive
        + material.emissive;

    // A clear coat: a smooth lacquer over the surface, reflecting with a
    // plain dielectric's Fresnel and dimming what is under it by as much, as
    // Bevy's forward pass layers it. A car window's crisp reflection is this
    // layer: the glass under it is rough and darkly tinted, so without it the
    // window showed only a blurred sheen. Taken as mirror-smooth, which every
    // clear coat in practice nearly is; one ray, shared with the base when its
    // own reflection was a single mirror ray already.
    let clearcoat = record.clearcoat;
    if clearcoat > 0.0 {
        let coat_fresnel = clearcoat * fresnel(vec3(0.04), NdotV).x;
        var coat = reflected;
        if rays > 1u {
            coat = incoming(origin, reflect(direction, normal), rng);
        }
        lit = lit * (1.0 - coat_fresnel) + coat_fresnel * (coat.exposed * view.exposure + coat.emissive);
    }

    // Each alpha mode's blend as Bevy's forward pass does it, written as what
    // the layer adds and what it lets through (`behind` becomes `through *
    // behind`): alpha mixes, premultiplied adds its own light whole, add
    // keeps everything behind, multiply tints it.
    switch record.blend & BLEND_MODE_MASK {
        case BLEND_PREMULTIPLIED: { return Layer(lit, vec3(1.0 - alpha)); }
        case BLEND_ADD: { return Layer(lit * alpha, vec3(1.0)); }
        case BLEND_MULTIPLY: { return Layer(vec3(0.0), mix(vec3(1.0), lit, alpha)); }
        default: { return Layer(lit * alpha, vec3(1.0 - alpha)); }
    }
}
