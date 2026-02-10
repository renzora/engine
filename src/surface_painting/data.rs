//! Surface painting data types — components and resources.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::terrain::{BrushFalloffType, BrushShape};

/// Default layer shader path (light).
pub const DEFAULT_LAYER_SHADER: &str = "assets/shaders/layers/default.wgsl";
/// Default dark layer shader path.
pub const DEFAULT_LAYER_SHADER_DARK: &str = "assets/shaders/layers/default_dark.wgsl";

/// A single material layer for surface painting.
/// Each layer is driven by a .wgsl or .material_bp shader file.
#[derive(Clone, Debug, Serialize, Deserialize, Reflect)]
pub struct MaterialLayer {
    pub name: String,
    /// Path to the .wgsl or .material_bp shader driving this layer.
    pub texture_path: Option<String>,
    pub uv_scale: Vec2,
    pub metallic: f32,
    pub roughness: f32,
    /// Cached WGSL source for this layer (loaded from texture_path if .wgsl)
    #[serde(skip)]
    #[reflect(ignore)]
    pub cached_shader_source: Option<String>,
}

impl Default for MaterialLayer {
    fn default() -> Self {
        Self {
            name: "Layer".to_string(),
            texture_path: Some(DEFAULT_LAYER_SHADER.to_string()),
            uv_scale: Vec2::splat(1.0),
            metallic: 0.0,
            roughness: 0.5,
            cached_shader_source: None,
        }
    }
}

/// Component holding per-mesh surface painting data.
#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component)]
pub struct PaintableSurfaceData {
    /// Material layers (max 4).
    pub layers: Vec<MaterialLayer>,
    /// Resolution of the splatmap texture (width = height).
    pub splatmap_resolution: u32,
    /// CPU-side weight data. Length = resolution * resolution.
    /// Each element is [r, g, b, a] weights for layers 0..3.
    pub splatmap_weights: Vec<[f32; 4]>,
    /// Whether the splatmap needs uploading to GPU.
    #[serde(skip)]
    #[reflect(ignore)]
    pub dirty: bool,
    /// Whether the shader needs regenerating (layer sources changed).
    #[serde(skip)]
    #[reflect(ignore)]
    pub shader_dirty: bool,
}

impl Default for PaintableSurfaceData {
    fn default() -> Self {
        let resolution = 256u32;
        let texel_count = (resolution * resolution) as usize;
        // Default: 100% layer 0
        let weights = vec![[1.0, 0.0, 0.0, 0.0]; texel_count];
        Self {
            layers: vec![
                MaterialLayer {
                    name: "Base".to_string(),
                    ..Default::default()
                },
                MaterialLayer {
                    name: "Layer 2".to_string(),
                    texture_path: Some(DEFAULT_LAYER_SHADER_DARK.to_string()),
                    ..Default::default()
                },
            ],
            splatmap_resolution: resolution,
            splatmap_weights: weights,
            dirty: true,
            shader_dirty: true,
        }
    }
}

/// Brush mode for surface painting.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum PaintBrushType {
    #[default]
    Paint,
    Erase,
    Smooth,
    Fill,
}

/// Resource: surface paint tool settings.
#[derive(Resource)]
pub struct SurfacePaintSettings {
    pub active_layer: usize,
    pub brush_type: PaintBrushType,
    pub brush_radius: f32,
    pub brush_strength: f32,
    pub brush_falloff: f32,
    pub brush_shape: BrushShape,
    pub falloff_type: BrushFalloffType,
}

impl Default for SurfacePaintSettings {
    fn default() -> Self {
        Self {
            active_layer: 0,
            brush_type: PaintBrushType::Paint,
            brush_radius: 0.1,
            brush_strength: 0.5,
            brush_falloff: 1.0,
            brush_shape: BrushShape::Circle,
            falloff_type: BrushFalloffType::Smooth,
        }
    }
}

/// Lightweight layer info for the UI (cached from PaintableSurfaceData).
#[derive(Clone, Debug, Default)]
pub struct LayerPreview {
    pub name: String,
    pub material_source: Option<String>,
}

/// Pending UI commands that get applied to the PaintableSurfaceData by a system.
#[derive(Clone, Debug)]
pub enum SurfacePaintCommand {
    AddLayer,
    RemoveLayer(usize),
    AssignMaterial { layer: usize, path: String },
    ClearMaterial(usize),
}

/// Resource: runtime state for surface painting.
#[derive(Resource, Default)]
pub struct SurfacePaintState {
    pub is_painting: bool,
    pub hover_position: Option<Vec3>,
    pub hover_uv: Option<Vec2>,
    pub active_entity: Option<Entity>,
    /// Whether the brush preview is currently being drawn.
    pub brush_visible: bool,
    /// Cached layer info from the active entity's PaintableSurfaceData (for UI).
    pub layers_preview: Vec<LayerPreview>,
    /// Number of layers on active entity (for UI clamping).
    pub layer_count: usize,
    /// Pending commands from the UI to apply to the component.
    pub pending_commands: Vec<SurfacePaintCommand>,
    /// Tracks the last-seen dragging material path (for drop detection across frame boundaries).
    pub last_dragging_material: Option<String>,
}
