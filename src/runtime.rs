use bevy::prelude::*;
use bevy::render::{
    settings::{Backends, RenderCreation, WgpuSettings},
    RenderPlugin,
};


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

    app.add_plugins(
        DefaultPlugins.set(RenderPlugin {
            render_creation: RenderCreation::Automatic(platform_wgpu_settings()),
            ..default()
        })
    );
    app.add_plugins(renzora_runtime::RuntimePlugin);
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

    app
}
