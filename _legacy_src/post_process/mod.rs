//! GPU-side settings structs for custom post-processing effects.
//!
//! Each struct implements `FullscreenMaterial` and is paired with a WGSL shader.
//! The `FullscreenMaterialPlugin<T>` handles all render graph setup automatically.

use bevy::core_pipeline::{
    core_3d::graph::{Core3d, Node3d},
    fullscreen_material::FullscreenMaterial,
};
use bevy::prelude::*;
use bevy::render::{
    extract_component::ExtractComponent,
    render_graph::{InternedRenderLabel, InternedRenderSubGraph, RenderLabel, RenderSubGraph},
    render_resource::ShaderType,
};
use bevy::shader::ShaderRef;

// ── Vignette ────────────────────────────────────────────────────────────

#[derive(Component, Clone, Copy, Default, ShaderType, ExtractComponent)]
#[extract_component_filter(With<Camera3d>)]
pub struct VignetteSettings {
    pub intensity: f32,
    pub radius: f32,
    pub smoothness: f32,
    pub color_r: f32,
    pub color_g: f32,
    pub color_b: f32,
    pub _padding1: f32,
    pub _padding2: f32,
}

impl FullscreenMaterial for VignetteSettings {
    fn fragment_shader() -> ShaderRef {
        "shaders/post_process/vignette.wgsl".into()
    }
    fn sub_graph() -> Option<InternedRenderSubGraph> {
        Some(Core3d.intern())
    }
    fn node_edges() -> Vec<InternedRenderLabel> {
        vec![
            Node3d::Tonemapping.intern(),
            Self::node_label().intern(),
            Node3d::EndMainPassPostProcessing.intern(),
        ]
    }
}

// ── Film Grain ──────────────────────────────────────────────────────────

#[derive(Component, Clone, Copy, Default, ShaderType, ExtractComponent)]
#[extract_component_filter(With<Camera3d>)]
pub struct FilmGrainSettings {
    pub intensity: f32,
    pub grain_size: f32,
    pub time: f32,
    pub _padding0: f32,
    pub _padding1: f32,
    pub _padding2: f32,
    pub _padding3: f32,
    pub _padding4: f32,
}

impl FullscreenMaterial for FilmGrainSettings {
    fn fragment_shader() -> ShaderRef {
        "shaders/post_process/film_grain.wgsl".into()
    }
    fn sub_graph() -> Option<InternedRenderSubGraph> {
        Some(Core3d.intern())
    }
    fn node_edges() -> Vec<InternedRenderLabel> {
        vec![
            Node3d::Tonemapping.intern(),
            Self::node_label().intern(),
            Node3d::EndMainPassPostProcessing.intern(),
        ]
    }
}

// ── Pixelation ──────────────────────────────────────────────────────────

#[derive(Component, Clone, Copy, Default, ShaderType, ExtractComponent)]
#[extract_component_filter(With<Camera3d>)]
pub struct PixelationSettings {
    pub pixel_size: f32,
    pub _padding0: f32,
    pub _padding1: f32,
    pub _padding2: f32,
    pub _padding3: f32,
    pub _padding4: f32,
    pub _padding5: f32,
    pub _padding6: f32,
}

impl FullscreenMaterial for PixelationSettings {
    fn fragment_shader() -> ShaderRef {
        "shaders/post_process/pixelation.wgsl".into()
    }
    fn sub_graph() -> Option<InternedRenderSubGraph> {
        Some(Core3d.intern())
    }
    fn node_edges() -> Vec<InternedRenderLabel> {
        vec![
            Node3d::Tonemapping.intern(),
            Self::node_label().intern(),
            Node3d::EndMainPassPostProcessing.intern(),
        ]
    }
}

// ── CRT ─────────────────────────────────────────────────────────────────

#[derive(Component, Clone, Copy, Default, ShaderType, ExtractComponent)]
#[extract_component_filter(With<Camera3d>)]
pub struct CrtSettings {
    pub scanline_intensity: f32,
    pub curvature: f32,
    pub chromatic_amount: f32,
    pub vignette_amount: f32,
    pub _padding0: f32,
    pub _padding1: f32,
    pub _padding2: f32,
    pub _padding3: f32,
}

impl FullscreenMaterial for CrtSettings {
    fn fragment_shader() -> ShaderRef {
        "shaders/post_process/crt.wgsl".into()
    }
    fn sub_graph() -> Option<InternedRenderSubGraph> {
        Some(Core3d.intern())
    }
    fn node_edges() -> Vec<InternedRenderLabel> {
        vec![
            Node3d::Tonemapping.intern(),
            Self::node_label().intern(),
            Node3d::EndMainPassPostProcessing.intern(),
        ]
    }
}

// ── God Rays ────────────────────────────────────────────────────────────

#[derive(Component, Clone, Copy, ShaderType, ExtractComponent)]
#[extract_component_filter(With<Camera3d>)]
pub struct GodRaysSettings {
    pub intensity: f32,
    pub decay: f32,
    pub density: f32,
    pub num_samples: u32,
    pub light_pos_x: f32,
    pub light_pos_y: f32,
    pub _padding1: f32,
    pub _padding2: f32,
}

