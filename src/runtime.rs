use bevy::prelude::*;
use bevy::render::{
    settings::{RenderCreation, WgpuSettings},
    RenderPlugin,
};
#[cfg(target_os = "android")]
use bevy::render::settings::Backends;


/// Pick the best GPU backend for the current platform.
fn platform_wgpu_settings() -> WgpuSettings {
    // Android: force Vulkan (all supported devices have Vulkan)
    #[cfg(target_os = "android")]
    {
        WgpuSettings {
            backends: Some(Backends::VULKAN),
            ..default()
        }
    }

    // Desktop: let wgpu auto-select (Vulkan/DX12/Metal)
    #[cfg(not(target_os = "android"))]
    {
        WgpuSettings::default()
    }
}

/// Build the runtime app with all engine plugins (no editor).
/// Used by both the desktop binary and the Android cdylib.
pub fn build_runtime_app() -> App {
    let mut app = App::new();

    // Register custom asset reader BEFORE DefaultPlugins so AssetPlugin uses it.
    renzora_runtime::setup_asset_reader(&mut app);

    app.add_plugins(
        DefaultPlugins.set(RenderPlugin {
            render_creation: RenderCreation::Automatic(platform_wgpu_settings()),
            ..default()
        })
    );
    app.add_plugins(renzora_runtime::RuntimePlugin);
    app.add_plugins(renzora_scripting::ScriptingPlugin::new());
    app.add_plugins(renzora_blueprint::BlueprintPlugin);
    app.add_plugins(renzora_physics::PhysicsPlugin);
    app.add_plugins(renzora_stinger::StingerPlugin);
    app.add_plugins(renzora_audio::KiraPlugin);
    app.add_plugins(renzora_vignette::VignettePlugin);
    app.add_plugins(renzora_film_grain::FilmGrainPlugin);
    app.add_plugins(renzora_pixelation::PixelationPlugin);
    app.add_plugins(renzora_crt::CrtPlugin);
    app.add_plugins(renzora_god_rays::GodRaysPlugin);
    app.add_plugins(renzora_gaussian_blur::GaussianBlurPlugin);
    app.add_plugins(renzora_palette_quantization::PaletteQuantizationPlugin);
    app.add_plugins(renzora_distortion::DistortionPlugin);
    app.add_plugins(renzora_underwater::UnderwaterPlugin);
    app.add_plugins(renzora_chromatic_aberration::ChromaticAberrationPlugin);
    app.add_plugins(renzora_sharpen::SharpenPlugin);
    app.add_plugins(renzora_color_grading::ColorGradingPlugin);
    app.add_plugins(renzora_scanlines::ScanlinesPlugin);
    app.add_plugins(renzora_grayscale::GrayscalePlugin);
    app.add_plugins(renzora_posterize::PosterizePlugin);
    app.add_plugins(renzora_emboss::EmbossPlugin);
    app.add_plugins(renzora_oil_painting::OilPaintingPlugin);
    app.add_plugins(renzora_edge_glow::EdgeGlowPlugin);
    app.add_plugins(renzora_matrix::MatrixPlugin);
    app.add_plugins(renzora_outline::OutlinePlugin);
    app.add_plugins(renzora_toon::ToonPlugin);
    app.add_plugins(renzora_sepia::SepiaPlugin);
    app.add_plugins(renzora_invert::InvertPlugin);
    app.add_plugins(renzora_pillowbox::PillowboxPlugin);
    app.add_plugins(renzora_letterbox::LetterboxPlugin);
    app.add_plugins(renzora_night_vision::NightVisionPlugin);
    app.add_plugins(renzora_glitch::GlitchPlugin);
    app.add_plugins(renzora_radial_blur::RadialBlurPlugin);
    app.add_plugins(renzora_halftone::HalftonePlugin);
    app.add_plugins(renzora_hex_pixelate::HexPixelatePlugin);
    app.add_plugins(renzora_dithering::DitheringPlugin);
    app.add_plugins(renzora_frosted_glass::FrostedGlassPlugin);
    app.add_plugins(renzora_skybox::SkyboxPlugin);
    app.add_plugins(renzora_night_stars::NightStarsPlugin);
    app.add_plugins(renzora_clouds::CloudsPlugin);
    app.add_plugins(renzora_lighting::LightingPlugin);
    app.add_plugins(renzora_shape_library::ShapeLibraryPlugin);
    app.add_plugins(renzora_hanabi::HanabiParticlePlugin);
    app.add_plugins(renzora_terrain::TerrainPlugin);
    app.add_plugins(renzora_water::WaterPlugin);
    app.add_plugins(renzora_material::MaterialPlugin);
    app.add_plugins(renzora_shader::ShaderPlugin);
    app.add_plugins(renzora_rt::RtPlugin);
    app.add_plugins(renzora_animation::AnimationPlugin);
    app.add_plugins(renzora_game_ui::GameUiPlugin);
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
    app.add_plugins(renzora_gauges::GaugesPlugin);
    // app.add_plugins(renzora_forward_decal::DecalPlugin); // disabled: Bevy bindless bind group mismatch with ForwardDecalMaterial

    app
}
