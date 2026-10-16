//! Where Bevy's shader modules went between 0.19 and 0.20.
//!
//! In 0.19 a module was named by its `#define_import_path` line, whatever the
//! file was called. In 0.20 a module is named by where its file sits in its
//! crate, so most of `bevy_pbr::*` moved under `bevy_pbr::render::*`. This was
//! built by matching every 0.19 `.wgsl` file to its 0.20 `.wesl` counterpart
//! by path and by the declarations they share, then checked by hand where the
//! match was not by path.

/// `(0.19 module, 0.20 module)`.
pub const MODULES: &[(&str, &str)] = &[
    ("bevy_core_pipeline::fullscreen_vertex_shader", "bevy_core_pipeline::fullscreen_vertex_shader"),
    ("bevy_core_pipeline::oit", "bevy_core_pipeline::oit::draw"),
    ("bevy_core_pipeline::tonemapping", "bevy_core_pipeline::tonemapping"),
    ("bevy_core_pipeline::tonemapping_lut_bindings", "bevy_core_pipeline::tonemapping::lut_bindings"),
    ("bevy_pbr::ambient", "bevy_pbr::render::pbr_ambient"),
    ("bevy_pbr::atmosphere::bindings", "bevy_pbr::atmosphere::bindings"),
    ("bevy_pbr::atmosphere::bruneton_functions", "bevy_pbr::atmosphere::bruneton_functions"),
    ("bevy_pbr::atmosphere::functions", "bevy_pbr::atmosphere::functions"),
    ("bevy_pbr::atmosphere::types", "bevy_pbr::atmosphere::types"),
    ("bevy_pbr::cluster", "bevy_pbr::cluster"),
    ("bevy_pbr::clustered_forward", "bevy_pbr::render::clustered_forward"),
    ("bevy_pbr::decal::clustered", "bevy_pbr::decal::clustered"),
    ("bevy_pbr::decal::forward", "bevy_pbr::decal::forward"),
    ("bevy_pbr::environment_map", "bevy_pbr::light_probe::environment_map"),
    ("bevy_pbr::fog", "bevy_pbr::render::fog"),
    ("bevy_pbr::forward_io", "bevy_pbr::render::forward_io"),
    ("bevy_pbr::irradiance_volume", "bevy_pbr::light_probe::irradiance_volume"),
    ("bevy_pbr::light_probe", "bevy_pbr::light_probe"),
    ("bevy_pbr::lighting", "bevy_pbr::render::pbr_lighting"),
    ("bevy_pbr::lightmap", "bevy_pbr::lightmap"),
    ("bevy_pbr::mesh_bindings", "bevy_pbr::render::mesh_bindings"),
    ("bevy_pbr::mesh_functions", "bevy_pbr::render::mesh_functions"),
    ("bevy_pbr::mesh_preprocess_types", "bevy_render::occlusion_culling::mesh_preprocess_types"),
    ("bevy_pbr::mesh_types", "bevy_pbr::render::mesh_types"),
    ("bevy_pbr::mesh_view_bindings", "bevy_pbr::render::mesh_view_bindings"),
    ("bevy_pbr::mesh_view_types", "bevy_pbr::render::mesh_view_types"),
    ("bevy_pbr::meshlet_bindings", "bevy_pbr::meshlet::bindings"),
    ("bevy_pbr::meshlet_cull_shared", "bevy_pbr::meshlet::cull_shared"),
    ("bevy_pbr::meshlet_visibility_buffer_resolve", "bevy_pbr::meshlet::visibility_buffer_resolve"),
    ("bevy_pbr::morph", "bevy_pbr::render::morph"),
    ("bevy_pbr::occlusion_culling", "bevy_pbr::render::occlusion_culling"),
    ("bevy_pbr::parallax_mapping", "bevy_pbr::render::parallax_mapping"),
    ("bevy_pbr::pbr_bindings", "bevy_pbr::render::pbr_bindings"),
    ("bevy_pbr::pbr_deferred_functions", "bevy_pbr::deferred::functions"),
    ("bevy_pbr::pbr_deferred_types", "bevy_pbr::deferred::types"),
    ("bevy_pbr::pbr_fragment", "bevy_pbr::render::pbr_fragment"),
    ("bevy_pbr::pbr_functions", "bevy_pbr::render::pbr_functions"),
    ("bevy_pbr::pbr_prepass_functions", "bevy_pbr::render::pbr_prepass_functions"),
    ("bevy_pbr::pbr_types", "bevy_pbr::render::pbr_types"),
    ("bevy_pbr::prepass_bindings", "bevy_pbr::prepass::bindings"),
    ("bevy_pbr::prepass_io", "bevy_pbr::prepass::io"),
    ("bevy_pbr::prepass_utils", "bevy_pbr::prepass::utils"),
    ("bevy_pbr::raymarch", "bevy_pbr::ssr::raymarch"),
    ("bevy_pbr::rgb9e5", "bevy_pbr::render::rgb9e5"),
    ("bevy_pbr::shadow_sampling", "bevy_pbr::render::shadow_sampling"),
    ("bevy_pbr::shadows", "bevy_pbr::render::shadows"),
    ("bevy_pbr::skinning", "bevy_pbr::render::skinning"),
    ("bevy_pbr::ssao_utils", "bevy_pbr::ssao::utils"),
    ("bevy_pbr::ssr", "bevy_pbr::ssr"),
    ("bevy_pbr::transmission", "bevy_pbr::transmission"),
    ("bevy_pbr::utils", "bevy_pbr::render::utils"),
    ("bevy_pbr::view_transformations", "bevy_pbr::render::view_transformations"),
    ("bevy_post_process::effect_stack::chromatic_aberration", "bevy_post_process::effect_stack::chromatic_aberration"),
    ("bevy_post_process::effect_stack::lens_distortion", "bevy_post_process::effect_stack::lens_distortion"),
    ("bevy_post_process::effect_stack::vignette", "bevy_post_process::effect_stack::vignette"),
    ("bevy_post_process::gaussian_blur", "bevy_post_process::gaussian_blur"),
    ("bevy_render::bindless", "bevy_render::bindless"),
    ("bevy_render::color_operations", "bevy_render::color_operations"),
    ("bevy_render::globals", "bevy_render::globals"),
    ("bevy_render::maths", "bevy_render::maths"),
    ("bevy_render::view", "bevy_render::view"),
    ("bevy_solari::brdf", "bevy_solari::scene::brdf"),
    ("bevy_solari::gbuffer_utils", "bevy_solari::realtime::gbuffer_utils"),
    ("bevy_solari::presample_light_tiles", "bevy_solari::realtime::presample_light_tiles"),
    ("bevy_solari::realtime_bindings", "bevy_solari::realtime::bindings"),
    ("bevy_solari::resolve_dlss_rr_textures", "bevy_solari::realtime::resolve_dlss_rr_textures"),
    ("bevy_solari::sampling", "bevy_solari::scene::sampling"),
    ("bevy_solari::scene_bindings", "bevy_solari::scene::bindings"),
    ("bevy_solari::world_cache", "bevy_solari::realtime::world_cache_query"),
    ("bevy_sprite::mesh2d_bindings", "bevy_sprite_render::mesh2d::bindings"),
    ("bevy_sprite::mesh2d_functions", "bevy_sprite_render::mesh2d::functions"),
    ("bevy_sprite::mesh2d_types", "bevy_sprite_render::mesh2d::types"),
    ("bevy_sprite::mesh2d_vertex_output", "bevy_sprite_render::mesh2d::vertex_output"),
    ("bevy_sprite::mesh2d_view_bindings", "bevy_sprite_render::mesh2d::view_bindings"),
    ("bevy_sprite::sprite_view_bindings", "bevy_sprite_render::render::sprite_view_bindings"),
    ("bevy_ui::ui_node", "bevy_ui_render::ui"),
    ("bevy_ui::ui_vertex_output", "bevy_ui_render::ui_vertex_output"),
];

