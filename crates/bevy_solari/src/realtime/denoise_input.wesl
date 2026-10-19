// RENZORA PATCH: how the lighting passes hand their estimate to the denoiser.
// Kept apart from `denoise_utils.wesl` because it writes the lighting passes'
// own bindings, which the denoiser passes do not have.

import package::realtime::bindings::{noisy_diffuse, noisy_specular, view};
import package::realtime::denoise_utils::{demodulation_factors, sanitize_signal};
import package::scene::bindings::ResolvedMaterial;

/// Write one pixel's estimate, split into diffuse and specular, each divided by
/// its material factor and exposed. `total` and `specular` are unexposed
/// radiance, `specular` being the part of `total` through the specular lobe.
///
/// Exposed, because luminance moments are kept in 16-bit floats and sunlight in
/// physical units overflows them; exposed radiance sits near 1 for anything the
/// camera shows well. The composite therefore does not expose again.
fn write_noisy_signal(pixel_id: vec2<u32>, total: vec3<f32>, specular: vec3<f32>, material: ResolvedMaterial, world_normal: vec3<f32>, world_position: vec3<f32>) {
    let wo = normalize(view.world_position - world_position);
    let factors = demodulation_factors(material, world_normal, wo);
    let diffuse = max(total - specular, vec3(0.0));
    textureStore(noisy_diffuse, pixel_id, vec4(sanitize_signal(diffuse * view.exposure / factors.diffuse), 0.0));
    textureStore(noisy_specular, pixel_id, vec4(sanitize_signal(specular * view.exposure / factors.specular), 0.0));
}
