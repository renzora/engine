// Volumetric clouds — a raymarch through a spherical cloud deck wrapped around
// a virtual planet, after "The Real-Time Volumetric Cloudscapes of Horizon Zero
// Dawn" (Schneider/Vos) and "Physically Based Sky, Atmosphere and Cloud
// Rendering in Frostbite" (Hillaire).
//
// This replaces an earlier 2.5D dome that painted FBM straight onto a sphere.
// The difference that matters is that density here is a *field with thickness*:
// a second march toward the sun gives every sample its own shadow, so clouds
// have bright sunward faces and dark undersides instead of a fake vertical
// gradient, and the deck curves down to the horizon on its own because the
// march is against real shell geometry rather than a painted gradient.
//
// The mesh is still a dome centred on the camera, but only as a way to get one
// fragment per sky pixel — nothing about the shading uses its surface. The
// march starts at the camera, so flying up through the deck works: the entry
// interval is solved for all three cases (below, inside, above).
//
// Everything here is in KILOMETRES with y = altitude above the surface. At a
// 6371 km planet radius, `length(planet_space_position) - radius` in metres
// loses the whole 1.15 km deck to f32 rounding; kilometres plus the analytic
// height expansion in `height_at` keeps every intermediate small.

#import bevy_pbr::forward_io::VertexOutput
#import bevy_pbr::mesh_view_bindings::view

struct CloudsUniform {
    // xyz = unit vector toward the sun, w unused.
    sun_direction: vec4<f32>,
    // rgb = sun radiance reaching the deck, w unused.
    sun_color: vec4<f32>,
    // rgb = skylight filling the top of the deck, w unused.
    ambient_top: vec4<f32>,
    // rgb = skylight filling the base of the deck, w unused.
    ambient_bottom: vec4<f32>,
    // rgb = horizon haze in the sun's half of the sky, a = how strongly it
    // takes over near the horizon.
    haze_sunward: vec4<f32>,
    // rgb = horizon haze opposite the sun, w unused.
    haze_away: vec4<f32>,
    // xyz = accumulated wind displacement, km. Applied to the *sample*
    // position, so the deck drifts without the ray geometry moving with it.
    wind_offset: vec4<f32>,
    // xy = the warp field's own scroll in km, z = the detail volume's phase in
    // whole turns. Both drive shape evolution; see `shape_warp`.
    morph_offset: vec4<f32>,

    planet_radius: f32,
    bottom_height: f32,
    top_height: f32,
    base_scale: f32,
    detail_scale: f32,
    coverage: f32,
    // Extinction per km at full density.
    extinction: f32,
    detail_strength: f32,
    edge_softness: f32,
    base_softness: f32,
    powder_strength: f32,
    min_transmittance: f32,
    forward_scattering: f32,
    backward_scattering: f32,
    scattering_blend: f32,
    view_steps: u32,
    shadow_steps: u32,
    // 1 in daylight, 0 at night. Fades the deck out rather than leaving an
    // unlit silhouette punched through the night sky.
    day_factor: f32,
}

@group(3) @binding(0) var<uniform> clouds: CloudsUniform;
@group(3) @binding(1) var base_texture: texture_2d<f32>;
@group(3) @binding(2) var base_sampler: sampler;
@group(3) @binding(3) var detail_texture: texture_3d<f32>;
@group(3) @binding(4) var detail_sampler: sampler;

// Ceiling on the span a view ray is considered to cover: it bounds the step size
// below, and stands in as the distance for haze on a ray that met no cloud at
// all. Near the horizon the deck's chord runs for hundreds of km, and by this
// distance the haze has taken the deck over completely anyway.
const MAX_MARCH_KM: f32 = 120.0;

// The view march steps geometrically rather than uniformly. A uniform step has
// to choose between resolving the cloud the camera is *inside* and reaching the
// one on the horizon: fly into the deck and a step sized for the horizon puts
// two samples between you and the far side, so the envelope you should be
// swimming through turns into a flat wall. Growing the step spends resolution
// where the eye is and distance where it is not — which is also the order in
// which the haze stops caring.
//
// The start is capped as well as the growth, so a ray whose span runs for a
// hundred km still begins with fine samples instead of scaling its whole
// schedule up.
const VIEW_STEP_GROWTH: f32 = 1.12;
const VIEW_STEP_START: f32 = 0.05;
const VIEW_STEP_MAX: f32 = 0.9;

