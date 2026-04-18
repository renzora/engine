//! Renzora Runtime — shared library re-exporting all core engine crates.
//!
//! Plugins and the editor binary link against this dylib instead of
//! statically embedding each crate. Keeps plugin DLLs small.

use bevy::prelude::*;

// Core
pub use renzora_engine;
pub use renzora;
pub use renzora_scripting;
pub use renzora_blueprint;
pub use renzora_input;
pub use renzora_physics;
pub use renzora_terrain;
pub use renzora_lighting;
pub use renzora_water;
pub use renzora_animation;
pub use renzora_game_ui;
pub use renzora_gauges;
pub use renzora_hanabi;
pub use renzora_network;
pub use renzora_audio;
pub use renzora_shader;

// Postprocess framework + vital effects
pub use renzora_postprocess;
pub use renzora_tonemapping;
pub use renzora_bloom_effect;
pub use renzora_antialiasing;
pub use renzora_ssao;
pub use renzora_ssr;
pub use renzora_auto_exposure;
pub use renzora_oit;
pub use renzora_dof;
pub use renzora_motion_blur;
pub use renzora_distance_fog;

// Environment
pub use renzora_atmosphere;
pub use renzora_skybox;
pub use renzora_clouds;
pub use renzora_night_stars;

// ── App setup (single source of truth for engine plugin registration) ────

pub fn platform_wgpu_settings() -> bevy::render::settings::WgpuSettings {
    #[cfg(target_os = "android")]
    {
        use bevy::render::settings::{Backends, WgpuSettings};
        WgpuSettings {
            backends: Some(Backends::VULKAN),
            ..default()
        }
    }

    #[cfg(not(target_os = "android"))]
    {
        use bevy::render::settings::{WgpuFeatures, WgpuSettings};
        WgpuSettings {
            features: WgpuFeatures::POLYGON_MODE_LINE,
            ..default()
        }
    }
}

pub fn init_app() -> App {
    let mut app = App::new();
    renzora_engine::setup_asset_reader(&mut app);
    app
}

pub fn add_default_rendering(app: &mut App) {
    use bevy::render::{settings::RenderCreation, RenderPlugin};
    use bevy::window::{Window, WindowPlugin};
    app.add_plugins(
        DefaultPlugins
            .set(RenderPlugin {
                render_creation: RenderCreation::Automatic(platform_wgpu_settings()),
                ..default()
            })
            .set(ImagePlugin {
                default_sampler: bevy::image::ImageSamplerDescriptor {
                    address_mode_u: bevy::image::ImageAddressMode::Repeat,
                    address_mode_v: bevy::image::ImageAddressMode::Repeat,
                    address_mode_w: bevy::image::ImageAddressMode::Repeat,
                    ..default()
                },
                ..default()
            })
            .set(WindowPlugin {
                primary_window: Some(Window {
                    title: "Renzora".into(),
                    decorations: false,
                    resizable: true,
                    ..default()
                }),
                ..default()
            })
    );
    app.add_systems(Startup, maximize_primary_window);
}

fn maximize_primary_window(
    mut windows: Query<&mut bevy::window::Window, With<bevy::window::PrimaryWindow>>,
) {
    if let Ok(mut window) = windows.single_mut() {
        window.set_maximized(true);
    }
}

pub fn add_engine_plugins(app: &mut App) {
    app.add_plugins(renzora_engine::RuntimePlugin);
    app.add_plugins(renzora_scripting::ScriptingPlugin::new());
    app.add_plugins(renzora_blueprint::BlueprintPlugin);
    app.add_plugins(renzora_input::InputPlugin);
    app.add_plugins(renzora_physics::PhysicsPlugin);
    app.add_plugins(renzora_navmesh::NavMeshPlugin);
    app.add_plugins(renzora_globals::GlobalsPlugin);
    app.add_plugins(renzora_terrain::TerrainPlugin);
    app.add_plugins(renzora_lighting::LightingPlugin);
    app.add_plugins(renzora_water::WaterPlugin);
    app.add_plugins(renzora_terrain::foliage::FoliagePlugin);
    app.add_plugins(renzora_animation::AnimationPlugin);
    app.add_plugins(renzora_game_ui::GameUiPlugin);
    app.add_plugins(renzora_shader::material::MaterialPlugin);
    app.add_plugins(renzora_gauges::GaugesPlugin);
    app.add_plugins(renzora_hanabi::HanabiParticlePlugin);
    app.add_plugins(renzora_network::NetworkPlugin);
    app.add_plugins(renzora_audio::KiraPlugin);
    app.add_plugins(renzora_shader::ShaderPlugin);
    app.add_plugins(renzora_skybox::SkyboxPlugin);
    app.add_plugins(renzora_night_stars::NightStarsPlugin);
    app.add_plugins(renzora_clouds::CloudsPlugin);
    app.add_plugins(renzora_tonemapping::TonemappingPlugin);
    app.add_plugins(renzora_bloom_effect::BloomEffectPlugin);
    app.add_plugins(renzora_dof::DepthOfFieldPlugin);
    app.add_plugins(renzora_motion_blur::MotionBlurPlugin);
    app.add_plugins(renzora_antialiasing::AntiAliasingPlugin);
    app.add_plugins(renzora_distance_fog::DistanceFogPlugin);
    app.add_plugins(renzora_atmosphere::AtmospherePlugin);
    app.add_plugins(renzora_ssao::SsaoPlugin);
    app.add_plugins(renzora_ssr::SsrPlugin);
    app.add_plugins(renzora_auto_exposure::AutoExposurePlugin);
    app.add_plugins(renzora_oit::OitPlugin);
}

/// Build the full runtime app (rendering + all engine plugins).
pub fn build_runtime_app() -> App {
    let mut app = init_app();
    add_default_rendering(&mut app);
    add_engine_plugins(&mut app);
    app
}

