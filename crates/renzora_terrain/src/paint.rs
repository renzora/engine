//! Surface painting — splatmap weight storage and brush operations.
//!
//! Each paintable surface has up to 4 material layers, blended via per-texel
//! RGBA weights. Layers will eventually reference `MaterialGraph` assets
//! compiled with the `TerrainLayer` domain.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::data::{BrushFalloffType, BrushShape, compute_brush_falloff};
use crate::splatmap_material::LayerAnimationType;

// ── Data ─────────────────────────────────────────────────────────────────────

/// A single material layer for surface painting.
#[derive(Clone, Debug, Serialize, Deserialize, Reflect)]
pub struct MaterialLayer {
    pub name: String,
    /// Path to the `.material` graph driving this layer.
    pub material_path: Option<String>,
    pub uv_scale: Vec2,
    pub metallic: f32,
    pub roughness: f32,
    /// Base color for procedural layer shading.
    pub color: Vec3,
    /// Procedural animation type (grass, water, rock, etc.).
    #[serde(default)]
    #[reflect(ignore)]
    pub animation_type: LayerAnimationType,
    /// Animation speed multiplier.
    pub animation_speed: f32,
    /// Albedo texture path (asset-relative).
    #[serde(default)]
    pub albedo_path: Option<String>,
    /// Normal map texture path (asset-relative).
    #[serde(default)]
    pub normal_path: Option<String>,
    /// ARM (AO/Roughness/Metallic) packed texture path (asset-relative).
    #[serde(default)]
    pub arm_path: Option<String>,
    /// Cached compiled WGSL source for this layer (populated at runtime).
    #[serde(skip)]
    #[reflect(ignore)]
    pub cached_shader_source: Option<String>,
}

impl Default for MaterialLayer {
    fn default() -> Self {
        Self {
            name: "Layer".to_string(),
            material_path: None,
            uv_scale: Vec2::splat(0.1),
            metallic: 0.0,
            roughness: 0.5,
            color: Vec3::new(0.5, 0.5, 0.5),
            animation_type: LayerAnimationType::Solid,
            animation_speed: 1.0,
            albedo_path: None,
            normal_path: None,
            arm_path: None,
            cached_shader_source: None,
        }
    }
}

/// Maximum number of texture layers supported.
pub const MAX_LAYERS: usize = 8;