// The sun march steps a fixed fraction of the deck's thickness, growing each
// step. Six default steps at 1% reach roughly 8% of the thickness before the
// density has saturated the transmittance, which is enough to separate a lit
// face from a shadowed one; tying it to the thickness means a stylised 8 km-deep
// deck gets a proportionally longer march instead of a useless 100 m one.
const SHADOW_STEP_FRACTION: f32 = 0.01;
const SHADOW_STEP_GROWTH: f32 = 1.3;
// Floor on the sun's slope when stretching that march. Without it a sun on the
// horizon asks for an unbounded path and every cloud in the sky turns black.
const SHADOW_MIN_SLOPE: f32 = 0.12;

// Multiple scattering, after Wrenninge: each octave carries less light, is
// extincted less (so it reaches deeper into the cloud), and scatters more
// isotropically. Without it a single Henyey-Greenstein lobe spans a 400:1 range
// across the sky, and everything more than about 60 degrees off the sun is lit
// by ambient alone — which is exactly what a flat grey cloud looks like.
const MS_OCTAVES: u32 = 3u;
const MS_ATTENUATION: f32 = 0.5;
const MS_EXTINCTION: f32 = 0.5;
const MS_ECCENTRICITY: f32 = 0.5;

// Distance over which haze accumulates, and over which the erosion detail fades
// out. The detail volume is 32 cells repeating every few hundred metres: close
// up that reads as wisps, but a few km out it is below a pixel and all that
// survives is the repeat, tiled across the horizon. Fading it is both the fix
// for that and the cheapest thing in the march to stop paying for.
const HAZE_DISTANCE_KM: f32 = 45.0;
const DETAIL_FADE_KM: f32 = 14.0;

// How much wider than the base silhouette the coverage modulation runs, and the
// rotation applied to it. See `weather`.
const WEATHER_SPREAD: f32 = 11.0;
const WEATHER_ROTATION: vec2<f32> = vec2<f32>(0.8624, 0.5062);

// Shape evolution. Wind alone only *translates* the deck, and a cloud whose
// silhouette never changes reads as a cutout sliding across the sky however
// fast it moves. A vector field displaces the base sample position, and that
// field scrolls along its own axis at its own speed — so at any fixed point in
// the world the displacement is changing, and clouds deform in place rather
// than merely passing through. It runs at a third of the silhouette's frequency:
// low enough that one cloud spans several times its own width of it and
// stretches and folds, rather than just shifting whole.
//
// `WARP_SPREAD` is duplicated in `lib.rs`, which needs it to wrap the scroll.
const WARP_SPREAD: f32 = 3.0;
// Displacement at full swing, as a fraction of the base silhouette's period.
const WARP_FRACTION: f32 = 0.16;

// ── Small helpers ────────────────────────────────────────────────────────────

fn linear_step(lo: f32, hi: f32, v: f32) -> f32 {
    return clamp((v - lo) / (hi - lo), 0.0, 1.0);
}

fn remap(v: f32, lo: f32, hi: f32) -> f32 {
    return (v - lo) / (hi - lo);
}

fn hash13(p_in: vec3<f32>) -> f32 {
    var p = fract(p_in * 1031.1031);
    p += dot(p, p.yzx + 19.19);
    return fract((p.x + p.y) * p.z);
}

// Henyey-Greenstein phase, without the 1/4pi — the radiance here is an artistic
// scale, not a calibrated one, and folding the normalisation in would only mean
// multiplying the sun colour back up by 4pi.
fn henyey_greenstein(cos_theta: f32, g: f32) -> f32 {
    let g2 = g * g;
    return (1.0 - g2) / pow(max(1.0 + g2 - 2.0 * g * cos_theta, 1e-4), 1.5);
}

// ── Shell geometry ───────────────────────────────────────────────────────────

struct Roots {
    near: f32,
    far: f32,
    hit: bool,
}

// Intersect a ray leaving radius `r0` at cosine-to-local-up `mu` with the
// concentric sphere of radius `r`.
//
// Solved in the stable quadratic form rather than the textbook one: for the
// near-horizontal rays that cover most of a sky, the linear term and the
// discriminant agree to six digits, and subtracting them for the near root
// cancels away everything f32 had. `r0^2 - r^2` is likewise written as a
// product, because at a planetary radius the two squares agree to seven digits
// on their own.
fn sphere_roots(r0: f32, mu: f32, r: f32) -> Roots {
    let b = r0 * mu;
    let c = (r0 - r) * (r0 + r);
    let disc = b * b - c;
    if disc < 0.0 {
        return Roots(0.0, 0.0, false);
    }
    let sq = sqrt(disc);
    let q = -(b + select(-sq, sq, b >= 0.0));
    if abs(q) < 1e-9 {
        return Roots(0.0, 0.0, false);
    }
    let t0 = q;
    let t1 = c / q;
    return Roots(min(t0, t1), max(t0, t1), true);
}

