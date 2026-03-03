//! Component definitions for the component registry

pub mod audio_listener;
pub mod audio_emitter;
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
pub(crate) mod text_3d;
pub(crate) mod sun;
pub mod terrain;
pub(crate) mod clouds;
pub(crate) mod surface_painting;
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

// Night stars
pub(crate) mod night_stars;

// Animation
pub(crate) mod animator;

// Navigation
pub(crate) mod navigation_agent;

// VR/XR components (feature-gated)
#[cfg(feature = "xr")]
pub(crate) mod vr_controller;
#[cfg(feature = "xr")]
pub(crate) mod vr_teleport_area;
#[cfg(feature = "xr")]
pub(crate) mod vr_grabbable;
#[cfg(feature = "xr")]
pub(crate) mod vr_hand_model;
#[cfg(feature = "xr")]
pub(crate) mod vr_pointer;
#[cfg(feature = "xr")]
pub(crate) mod vr_snap_zone;
#[cfg(feature = "xr")]
pub(crate) mod vr_climbable;
#[cfg(feature = "xr")]
pub(crate) mod vr_spatial_anchor;
#[cfg(feature = "xr")]
pub(crate) mod vr_overlay_panel;
#[cfg(feature = "xr")]
pub(crate) mod vr_tracked_object;
#[cfg(feature = "xr")]
pub(crate) mod vr_passthrough_window;

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
    text_3d::register(registry);
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
    audio_emitter::register(registry);

    // Animation
    animator::register(registry);

    // Gameplay
    health::register(registry);
    navigation_agent::register(registry);

    // Terrain
    terrain::register(registry);

    // Ambient light
    ambient_light::register(registry);

    // Clouds
    clouds::register(registry);

    // Night Stars
    night_stars::register(registry);

    // Surface painting
    surface_painting::register(registry);

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

    // VR/XR
    #[cfg(feature = "xr")]
    {
        vr_controller::register(registry);
        vr_teleport_area::register(registry);
        vr_grabbable::register(registry);
        vr_hand_model::register(registry);
        vr_pointer::register(registry);
        vr_snap_zone::register(registry);
        vr_climbable::register(registry);
        vr_spatial_anchor::register(registry);
        vr_overlay_panel::register(registry);
        vr_tracked_object::register(registry);
        vr_passthrough_window::register(registry);
    }

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

}