/// Declarations that left their module: `(0.19 module::item, 0.20 module)`.
/// Checked in addition to [`MODULES`], and first, since the module an item
/// was imported from may still exist without it.
pub const MOVED_ITEMS: &[(&str, &str)] = &[
    ("bevy_pbr::utils::octahedral_encode", "bevy_render::utils"),
    ("bevy_pbr::utils::octahedral_decode", "bevy_render::utils"),
    ("bevy_pbr::utils::octahedral_decode_signed", "bevy_render::utils"),
    ("bevy_pbr::environment_map::quat_rotate", "bevy_render::maths"),
    ("bevy_pbr::atmosphere::functions::sample_transmittance_lut_segment", "bevy_pbr::atmosphere::bruneton_functions"),
    // 0.19's mesh2d_view_types only re-exported these two.
    ("bevy_sprite::mesh2d_view_types::View", "bevy_render::view"),
    ("bevy_sprite::mesh2d_view_types::Globals", "bevy_render::globals"),
];

/// Shader defs Bevy renamed: `(0.19, 0.20)`.
///
/// A custom material now gates its `oit_draw` call on whether the *material*
/// takes part in order-independent transparency, which is the def that used to
/// be the camera-wide one.
pub const DEFS: &[(&str, &str)] = &[("OIT_ENABLED", "MATERIAL_OIT_ENABLED")];

/// Is `root` the first segment of a Bevy shader module path?
pub fn is_bevy_root(root: &str) -> bool {
    MODULES
        .iter()
        .chain(MOVED_ITEMS.iter())
        .any(|(old, _)| old.split("::").next() == Some(root))
}