// The span of the view ray that lies inside the deck. `y <= x` means this pixel
// sees no clouds at all — below the horizon, blocked by the planet, or (from
// above the deck) pointing up and away from it.
fn layer_interval(r0: f32, mu: f32) -> vec2<f32> {
    let miss = vec2<f32>(1.0, -1.0);
    let rb = clouds.planet_radius + clouds.bottom_height;
    let rt = clouds.planet_radius + clouds.top_height;

    let bottom = sphere_roots(r0, mu, rb);
    let top = sphere_roots(r0, mu, rt);
    let ground = sphere_roots(r0, mu, clouds.planet_radius);

    var t0 = 0.0;
    var t1 = 0.0;

    if r0 < rb {
        // Under the deck. Inside both shells, so each has exactly one root
        // ahead of the ray: in through the base, out through the top.
        t0 = bottom.far;
        t1 = top.far;
    } else if r0 > rt {
        // Above the deck — only rays angled back down ever reach it.
        if !top.hit || top.near <= 0.0 {
            return miss;
        }
        t0 = top.near;
        t1 = select(top.far, bottom.near, bottom.hit && bottom.near > 0.0);
    } else {
        // Inside the deck. Start at the eye and leave through whichever shell
        // comes first — this is the case that lets a camera fly through cloud.
        t0 = 0.0;
        t1 = select(top.far, bottom.near, bottom.hit && bottom.near > 0.0);
    }

    // The planet itself is opaque, so a ray that meets the ground stops there.
    // This is also what removes the clouds below the horizon: from under the
    // deck a downward ray hits the ground long before the base shell, which
    // collapses the interval to nothing.
    if ground.hit && ground.near > 0.0 {
        t1 = min(t1, ground.near);
    }

    return vec2<f32>(max(t0, 0.0), t1);
}

// Altitude above the surface at distance `t` along the ray, from the two-term
// expansion of sqrt(r0^2 + 2*r0*mu*t + t^2) - planet_radius.
//
// The leading `a + mu*t` is the flat-earth answer and the quadratic term is the
// curvature drop; the first term dropped is on the order of t^4/r0^3, which at
// the ~175 km tangent distance of a 2.4 km deck is under half a metre.
fn height_at(a: f32, mu: f32, r0: f32, t: f32) -> f32 {
    return a + mu * t + (1.0 - mu * mu) * t * t / (2.0 * r0);
}

// ── Density field ────────────────────────────────────────────────────────────

// `p` is a world-space position in km with y = altitude; `h` is the same point's
// position through the deck, 0 at the base and 1 at the top.
//
// Both textures tile, so the wind offset can grow without ever leaving them and
// the sample coordinates never need wrapping by hand.
fn cloud_base(p: vec3<f32>, h: f32, warp: vec2<f32>) -> f32 {
    let uv = (p.xz + warp + clouds.wind_offset.xz) * (0.05 * clouds.base_scale);
    let c = textureSampleLevel(base_texture, base_sampler, uv, 0.0).rgb;

    // Guerrilla's height-remap: subtracting a profile that is small in the
    // middle of the deck and large at both ends before the remap is what makes
    // bases flat and tops cauliflower, from a map with no vertical detail.
    let profile = h * h * c.b + pow(1.0 - h, 16.0);
    return remap(c.r - profile, c.g, 1.0);
}

fn cloud_detail(p: vec3<f32>) -> f32 {
    // The reference's 1/32 is folded in here because the volume is sampled in
    // normalised coordinates, not texels — which is also what buys the trilinear
    // filtering it had to fake with a manual lerp along one axis.
    let q = (p + clouds.wind_offset.xyz)
        * (1.6 * clouds.base_scale * clouds.detail_scale / 32.0);
    // Walking the volume's third axis over time makes the erosion evolve instead
    // of being carried along rigidly — the fine-grained half of the morph, and
    // free, because the volume is 3D whether or not anything moves through it.
    let morph = vec3<f32>(0.0, clouds.morph_offset.z, 0.0);
    return textureSampleLevel(detail_texture, detail_sampler, q + morph, 0.0).r;
}

