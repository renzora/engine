//! Component definitions for the component registry

mod audio_listener;
mod camera_2d;
mod camera_3d;
pub(crate) mod cloth;
mod camera_rig;
mod colliders;
mod directional_light;
mod hanabi_effect;
pub mod health;
pub mod lighting;
mod material;
mod mesh_renderer;
mod meshlet_mesh;
mod point_light;
mod rigid_body;
mod script;
mod solari_lighting;
mod spot_light;
mod sprite_2d;
pub(crate) mod sun;
pub mod terrain;
pub(crate) mod clouds;
pub(crate) mod surface_painting;
pub(crate) mod voxel_world;
mod ui_button;
mod ui_image;
mod ui_label;
mod ui_panel;
mod world_environment;

// Post-processing & lighting components
pub(crate) mod ambient_light;
pub(crate) mod ambient_occlusion;
pub(crate) mod anti_aliasing;
pub(crate) mod bloom;
pub(crate) mod depth_of_field;
pub(crate) mod fog;
pub(crate) mod motion_blur;
pub(crate) mod reflections;
pub mod skybox;
pub(crate) mod tonemapping;

// New post-processing components
pub(crate) mod taa;
pub(crate) mod smaa;
pub(crate) mod cas;
pub(crate) mod chromatic_aberration;
pub(crate) mod auto_exposure;
pub(crate) mod volumetric_fog;
pub(crate) mod vignette;
pub(crate) mod film_grain;
pub(crate) mod pixelation;
pub(crate) mod crt;
pub(crate) mod god_rays;
pub(crate) mod gaussian_blur;
pub(crate) mod palette_quantization;
pub(crate) mod distortion;
pub(crate) mod underwater;

// Geo map components
pub(crate) mod geo_map;
pub(crate) mod geo_position;
pub(crate) mod geo_marker;

// Navigation
pub(crate) mod navigation_agent;

// Re-export commonly used gameplay components
pub use health::HealthData;

use super::ComponentRegistry;

/// Register all built-in components
pub fn register_all_components(registry: &mut ComponentRegistry) {
    // Lighting
    point_light::register(registry);
    directional_light::register(registry);
    spot_light::register(registry);
    sun::register(registry);
    solari_lighting::register(registry);

    // Camera
    camera_3d::register(registry);
    camera_2d::register(registry);
    camera_rig::register(registry);

    // Physics
    rigid_body::register(registry);
    colliders::register(registry);
    cloth::register(registry);

    // Rendering
    mesh_renderer::register(registry);
    sprite_2d::register(registry);
    material::register(registry);
    meshlet_mesh::register(registry);

    // Scripting
    script::register(registry);

    // UI
    ui_panel::register(registry);
    ui_label::register(registry);
    ui_button::register(registry);
    ui_image::register(registry);

    // Environment & Effects
    world_environment::register(registry);
    hanabi_effect::register(registry);

    // Audio
    audio_listener::register(registry);

    // Gameplay
    health::register(registry);
    navigation_agent::register(registry);

    // Terrain
    terrain::register(registry);

    // Ambient light
    ambient_light::register(registry);

    // Clouds
    clouds::register(registry);

    // Surface painting
    surface_painting::register(registry);

    // Voxel world
    voxel_world::register(registry);

    // Post-processing
    fog::register(registry);
    anti_aliasing::register(registry);
    ambient_occlusion::register(registry);
    reflections::register(registry);
    bloom::register(registry);
    tonemapping::register(registry);
    depth_of_field::register(registry);
    motion_blur::register(registry);
    skybox::register(registry);

    // New post-processing effects
    taa::register(registry);
    smaa::register(registry);
    cas::register(registry);
    chromatic_aberration::register(registry);
    auto_exposure::register(registry);
    volumetric_fog::register(registry);
    vignette::register(registry);
    film_grain::register(registry);
    pixelation::register(registry);
    crt::register(registry);
    god_rays::register(registry);
    gaussian_blur::register(registry);
    palette_quantization::register(registry);
    distortion::register(registry);
    underwater::register(registry);

    // Geo map
    geo_map::register(registry);
    geo_position::register(registry);
    geo_marker::register(registry);
}