impl Default for GodRaysSettings {
    fn default() -> Self {
        Self {
            intensity: 0.5,
            decay: 0.97,
            density: 1.0,
            num_samples: 64,
            light_pos_x: 0.5,
            light_pos_y: 0.3,
            _padding1: 0.0,
            _padding2: 0.0,
        }
    }
}

impl FullscreenMaterial for GodRaysSettings {
    fn fragment_shader() -> ShaderRef {
        "shaders/post_process/god_rays.wgsl".into()
    }
    fn sub_graph() -> Option<InternedRenderSubGraph> {
        Some(Core3d.intern())
    }
    fn node_edges() -> Vec<InternedRenderLabel> {
        vec![
            Node3d::Tonemapping.intern(),
            Self::node_label().intern(),
            Node3d::EndMainPassPostProcessing.intern(),
        ]
    }
}

// ── Gaussian Blur ───────────────────────────────────────────────────────

#[derive(Component, Clone, Copy, Default, ShaderType, ExtractComponent)]
#[extract_component_filter(With<Camera3d>)]
pub struct GaussianBlurSettings {
    pub sigma: f32,
    pub kernel_size: u32,
    pub _padding0: f32,
    pub _padding1: f32,
    pub _padding2: f32,
    pub _padding3: f32,
    pub _padding4: f32,
    pub _padding5: f32,
}

impl FullscreenMaterial for GaussianBlurSettings {
    fn fragment_shader() -> ShaderRef {
        "shaders/post_process/gaussian_blur.wgsl".into()
    }
    fn sub_graph() -> Option<InternedRenderSubGraph> {
        Some(Core3d.intern())
    }
    fn node_edges() -> Vec<InternedRenderLabel> {
        vec![
            Node3d::Tonemapping.intern(),
            Self::node_label().intern(),
            Node3d::EndMainPassPostProcessing.intern(),
        ]
    }
}

// ── Palette Quantization ────────────────────────────────────────────────

#[derive(Component, Clone, Copy, Default, ShaderType, ExtractComponent)]
#[extract_component_filter(With<Camera3d>)]
pub struct PaletteQuantizationSettings {
    pub num_colors: u32,
    pub dithering: f32,
    pub _padding0: f32,
    pub _padding1: f32,
    pub _padding2: f32,
    pub _padding3: f32,
    pub _padding4: f32,
    pub _padding5: f32,
}

impl FullscreenMaterial for PaletteQuantizationSettings {
    fn fragment_shader() -> ShaderRef {
        "shaders/post_process/palette_quantization.wgsl".into()
    }
    fn sub_graph() -> Option<InternedRenderSubGraph> {
        Some(Core3d.intern())
    }
    fn node_edges() -> Vec<InternedRenderLabel> {
        vec![
            Node3d::Tonemapping.intern(),
            Self::node_label().intern(),
            Node3d::EndMainPassPostProcessing.intern(),
        ]
    }
}

// ── Distortion / Heat Haze ──────────────────────────────────────────────

#[derive(Component, Clone, Copy, Default, ShaderType, ExtractComponent)]
#[extract_component_filter(With<Camera3d>)]
pub struct DistortionSettings {
    pub intensity: f32,
    pub speed: f32,
    pub scale: f32,
    pub time: f32,
    pub _padding0: f32,
    pub _padding1: f32,
    pub _padding2: f32,
    pub _padding3: f32,
}

impl FullscreenMaterial for DistortionSettings {
    fn fragment_shader() -> ShaderRef {
        "shaders/post_process/distortion.wgsl".into()
    }
    fn sub_graph() -> Option<InternedRenderSubGraph> {
        Some(Core3d.intern())
    }
    fn node_edges() -> Vec<InternedRenderLabel> {
        vec![
            Node3d::Tonemapping.intern(),
            Self::node_label().intern(),
            Node3d::EndMainPassPostProcessing.intern(),
        ]
    }
}

// ── Underwater / Rain on Lens ───────────────────────────────────────────

#[derive(Component, Clone, Copy, Default, ShaderType, ExtractComponent)]
#[extract_component_filter(With<Camera3d>)]
pub struct UnderwaterSettings {
    pub distortion: f32,
    pub tint_r: f32,
    pub tint_g: f32,
    pub tint_b: f32,
    pub tint_strength: f32,
    pub wave_speed: f32,
    pub wave_scale: f32,
    pub time: f32,
}

impl FullscreenMaterial for UnderwaterSettings {
    fn fragment_shader() -> ShaderRef {
        "shaders/post_process/underwater.wgsl".into()
    }
    fn sub_graph() -> Option<InternedRenderSubGraph> {
        Some(Core3d.intern())
    }
    fn node_edges() -> Vec<InternedRenderLabel> {
        vec![
            Node3d::Tonemapping.intern(),
            Self::node_label().intern(),
            Node3d::EndMainPassPostProcessing.intern(),
        ]
    }
}