// Erodes a little off both ends of the deck so it does not terminate in a slab.
fn deck_gradient(h: f32) -> f32 {
    return linear_step(0.0, 0.1, h) - linear_step(0.8, 1.2, h);
}

// Low-frequency coverage modulation — a weather map, in the Guerrilla sense.
//
// The base atlas repeats every ~13 km of world at the default scale, which at
// cloud altitude is close enough together to read as wallpaper along the
// horizon. Sampling the same atlas an order of magnitude wider, and rotated so
// the two lattices never line up, gives every repeat of the silhouette a
// different amount of cloud in it and the eye stops finding the period. The two
// together only come back into phase after a thousand km.
//
// Sampled once per *view* step and carried into the sun march rather than being
// folded into `cloud_density`: it varies over tens of km, so it is constant to
// well inside a texel across the ~150 m the sun march covers, and doing it there
// would multiply this fetch by seven.
fn weather(xz: vec2<f32>) -> f32 {
    let rotated = vec2<f32>(
        xz.x * WEATHER_ROTATION.x - xz.y * WEATHER_ROTATION.y,
        xz.x * WEATHER_ROTATION.y + xz.y * WEATHER_ROTATION.x,
    );
    let uv = (rotated + clouds.wind_offset.xz)
        * (0.05 * clouds.base_scale / WEATHER_SPREAD);
    let c = textureSampleLevel(base_texture, base_sampler, uv, 0.0).r;
    // Centred on 1 so the authored coverage still means what it says on average,
    // but swinging wide enough on either side to open real gaps and pile up real
    // banks. A timid range here leaves an even blanket, which is the look that
    // gives the repeat away however well the noise itself tiles.
    return clamp(0.35 + 1.3 * c, 0.0, 1.65);
}

// The displacement applied to the base silhouette, in km. See the note on
// `WARP_SPREAD`. Like `weather`, sampled once per view step.
fn shape_warp(xz: vec2<f32>) -> vec2<f32> {
    let uv = (xz + clouds.morph_offset.xy)
        * (0.05 * clouds.base_scale / WARP_SPREAD);
    let c = textureSampleLevel(base_texture, base_sampler, uv, 0.0).gb;
    // `g` is the remap window, in [-1, 0]; `b` is the height modifier, in [0, 1].
    // Neither was authored as a vector field, but reading two decorrelated
    // channels as one is a free way to get a smooth pseudo-random displacement,
    // and the atlas is already in the cache.
    let swing = vec2<f32>(c.x + 0.5, c.y - 0.5) * 2.0;
    return swing * (WARP_FRACTION / (0.05 * clouds.base_scale));
}

// Normalised density, 0..1. Extinction is applied by the callers so this stays
// a pure shape function that both marches can share.
//
// `coverage` and `detail_fade` come from the view step this sample belongs to,
// for the reasons above.
fn cloud_density(
    p: vec3<f32>,
    h: f32,
    coverage: f32,
    warp: vec2<f32>,
    detail_fade: f32,
) -> f32 {
    var m = cloud_base(p, h, warp) * deck_gradient(h);

    // Erode only where the base shape is thin. Detail carved into a dense core
    // would punch holes through the middle of a cloud rather than fray its rim.
    let erosion = smoothstep(1.0, 0.5, m) * detail_fade;
    if erosion > 0.0 {
        m -= cloud_detail(p) * erosion * clouds.detail_strength;
    }

    // Coverage slides the whole field through the smoothstep window: 0 puts
    // everything below it (clear sky), 1 puts everything above (overcast).
    m = smoothstep(0.0, clouds.edge_softness, m + coverage - 1.0);
    m *= min(h / clouds.base_softness, 1.0);
    return clamp(m, 0.0, 1.0);
}

