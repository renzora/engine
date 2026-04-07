use bevy::prelude::*;

// Use the editor dylib (superset) or runtime dylib depending on build.
#[cfg(feature = "editor")]
use renzora_editor as renzora_shared;
#[cfg(not(feature = "editor"))]
use renzora_runtime as renzora_shared;

#[cfg(not(feature = "server"))]
use bevy::render::{
    settings::{RenderCreation, WgpuSettings},
    RenderPlugin,
};
#[cfg(target_os = "android")]
use bevy::render::settings::Backends;

/// Pick the best GPU backend for the current platform.
#[cfg(not(feature = "server"))]
fn platform_wgpu_settings() -> WgpuSettings {
    // Android: force Vulkan (all supported devices have Vulkan)
    #[cfg(target_os = "android")]
    {
        WgpuSettings {
            backends: Some(Backends::VULKAN),
            ..default()
        }
    }

    #[cfg(not(target_os = "android"))]
    {
        use bevy::render::settings::WgpuFeatures;
        WgpuSettings {
            features: WgpuFeatures::POLYGON_MODE_LINE
                | bevy::solari::SolariPlugins::required_wgpu_features(),
            ..default()
        }
    }
}

/// Build the runtime app with all engine plugins.
///
/// With the `server` feature: headless (no window, no renderer, no audio, no postprocessing).
/// Without: full client with rendering, audio, and all visual effects.
pub fn build_runtime_app() -> App {
    let mut app = init_app();
    add_default_rendering(&mut app);
    add_engine_plugins(&mut app);
    app
}

/// Build the runtime app with XR rendering already initialized.
///
/// Call this when XR rendering was set up by the XR plugin's `xr_init_rendering`.
/// Skips DefaultPlugins (they were already added by the XR plugin with OpenXR support).
pub fn build_runtime_app_xr(app: &mut App) {
    add_engine_plugins(app);
}

/// Phase 1: Create the App and set up pre-plugin resources (asset reader, DLSS).
pub fn init_app() -> App {
    let mut app = App::new();

    // Register custom asset reader BEFORE plugins so AssetPlugin uses it.
    renzora_shared::renzora_engine::setup_asset_reader(&mut app);

    // DLSS requires a project ID before DefaultPlugins
    app.insert_resource(bevy::anti_alias::dlss::DlssProjectId(
        uuid::Uuid::from_bytes([
            0x72, 0x65, 0x6e, 0x7a, 0x6f, 0x72, 0x61, 0x2d,
            0x65, 0x6e, 0x67, 0x69, 0x6e, 0x65, 0x30, 0x31,
        ]),
    ));

    app
}

/// Phase 2: Add standard rendering (DefaultPlugins).
/// Skipped when XR plugin provides its own rendering pipeline.
pub fn add_default_rendering(app: &mut App) {
    #[cfg(not(feature = "server"))]
    {
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
        );
    }
    #[cfg(feature = "server")]
    {
        // Headless: use DefaultPlugins with no window.
        app.add_plugins(
            DefaultPlugins
                .set(bevy::window::WindowPlugin {
                    primary_window: None,
                    exit_condition: bevy::window::ExitCondition::DontExit,
                    ..default()
                })
        );
    }
}

/// Phase 3: Add all core engine plugins (gameplay, physics, audio, rendering effects).
pub fn add_engine_plugins(app: &mut App) {
    // --- Core gameplay (shared between client and server) ---
    app.add_plugins(renzora_shared::renzora_engine::RuntimePlugin);
    app.add_plugins(renzora_shared::renzora_scripting::ScriptingPlugin::new());
    app.add_plugins(renzora_shared::renzora_blueprint::BlueprintPlugin);
    app.add_plugins(renzora_shared::renzora_input::InputPlugin);
    app.add_plugins(renzora_shared::renzora_physics::PhysicsPlugin);
    app.add_plugins(renzora_shared::renzora_lifecycle::LifecyclePlugin);
    app.add_plugins(renzora_shared::renzora_terrain::TerrainPlugin);

    // --- Client-only: visual, audio, rendering, postprocessing ---
    #[cfg(not(feature = "server"))]
    {
        app.add_plugins(renzora_shared::renzora_lighting::LightingPlugin);
        app.add_plugins(renzora_shape_library::ShapeLibraryPlugin);
        app.add_plugins(renzora_shared::renzora_water::WaterPlugin);
        app.add_plugins(renzora_shared::renzora_terrain::foliage::FoliagePlugin);
        app.add_plugins(renzora_shared::renzora_animation::AnimationPlugin);
        app.add_plugins(renzora_shared::renzora_game_ui::GameUiPlugin);
        app.add_plugins(renzora_shared::renzora_shader::material::MaterialPlugin);
        app.add_plugins(renzora_shared::renzora_gauges::GaugesPlugin);
        app.add_plugins(renzora_shared::renzora_hanabi::HanabiParticlePlugin);
        app.add_plugins(renzora_shared::renzora_network::NetworkPlugin);
        app.add_plugins(renzora_shared::renzora_audio::KiraPlugin);
        app.add_plugins(renzora_shared::renzora_shader::ShaderPlugin);
        app.add_plugins(renzora_shared::renzora_skybox::SkyboxPlugin);
        app.add_plugins(renzora_shared::renzora_night_stars::NightStarsPlugin);
        app.add_plugins(renzora_shared::renzora_clouds::CloudsPlugin);
        app.add_plugins(renzora_shared::renzora_tonemapping::TonemappingPlugin);
        app.add_plugins(renzora_shared::renzora_bloom_effect::BloomEffectPlugin);
        app.add_plugins(renzora_shared::renzora_dof::DepthOfFieldPlugin);
        app.add_plugins(renzora_shared::renzora_motion_blur::MotionBlurPlugin);
        app.add_plugins(renzora_shared::renzora_antialiasing::AntiAliasingPlugin);
        app.add_plugins(renzora_shared::renzora_distance_fog::DistanceFogPlugin);
        app.add_plugins(renzora_shared::renzora_atmosphere::AtmospherePlugin);
        app.add_plugins(renzora_shared::renzora_ssao::SsaoPlugin);
        app.add_plugins(renzora_shared::renzora_ssr::SsrPlugin);
        app.add_plugins(renzora_shared::renzora_auto_exposure::AutoExposurePlugin);
        app.add_plugins(renzora_shared::renzora_oit::OitPlugin);
    }
}
