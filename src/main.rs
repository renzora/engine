use bevy::prelude::*;
use renzora_runtime::RuntimePlugin;
use renzora_editor::RenzoraEditorPlugin;
use renzora_viewport::ViewportPlugin;
use renzora_asset_browser::AssetBrowserPlugin;
use renzora_hierarchy::HierarchyPanelPlugin;
use renzora_inspector::InspectorPanelPlugin;
use renzora_test_component::TestComponentPlugin;
use renzora_grid::GridPlugin;
use renzora_camera::CameraPlugin;
use renzora_keybindings::KeybindingsPlugin;
use renzora_gizmo::GizmoPlugin;
use renzora_vignette::VignettePlugin;
use renzora_film_grain::FilmGrainPlugin;
use renzora_pixelation::PixelationPlugin;
use renzora_crt::CrtPlugin;
use renzora_god_rays::GodRaysPlugin;
use renzora_gaussian_blur::GaussianBlurPlugin;
use renzora_palette_quantization::PaletteQuantizationPlugin;
use renzora_distortion::DistortionPlugin;
use renzora_underwater::UnderwaterPlugin;
use renzora_chromatic_aberration::ChromaticAberrationPlugin;
use renzora_sharpen::SharpenPlugin;
use renzora_color_grading::ColorGradingPlugin;
use renzora_scanlines::ScanlinesPlugin;
use renzora_grayscale::GrayscalePlugin;
use renzora_posterize::PosterizePlugin;
use renzora_emboss::EmbossPlugin;
use renzora_oil_painting::OilPaintingPlugin;
use renzora_edge_glow::EdgeGlowPlugin;
use renzora_matrix::MatrixPlugin;
use renzora_outline::OutlinePlugin;
use renzora_toon::ToonPlugin;
use renzora_sepia::SepiaPlugin;
use renzora_invert::InvertPlugin;
use renzora_night_vision::NightVisionPlugin;
use renzora_glitch::GlitchPlugin;
use renzora_radial_blur::RadialBlurPlugin;
use renzora_halftone::HalftonePlugin;
use renzora_hex_pixelate::HexPixelatePlugin;
use renzora_dithering::DitheringPlugin;
use renzora_frosted_glass::FrostedGlassPlugin;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(RuntimePlugin)
        .add_plugins(GridPlugin)
        .add_plugins(CameraPlugin)
        .add_plugins(KeybindingsPlugin)
        .add_plugins(RenzoraEditorPlugin)
        .add_plugins(GizmoPlugin)
        .add_plugins(ViewportPlugin)
        .add_plugins(AssetBrowserPlugin)
        .add_plugins(HierarchyPanelPlugin)
        .add_plugins(InspectorPanelPlugin)
        .add_plugins(TestComponentPlugin)
        .add_plugins(VignettePlugin)
        .add_plugins(FilmGrainPlugin)
        .add_plugins(PixelationPlugin)
        .add_plugins(CrtPlugin)
        .add_plugins(GodRaysPlugin)
        .add_plugins(GaussianBlurPlugin)
        .add_plugins(PaletteQuantizationPlugin)
        .add_plugins(DistortionPlugin)
        .add_plugins(UnderwaterPlugin)
        .add_plugins(ChromaticAberrationPlugin)
        .add_plugins(SharpenPlugin)
        .add_plugins(ColorGradingPlugin)
        .add_plugins(ScanlinesPlugin)
        .add_plugins(GrayscalePlugin)
        .add_plugins(PosterizePlugin)
        .add_plugins(EmbossPlugin)
        .add_plugins(OilPaintingPlugin)
        .add_plugins(EdgeGlowPlugin)
        .add_plugins(MatrixPlugin)
        .add_plugins(OutlinePlugin)
        .add_plugins(ToonPlugin)
        .add_plugins(SepiaPlugin)
        .add_plugins(InvertPlugin)
        .add_plugins(NightVisionPlugin)
        .add_plugins(GlitchPlugin)
        .add_plugins(RadialBlurPlugin)
        .add_plugins(HalftonePlugin)
        .add_plugins(HexPixelatePlugin)
        .add_plugins(DitheringPlugin)
        .add_plugins(FrostedGlassPlugin)
        .run();
}