// RENZORA PATCH: shared by the lighting passes, which write the split, demodulated
// signal the denoiser filters, and by the denoiser's composite, which multiplies
// the material back in. Both sides must compute the same factors, so they live
// here once.

import bevy_core_pipeline::tonemapping::tonemapping_luminance as luminance;
import bevy_pbr::render::pbr_functions::{calculate_diffuse_color, calculate_F0};
import package::scene::bindings::ResolvedMaterial;

/// The smallest factor a signal is divided by. A black or fully metallic surface
/// has a diffuse albedo of 0, and dividing by it would turn the faint light that
/// still reaches it into noise of any size. Remodulating by the same floored
/// factor makes the round trip exact, so the floor costs nothing but filtering
/// quality on surfaces that are nearly black anyway.
const MIN_DEMODULATION_FACTOR: f32 = 0.02;

/// The largest value a demodulated, exposed signal may hold: just under what a
/// 16-bit float stores (65504). Not lower, because demodulation amplifies: a
/// dielectric's specular factor is about 0.04, so a highlight that exposes to 10
/// is already 250 here, and capping that would clip every bright glint.
const MAX_DENOISER_SIGNAL: f32 = 60000.0;

/// The largest luminance the second moment is computed from. Moments are kept in
/// 16-bit floats too, and the square of anything above 255 overflows them. Only
/// the noise estimate saturates; the signal itself is untouched.
const MAX_MOMENT_LUMINANCE: f32 = 250.0;

/// Per-channel factors the diffuse and specular lighting of a surface are divided
/// by before filtering, and multiplied by after: its diffuse albedo, and its
/// specular reflectance averaged over the lobe for this view direction.
///
/// Filtering lighting rather than lit colour is what lets a blur smooth noise
/// without smearing texture detail: a brick wall's mortar lines are in the
/// albedo, not in the light falling on it.
struct DemodulationFactors {
    diffuse: vec3<f32>,
    specular: vec3<f32>,
}

fn demodulation_factors(material: ResolvedMaterial, world_normal: vec3<f32>, wo: vec3<f32>) -> DemodulationFactors {
    let diffuse = calculate_diffuse_color(material.base_color, material.metallic, 0.0, 0.0);
    let F0 = calculate_F0(material.base_color, material.metallic, vec3(material.reflectance));
    let specular = env_brdf_approx2(F0, material.roughness, world_normal, wo);
    return DemodulationFactors(
        max(diffuse, vec3(MIN_DEMODULATION_FACTOR)),
        max(specular, vec3(MIN_DEMODULATION_FACTOR)),
    );
}

/// Make a lighting value safe to filter: NaN and infinity become 0 (one bad
/// sample would otherwise spread through the whole blur and stay in history for
/// good), negatives are dropped, and the value is capped so its moments fit.
fn sanitize_signal(signal: vec3<f32>) -> vec3<f32> {
    let finite = select(signal, vec3(0.0), (signal != signal) | (abs(signal) > vec3(65504.0)));
    let clamped = max(finite, vec3(0.0));
    let peak = max(clamped.r, max(clamped.g, clamped.b));
    return select(clamped, clamped * (MAX_DENOISER_SIGNAL / peak), peak > MAX_DENOISER_SIGNAL);
}

/// Split-sum approximation of the specular reflectance integrated over the lobe.
/// A copy of the polynomial fit the DLSS guide-buffer pass uses for its specular
/// albedo (`resolve_dlss_rr_textures.wesl`), given as `EnvBRDFApprox2` in the
/// appendix of NVIDIA's DLSS Ray Reconstruction integration guide. Copied rather
/// than imported because that module only compiles with the DLSS guide buffers
/// bound.
fn env_brdf_approx2(specular_color: vec3<f32>, alpha: f32, N: vec3<f32>, V: vec3<f32>) -> vec3<f32> {
    let NoV = abs(dot(N, V));

    var X: vec4<f32>;
    X.x = 1.0;
    X.y = NoV;
    X.z = NoV * NoV;
    X.w = NoV * X.z;

    var Y: vec4<f32>;
    Y.x = 1.0;
    Y.y = alpha;
    Y.z = alpha * alpha;
    Y.w = alpha * Y.z;

    let M1 = mat2x2<f32>(0.99044, 1.29678, -1.28514, -0.755907);
    let M2 = mat3x3<f32>(1.0, 20.3225, 121.563, 2.92338, -27.0302, 626.13, 59.4188, 222.592, 316.627);
    let M3 = mat2x2<f32>(0.0365463, 9.0632, 3.32707, -9.04756);
    let M4 = mat3x3<f32>(1.0, 9.04401, 5.56589, 3.59685, -16.3174, 19.7886, -1.36772, 9.22949, -20.2123);

    var bias = dot(M1 * X.xy, Y.xy) / dot(M2 * X.xyw, Y.xyw);
    let scale = dot(M3 * X.xy, Y.xy) / dot(M4 * X.xzw, Y.xyw);

    bias *= saturate(specular_color.g * 50.0);

    return fma(specular_color, vec3(max(0.0, scale)), vec3(max(0.0, bias)));
}

/// Luminance of a demodulated signal, the quantity the denoiser's moments and
/// edge-stopping weights are computed on.
fn signal_luminance(signal: vec3<f32>) -> f32 {
    return luminance(signal);
}

/// First and second luminance moments of one sample, for the noise estimate.
fn signal_moments(signal: vec3<f32>) -> vec2<f32> {
    let l = min(luminance(signal), MAX_MOMENT_LUMINANCE);
    return vec2(l, l * l);
}
