use bevy::prelude::*;

/// Build the runtime app with all engine plugins (no editor).
/// Used by both the desktop binary and the Android cdylib.
pub fn build_runtime_app() -> App {
    let mut app = App::new();

    app.add_plugins(DefaultPlugins);
    app.add_plugins(renzora_runtime::RuntimePlugin);

    // Post-process plugins
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

    // Environment plugins
    app.add_plugins(renzora_skybox::SkyboxPlugin);
    app.add_plugins(renzora_clouds::CloudsPlugin);
    app.add_plugins(renzora_lighting::LightingPlugin);

    app
}
