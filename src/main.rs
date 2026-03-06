use bevy::prelude::*;
use renzora_runtime::RuntimePlugin;

#[cfg(feature = "editor")]
use {
    renzora_editor::RenzoraEditorPlugin,
    renzora_splash::SplashPlugin,
    renzora_viewport::ViewportPlugin,
    renzora_asset_browser::AssetBrowserPlugin,
    renzora_hierarchy::HierarchyPanelPlugin,
    renzora_inspector::InspectorPanelPlugin,
    renzora_test_component::TestComponentPlugin,
    renzora_grid::GridPlugin,
    renzora_camera::CameraPlugin,
    renzora_keybindings::KeybindingsPlugin,
    renzora_gizmo::GizmoPlugin,
    renzora_scene::ScenePlugin,
    renzora_export::ExportPlugin,
};

fn main() {
    let mut app = App::new();

    #[cfg(target_arch = "wasm32")]
    app.add_plugins(DefaultPlugins);

    #[cfg(not(target_arch = "wasm32"))]
    app.add_plugins(DefaultPlugins);

    app.add_plugins(RuntimePlugin);

    // Editor plugins
    #[cfg(feature = "editor")]
    app.add_plugins((
        SplashPlugin,
        RenzoraEditorPlugin,
        GridPlugin,
        CameraPlugin,
        KeybindingsPlugin,
        GizmoPlugin,
        ViewportPlugin,
        AssetBrowserPlugin,
        HierarchyPanelPlugin,
        InspectorPanelPlugin,
        TestComponentPlugin,
        ScenePlugin,
        ExportPlugin,
    ));

    // Post-process plugins
    app.add_plugins((
        renzora_vignette::VignettePlugin,
        renzora_film_grain::FilmGrainPlugin,
        renzora_pixelation::PixelationPlugin,
        renzora_crt::CrtPlugin,
        renzora_god_rays::GodRaysPlugin,
        renzora_gaussian_blur::GaussianBlurPlugin,
        renzora_palette_quantization::PaletteQuantizationPlugin,
        renzora_distortion::DistortionPlugin,
        renzora_underwater::UnderwaterPlugin,
        renzora_chromatic_aberration::ChromaticAberrationPlugin,
        renzora_sharpen::SharpenPlugin,
        renzora_color_grading::ColorGradingPlugin,
        renzora_scanlines::ScanlinesPlugin,
        renzora_grayscale::GrayscalePlugin,
        renzora_posterize::PosterizePlugin,
    ));
    app.add_plugins((
        renzora_emboss::EmbossPlugin,
        renzora_oil_painting::OilPaintingPlugin,
        renzora_edge_glow::EdgeGlowPlugin,
        renzora_matrix::MatrixPlugin,
        renzora_outline::OutlinePlugin,
        renzora_toon::ToonPlugin,
        renzora_sepia::SepiaPlugin,
        renzora_invert::InvertPlugin,
        renzora_night_vision::NightVisionPlugin,
        renzora_glitch::GlitchPlugin,
        renzora_radial_blur::RadialBlurPlugin,
        renzora_halftone::HalftonePlugin,
        renzora_hex_pixelate::HexPixelatePlugin,
        renzora_dithering::DitheringPlugin,
        renzora_frosted_glass::FrostedGlassPlugin,
    ));
    app.add_plugins((
        renzora_skybox::SkyboxPlugin,
        renzora_clouds::CloudsPlugin,
        renzora_lighting::LightingPlugin,
    ));

    app.run();
}
