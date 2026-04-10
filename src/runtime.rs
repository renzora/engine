use bevy::prelude::*;

#[cfg(feature = "editor")]
pub use renzora_editor as renzora_shared;
#[cfg(not(feature = "editor"))]
pub use renzora_runtime as renzora_shared;

#[cfg(any(feature = "editor", not(feature = "server")))]
use bevy::render::{
    settings::{RenderCreation, WgpuSettings},
    RenderPlugin,
};

// ── App setup ─────────────────────────────────────────────────────────────

#[cfg(any(feature = "editor", not(feature = "server")))]
pub fn platform_wgpu_settings() -> WgpuSettings {
    #[cfg(target_os = "android")]
    {
        use bevy::render::settings::Backends;
        WgpuSettings {
            backends: Some(Backends::VULKAN),
            ..default()
        }
    }

    #[cfg(not(target_os = "android"))]
    {
        use bevy::render::settings::WgpuFeatures;
        WgpuSettings {
            features: WgpuFeatures::POLYGON_MODE_LINE,
            ..default()
        }
    }
}

pub fn init_app() -> App {
    let mut app = App::new();
    renzora_shared::renzora_engine::setup_asset_reader(&mut app);
    app
}

pub fn add_default_rendering(app: &mut App) {
    #[cfg(any(feature = "editor", not(feature = "server")))]
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
    #[cfg(all(feature = "server", not(feature = "editor")))]
    {
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

pub fn add_engine_plugins(app: &mut App) {
    app.add_plugins(renzora_shared::renzora_engine::RuntimePlugin);
    app.add_plugins(renzora_shared::renzora_scripting::ScriptingPlugin::new());
    app.add_plugins(renzora_shared::renzora_blueprint::BlueprintPlugin);
    app.add_plugins(renzora_shared::renzora_input::InputPlugin);
    app.add_plugins(renzora_shared::renzora_physics::PhysicsPlugin);
    app.add_plugins(renzora_shared::renzora_lifecycle::LifecyclePlugin);
    app.add_plugins(renzora_shared::renzora_terrain::TerrainPlugin);

    // Only skip rendering plugins when building a server-only binary (no editor).
    // When editor+server are both enabled (unified build), rendering is needed.
    #[cfg(any(feature = "editor", not(feature = "server")))]
    {
        app.add_plugins(renzora_shared::renzora_lighting::LightingPlugin);
        #[cfg(feature = "editor")]
        app.add_plugins(renzora_shared::renzora_shape_library::ShapeLibraryPlugin);
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

/// Build the full runtime app (used by WASM start and server).
pub fn build_runtime_app() -> App {
    let mut app = init_app();
    add_default_rendering(&mut app);
    add_engine_plugins(&mut app);
    app
}