/// Component holding per-mesh surface painting data.
#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component)]
pub struct PaintableSurfaceData {
    /// Material layers (up to 8).
    pub layers: Vec<MaterialLayer>,
    /// Resolution of the splatmap texture (width = height).
    pub splatmap_resolution: u32,
    /// CPU-side weight data. Length = resolution * resolution.
    /// Each element is 8 weights for layers 0..7 (two RGBA splatmaps).
    pub splatmap_weights: Vec<[f32; 8]>,
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
        Self {
            layers: vec![
                MaterialLayer {
                    name: "Grass".to_string(),
                    color: Vec3::new(0.25, 0.50, 0.15),
                    animation_type: LayerAnimationType::Grass,
                    animation_speed: 1.0,
                    roughness: 0.8,
                    ..Default::default()
                },
                MaterialLayer {
                    name: "Dirt".to_string(),
                    color: Vec3::new(0.45, 0.35, 0.22),
                    animation_type: LayerAnimationType::Dirt,
                    roughness: 0.9,
                    ..Default::default()
                },
                MaterialLayer {
                    name: "Water".to_string(),
                    color: Vec3::new(0.10, 0.25, 0.40),
                    animation_type: LayerAnimationType::Water,
                    animation_speed: 1.0,
                    metallic: 0.1,
                    roughness: 0.2,
                    ..Default::default()
                },
                MaterialLayer {
                    name: "Rock".to_string(),
                    color: Vec3::new(0.40, 0.38, 0.35),
                    animation_type: LayerAnimationType::Rock,
                    roughness: 0.9,
                    ..Default::default()
                },
            ],
            splatmap_resolution: resolution,
            splatmap_weights: vec![[1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]; texel_count],
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
    pub brush_visible: bool,
    /// Cached layer info from the active entity's PaintableSurfaceData (for UI).
    pub layers_preview: Vec<LayerPreview>,
    pub layer_count: usize,
    /// Pending commands from the UI to apply to the component.
    pub pending_commands: Vec<SurfacePaintCommand>,
}

// ── Brush Operations ─────────────────────────────────────────────────────────

/// Apply a paint brush stroke at a UV position on a `PaintableSurfaceData`.
///
/// `uv` is the brush center in [0, 1] UV space.
/// `settings` provides brush parameters.
/// `dt` is the frame delta time.
pub fn apply_paint_brush(
    surface: &mut PaintableSurfaceData,
    settings: &SurfacePaintSettings,
    uv: Vec2,
    dt: f32,
) {
    let res = surface.splatmap_resolution;
    let layer = settings.active_layer;
    let num_layers = surface.layers.len().min(MAX_LAYERS);

    if layer >= num_layers {
        return;
    }

    let radius = settings.brush_radius;
    let strength = settings.brush_strength * dt * 4.0;

    // Texel range to iterate
    let min_u = ((uv.x - radius) * res as f32).floor().max(0.0) as u32;
    let max_u = ((uv.x + radius) * res as f32).ceil().min(res as f32 - 1.0) as u32;
    let min_v = ((uv.y - radius) * res as f32).floor().max(0.0) as u32;
    let max_v = ((uv.y + radius) * res as f32).ceil().min(res as f32 - 1.0) as u32;

    for tv in min_v..=max_v {
        for tu in min_u..=max_u {
            let texel_u = (tu as f32 + 0.5) / res as f32;
            let texel_v = (tv as f32 + 0.5) / res as f32;

            let du = texel_u - uv.x;
            let dv = texel_v - uv.y;

            let dist = match settings.brush_shape {
                BrushShape::Circle => (du * du + dv * dv).sqrt(),
                BrushShape::Square => du.abs().max(dv.abs()),
                BrushShape::Diamond => du.abs() + dv.abs(),
            };

            if dist > radius {
                continue;
            }

            let t = dist / radius;
            let falloff = compute_brush_falloff(t, settings.brush_falloff, settings.falloff_type);
            let effect = (strength * falloff).min(1.0);

            let idx = (tv * res + tu) as usize;

            // Work on a local copy to avoid borrow conflicts (Smooth reads neighbors)
            let mut w = surface.splatmap_weights[idx];

            match settings.brush_type {
                PaintBrushType::Paint => {
                    let add = effect * (1.0 - w[layer]);
                    w[layer] += add;
                    let remaining = 1.0 - w[layer];
                    let others_sum: f32 = (0..num_layers)
                        .filter(|&i| i != layer)
                        .map(|i| w[i])
                        .sum();
                    if others_sum > 0.001 {
                        let scale = remaining / others_sum;
                        for i in 0..num_layers {
                            if i != layer {
                                w[i] *= scale;
                            }
                        }
                    }
                }
                PaintBrushType::Erase => {
                    let remove = effect * w[layer];
                    w[layer] -= remove;
                    let share = remove / (num_layers - 1).max(1) as f32;
                    for i in 0..num_layers {
                        if i != layer {
                            w[i] += share;
                        }
                    }
                }
                PaintBrushType::Smooth => {
                    let mut avg = [0.0f32; MAX_LAYERS];
                    let mut count = 0.0f32;
                    for nv in tv.saturating_sub(1)..=(tv + 1).min(res - 1) {
                        for nu in tu.saturating_sub(1)..=(tu + 1).min(res - 1) {
                            let ni = (nv * res + nu) as usize;
                            for c in 0..MAX_LAYERS {
                                avg[c] += surface.splatmap_weights[ni][c];
                            }
                            count += 1.0;
                        }
                    }
                    for c in 0..MAX_LAYERS {
                        avg[c] /= count;
                    }
                    for c in 0..num_layers {
                        w[c] += (avg[c] - w[c]) * effect;
                    }
                }
                PaintBrushType::Fill => {
                    for i in 0..MAX_LAYERS {
                        w[i] = if i == layer { 1.0 } else { 0.0 };
                    }
                }
            }

            // Normalize weights to ensure they sum to 1.0
            let sum: f32 = w[..num_layers].iter().sum();
            if sum > 0.001 {
                for i in 0..num_layers {
                    w[i] /= sum;
                }
            }

            surface.splatmap_weights[idx] = w;
            surface.dirty = true;
        }
    }
}

/// Convert splatmap weights (layers 0-3) to RGBA8 pixel data for GPU upload.
pub fn splatmap_to_rgba8_a(weights: &[[f32; 8]], resolution: u32) -> Vec<u8> {
    let count = (resolution * resolution) as usize;
    let mut pixels = Vec::with_capacity(count * 4);
    for w in weights.iter().take(count) {
        pixels.push((w[0] * 255.0).round() as u8);
        pixels.push((w[1] * 255.0).round() as u8);
        pixels.push((w[2] * 255.0).round() as u8);
        pixels.push((w[3] * 255.0).round() as u8);
    }
    pixels
}

/// Convert splatmap weights (layers 4-7) to RGBA8 pixel data for GPU upload.
pub fn splatmap_to_rgba8_b(weights: &[[f32; 8]], resolution: u32) -> Vec<u8> {
    let count = (resolution * resolution) as usize;
    let mut pixels = Vec::with_capacity(count * 4);
    for w in weights.iter().take(count) {
        pixels.push((w[4] * 255.0).round() as u8);
        pixels.push((w[5] * 255.0).round() as u8);
        pixels.push((w[6] * 255.0).round() as u8);
        pixels.push((w[7] * 255.0).round() as u8);
    }
    pixels
}
