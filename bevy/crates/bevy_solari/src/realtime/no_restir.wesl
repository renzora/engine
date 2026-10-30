import package::realtime::gbuffer_utils::gpixel_resolve;
import package::realtime::initial_path::generate_initial_path;
import package::realtime::bindings::{constants, depth_buffer, gbuffer, view, view_output};
import package::realtime::denoise_input::write_noisy_signal;
import package::realtime::debug_layout::{flush_ray_counts, SLOT_RAYS_INITIAL};
import package::scene::bindings::RAY_T_MAX;
import package::realtime::world_cache_query::{query_world_cache, WORLD_CACHE_CELL_LIFETIME};

enable wgpu_ray_query;

@compute @workgroup_size(8, 8, 1)
fn initial_and_shade(@builtin(workgroup_id) workgroup_id: vec3<u32>, @builtin(global_invocation_id) global_id: vec3<u32>) {
    if any(global_id.xy >= vec2u(view.main_pass_viewport.zw)) { return; }

    let pixel_index = global_id.x + global_id.y * u32(view.main_pass_viewport.z);
    var rng = pixel_index + constants.frame_rng;

    let depth = textureLoad(depth_buffer, global_id.xy, 0);
    if depth == 0.0 { return; }

    let surface = gpixel_resolve(textureLoad(gbuffer, global_id.xy, 0), depth, global_id.xy, view.main_pass_viewport.zw, view.world_from_clip);

    let path = generate_initial_path(surface.world_position, surface.world_normal, surface.material, workgroup_id.xy, global_id.xy, &rng);

    // RENZORA PATCH: the same estimate split into its two lobes, for the denoiser.
    write_noisy_signal(global_id.xy, path.radiance, path.radiance_specular, surface.material, surface.world_normal, surface.world_position);

    var pixel_color = path.radiance;
    pixel_color *= view.exposure;
    // RENZORA PATCH: emissive added after exposure; see the same patch in
    // restir.wesl.
    pixel_color += surface.material.emissive;
    textureStore(view_output, global_id.xy, vec4(pixel_color, 1.0));

    // RENZORA PATCH: the diagnostics' ray counts.
    flush_ray_counts(SLOT_RAYS_INITIAL, pixel_index);

    @if(VISUALIZE_WORLD_CACHE)
    textureStore(view_output, global_id.xy, vec4(query_world_cache(surface.world_position, surface.world_normal, view.world_position, RAY_T_MAX, WORLD_CACHE_CELL_LIFETIME, &rng) * view.exposure, 1.0));
}