// Optical depth from a sample toward the sun — the term that gives a cloud a
// lit face and a shadowed one, and the single largest realism cue in the model.
//
// Depth rather than transmittance, because the multiple-scattering octaves each
// attenuate the *same* path at a different rate; re-marching it once per octave
// would be three times the fetches for a number that is already in hand.
//
// Steps grow geometrically: the near samples decide the rim lighting and need
// resolution, the far ones only need to notice that a neighbouring cloud is in
// the way.
fn sun_optical_depth(
    origin: vec3<f32>,
    thickness: f32,
    coverage: f32,
    warp: vec2<f32>,
    detail_fade: f32,
) -> f32 {
    let sun = clouds.sun_direction.xyz;
    // Stretch the march as the sun drops. Light from overhead crosses the deck
    // by the shortest path there is; light from 20 degrees up crosses three
    // times as much cloud to reach the same point, and that is the whole reason
    // a midday sky is bright and flat while an evening one is deep and modelled.
    // A march of fixed length would sample the same 150 m at every sun angle and
    // throw all of that away — the sun's height would change the light's colour
    // and nothing about its shape.
    var step_km = thickness * SHADOW_STEP_FRACTION / max(sun.y, SHADOW_MIN_SLOPE);
    var t = step_km * 0.5;
    var optical_depth = 0.0;

    for (var i = 0u; i < clouds.shadow_steps; i += 1u) {
        // Local up is +Y to well inside a texel over the few km this march
        // covers — curvature only matters across the whole view ray.
        let p = origin + sun * t;
        let h = (p.y - clouds.bottom_height) / thickness;
        if h > 1.0 || h < 0.0 {
            break;
        }
        optical_depth += cloud_density(p, h, coverage, warp, detail_fade) * step_km;
        step_km *= SHADOW_STEP_GROWTH;
        t += step_km;
    }

    return optical_depth * clouds.extinction;
}

// ── Fragment ─────────────────────────────────────────────────────────────────

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    // The dome is centred on the camera, so the view direction is the offset
    // from the eye to this fragment — not the fragment's own position, which
    // would shear the whole sky once the camera leaves the world origin.
    let dir = normalize(in.world_position.xyz - view.world_position);

    let camera_km = view.world_position * 0.001;
    let altitude = camera_km.y;
    let r0 = clouds.planet_radius + altitude;
    let mu = dir.y;

    if clouds.day_factor < 0.002 {
        return vec4<f32>(0.0);
    }

    let span = layer_interval(r0, mu);
    if span.y <= span.x {
        // Premultiplied transparent black is a true no-op against the blend
        // equation, and returning it lets the shader leave here — `discard` in
        // WGSL only flags the invocation, it does not stop it.
        return vec4<f32>(0.0);
    }

    let thickness = clouds.top_height - clouds.bottom_height;
    let march = min(span.y - span.x, MAX_MARCH_KM);
    let steps = max(clouds.view_steps, 1u);

    // Size the first step so a geometric schedule of `steps` of them would cover
    // the whole span, then cap it: see the note on VIEW_STEP_GROWTH.
    let growth_sum =
        (pow(VIEW_STEP_GROWTH, f32(steps)) - 1.0) / (VIEW_STEP_GROWTH - 1.0);
    var dt = min(march / growth_sum, thickness * VIEW_STEP_START);

    // Dither the ray start so the step boundaries do not read as concentric
    // bands. The hash is of the view direction, not of time or screen position,
    // so the pattern is pinned to the sky: it neither crawls as the camera turns
    // nor flickers frame to frame, which leaves it in a form TAA can resolve.
    var t = span.x + dt * hash13(dir * 4096.0);

    // Frostbite's dual-lobe phase, once per multiple-scattering octave. The
    // phase depends only on the angle to the sun, so all of it is hoisted out of
    // the march rather than recomputed at every sample.
    let cos_theta = dot(dir, clouds.sun_direction.xyz);
    var phases: array<f32, 3>;
    var eccentricity = 1.0;
    for (var octave = 0u; octave < MS_OCTAVES; octave += 1u) {
        phases[octave] = mix(
            henyey_greenstein(cos_theta, clouds.forward_scattering * eccentricity),
            henyey_greenstein(cos_theta, clouds.backward_scattering * eccentricity),
            clouds.scattering_blend,
        );
        eccentricity *= MS_ECCENTRICITY;
    }

    var scattered = vec3<f32>(0.0);
    var transmittance = 1.0;
    // Distance to the first cloud the ray meets, for the haze below. -1 = none.
    var hit_distance = -1.0;

    for (var i = 0u; i < steps; i += 1u) {
        // The schedule can overshoot the exit, and `h` is clamped, so a sample
        // past the top of the deck would still report the gradient's value there
        // rather than nothing at all.
        if t > span.y {
            break;
        }

        let h_km = height_at(altitude, mu, r0, t);
        let h = clamp((h_km - clouds.bottom_height) / thickness, 0.0, 1.0);
        let p = vec3<f32>(camera_km.x + dir.x * t, h_km, camera_km.z + dir.z * t);

        // All three are constant across this sample's sun march (see `weather`).
        let coverage = clamp(clouds.coverage * weather(p.xz), 0.0, 1.0);
        let warp = shape_warp(p.xz);
        let detail_fade = 1.0 - clamp(t / DETAIL_FADE_KM, 0.0, 1.0);

        let density = cloud_density(p, h, coverage, warp, detail_fade);
        if density > 0.0 {
            if hit_distance < 0.0 {
                hit_distance = t;
            }

            let sigma = density * clouds.extinction;
            let step_transmittance = exp(-sigma * dt);
            let sun_depth = sun_optical_depth(p, thickness, coverage, warp, detail_fade);

            // Multiple scattering, as a sum of octaves over the one sun path:
            // each carries less light, is extincted less, and scatters more
            // isotropically than the last. The deepest octave is what keeps a
            // cloud's shaded body lit instead of collapsing to the ambient term.
            var direct = 0.0;
            var attenuation = 1.0;
            var extinction = 1.0;
            for (var octave = 0u; octave < MS_OCTAVES; octave += 1u) {
                direct += attenuation * exp(-sun_depth * extinction) * phases[octave];
                attenuation *= MS_ATTENUATION;
                extinction *= MS_EXTINCTION;
            }

            // Powder: a thin sunlit edge has had little chance to scatter light
            // back toward the eye yet, so it reads darker than its density
            // alone suggests. Without it, cloud rims look like cut paper.
            //
            // Measured over a fixed slice of the deck rather than over `dt`,
            // which is thirty times longer at the horizon than overhead — keying
            // it to the step would darken distant cloud for no reason but the
            // step size the march happened to pick.
            let powder = 1.0 - exp(-sigma * thickness * 0.1);
            let sunlight =
                clouds.sun_color.rgb * direct * mix(1.0, powder, clouds.powder_strength);
            let skylight = mix(clouds.ambient_bottom.rgb, clouds.ambient_top.rgb, h);

            // Hillaire's energy-conserving integration: integrate the source
            // term across the whole segment analytically instead of sampling it
            // at one point and multiplying by dt, which is what keeps the result
            // stable as the step size changes from zenith to horizon.
            scattered += transmittance * (sunlight + skylight) * (1.0 - step_transmittance);
            transmittance *= step_transmittance;

            if transmittance < clouds.min_transmittance {
                transmittance = 0.0;
                break;
            }
        }

        t += dt;
        dt = min(dt * VIEW_STEP_GROWTH, thickness * VIEW_STEP_MAX);
    }

    let alpha = 1.0 - transmittance;
    if alpha < 0.002 {
        return vec4<f32>(0.0);
    }

    // Atmospheric perspective, by distance rather than by elevation. Distant
    // cloud is seen through more air than near cloud, and the depth of air is
    // what the eye reads as depth of field in a sky. Keying it to how far the
    // ray actually travelled before hitting cloud — rather than to how low in
    // the sky the pixel is — also does the work of hiding the base map's
    // repeat, which only becomes visible at the distances haze is thickest at.
    let distance_km = select(march, hit_distance, hit_distance >= 0.0);
    let haze = clamp(
        (1.0 - exp(-distance_km / HAZE_DISTANCE_KM)) * clouds.haze_sunward.a,
        0.0,
        1.0,
    );

    // The haze colour is sampled from the atmosphere at two bearings — toward
    // the sun and away from it — and blended by azimuth, because at sunrise and
    // sunset the two halves of the horizon are not remotely the same colour and
    // a single value paints the whole rim the same shade of orange. `dir_xz`
    // vanishes at the zenith, where haze is thinnest anyway; the max() keeps a
    // NaN from propagating out of that corner.
    let dir_xz = vec2<f32>(dir.x, dir.z);
    let sun_xz = vec2<f32>(clouds.sun_direction.x, clouds.sun_direction.z);
    let alignment = clamp(
        dot(dir_xz, sun_xz) / max(length(dir_xz) * length(sun_xz), 1e-4),
        -1.0,
        1.0,
    );
    let haze_color = mix(clouds.haze_away.rgb, clouds.haze_sunward.rgb, 0.5 + 0.5 * alignment);

    // The colours are premultiplied, so the haze is scaled by alpha to stay in
    // that space, and the night fade scales the whole result.
    let color = mix(scattered, haze_color * alpha, haze);
    return vec4<f32>(color, alpha) * clouds.day_factor;
}
