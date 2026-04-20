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
///
/// Each layer holds its own coverage `mask` (at splatmap resolution). The
/// paint tool stamps discs into the active layer's mask, independent of
/// other layers — no cross-layer weight normalization. The final splatmap
/// weights uploaded to the GPU are derived top-down: each upper layer takes
/// `min(mask, remaining_coverage)` of each texel, and layer 0 (the base)
/// absorbs whatever's left. Toggling `enabled` off hides the layer without
/// destroying its mask.
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
    /// Coverage mask at splatmap resolution. Paint tool stamps into this.
    /// Empty on load — populated by `ensure_masks_sized_system` to match the
    /// current splatmap resolution (1.0 everywhere for layer 0, 0.0 for others).
    #[serde(default)]
    pub mask: Vec<f32>,
    /// Normalized height delta applied where `mask` is 1.0. Composed into
    /// `TerrainChunkData::heights` so sculpting the base doesn't overwrite
    /// the carve. Negative carves down, positive raises.
    #[serde(default)]
    pub carve_depth: f32,
    /// Layer on/off — hide the layer's contribution to splatmap + carve
    /// without clearing the authored mask.
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    /// Cached compiled WGSL source for this layer (populated at runtime).
    #[serde(skip)]
    #[reflect(ignore)]
    pub cached_shader_source: Option<String>,
}

fn default_enabled() -> bool {
    true
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
            mask: Vec::new(),
            carve_depth: 0.0,
            enabled: true,
            cached_shader_source: None,
        }
    }
}

/// Maximum number of texture layers supported.
pub const MAX_LAYERS: usize = 8;

/// Component holding per-mesh surface painting data.
///
/// Authoritative data is per-layer coverage masks on each `MaterialLayer`.
/// The paint tool stamps into the active layer's mask; the `splatmap_weights`
/// buffer is derived top-down from the masks by a composition system and
/// uploaded to the GPU. This is non-destructive: disabling or deleting a
/// layer just hides its mask contribution.
#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component)]
pub struct PaintableSurfaceData {
    /// Material layers (up to 8). Layer 0 is the terrain base.
    pub layers: Vec<MaterialLayer>,
    /// Resolution of the splatmap texture (width = height). Masks are stored
    /// at this resolution per layer.
    pub splatmap_resolution: u32,
    /// Derived top-down composite of layer masks — what the GPU uploads.
    #[serde(skip, default)]
    #[reflect(ignore)]
    pub splatmap_weights: Vec<[f32; 8]>,
    /// Whether the splatmap needs recomposing + uploading to GPU.
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
            splatmap_weights: Vec::new(),
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
    pub carve_depth: f32,
    pub enabled: bool,
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
/// Stamps a disc into the currently-active layer's mask. Stamping over
/// already-painted coverage is idempotent (`max`), so dragging the brush
/// over the same spot doesn't amplify. Erase clamps to zero. Smooth blurs
/// the active layer's mask against itself. Other layers are untouched.
///
/// `uv` is the brush center in `[0, 1]` UV space. `dt` is frame delta time.
pub fn apply_paint_brush(
    surface: &mut PaintableSurfaceData,
    settings: &SurfacePaintSettings,
    uv: Vec2,
    dt: f32,
) {
    let res = surface.splatmap_resolution;
    let layer = settings.active_layer;
    if layer >= surface.layers.len() {
        return;
    }

    let radius = settings.brush_radius;
    let strength = (settings.brush_strength * dt * 4.0).clamp(0.0, 1.0);

    // Ensure the active layer's mask is sized before we stamp.
    ensure_layer_mask(&mut surface.layers[layer], res, layer == 0);

    let min_u = ((uv.x - radius) * res as f32).floor().max(0.0) as u32;
    let max_u = ((uv.x + radius) * res as f32).ceil().min(res as f32 - 1.0) as u32;
    let min_v = ((uv.y - radius) * res as f32).floor().max(0.0) as u32;
    let max_v = ((uv.y + radius) * res as f32).ceil().min(res as f32 - 1.0) as u32;

    // Smooth requires reading neighbours from a snapshot to avoid the feedback
    // loop of reading already-updated values. Copy once up-front.
    let mask_snapshot = if matches!(settings.brush_type, PaintBrushType::Smooth) {
        Some(surface.layers[layer].mask.clone())
    } else {
        None
    };

    let mask = &mut surface.layers[layer].mask;

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
            let contribution = (strength * falloff).clamp(0.0, 1.0);
            let idx = (tv * res + tu) as usize;
            let current = mask[idx];

            mask[idx] = match settings.brush_type {
                PaintBrushType::Paint => current.max(contribution),
                PaintBrushType::Erase => (current - contribution).max(0.0),
                PaintBrushType::Fill => 1.0,
                PaintBrushType::Smooth => {
                    let snap = mask_snapshot.as_ref().unwrap();
                    let mut sum = 0.0f32;
                    let mut count = 0.0f32;
                    for nv in tv.saturating_sub(1)..=(tv + 1).min(res - 1) {
                        for nu in tu.saturating_sub(1)..=(tu + 1).min(res - 1) {
                            let ni = (nv * res + nu) as usize;
                            sum += snap[ni];
                            count += 1.0;
                        }
                    }
                    let avg = sum / count;
                    current + (avg - current) * contribution
                }
            };
        }
    }
    surface.dirty = true;
}

/// Ensure a layer's mask matches the splatmap resolution. `is_base` seeds the
/// mask at 1.0 (terrain base is covered everywhere); other layers start empty.
pub fn ensure_layer_mask(layer: &mut MaterialLayer, resolution: u32, is_base: bool) {
    let texel_count = (resolution * resolution) as usize;
    if layer.mask.len() != texel_count {
        let fill = if is_base { 1.0 } else { 0.0 };
        layer.mask = vec![fill; texel_count];
    }
}

/// Derive the GPU-uploaded splatmap weights from each layer's mask.
///
/// Top-down compositing: starting from the topmost layer, each layer takes
/// `min(layer.mask[texel], remaining_coverage)` of that texel. Layer 0
/// always receives whatever coverage wasn't claimed so the base material
/// fills any gaps.
pub fn derive_splatmap_weights(surface: &mut PaintableSurfaceData) {
    let res = surface.splatmap_resolution;
    let texel_count = (res * res) as usize;
    let num_layers = surface.layers.len().min(MAX_LAYERS);

    // Make sure every layer has a correctly-sized mask before we read them.
    for (i, layer) in surface.layers.iter_mut().enumerate() {
        ensure_layer_mask(layer, res, i == 0);
    }

    if surface.splatmap_weights.len() != texel_count {
        surface.splatmap_weights = vec![[0.0; MAX_LAYERS]; texel_count];
    }

    for texel in 0..texel_count {
        let mut out = [0.0f32; MAX_LAYERS];
        let mut remaining = 1.0f32;
        for lidx in (1..num_layers).rev() {
            let layer = &surface.layers[lidx];
            if !layer.enabled || remaining <= 0.0 {
                continue;
            }
            let coverage = layer.mask[texel].clamp(0.0, 1.0);
            let taken = coverage.min(remaining);
            out[lidx] = taken;
            remaining -= taken;
        }
        out[0] = remaining.max(0.0);
        surface.splatmap_weights[texel] = out;
    }
}

/// System: recompose `splatmap_weights` from layer masks whenever a
/// `PaintableSurfaceData` is dirty. Only flags the chunk heightmap as dirty
/// when at least one enabled layer has a non-zero `carve_depth` — otherwise
/// the mesh doesn't need rebuilding and we save the per-frame mesh regen
/// cost while painting. Runs before height composition + GPU upload.
pub fn derive_splatmap_weights_system(
    mut surfaces: Query<(&mut PaintableSurfaceData, Option<&mut crate::data::TerrainChunkData>)>,
) {
    for (mut surface, chunk) in surfaces.iter_mut() {
        if !surface.dirty {
            continue;
        }
        derive_splatmap_weights(surface.as_mut());
        let any_carve = surface
            .layers
            .iter()
            .any(|l| l.enabled && l.carve_depth != 0.0);
        if any_carve {
            if let Some(mut chunk) = chunk {
                chunk.dirty = true;
            }
        }
    }
}

/// Mark freshly-spawned/loaded `PaintableSurfaceData` dirty so the first
/// frame rebuilds the composed splatmap from its authored masks.
pub fn mark_new_surfaces_dirty_system(
    mut surfaces: Query<&mut PaintableSurfaceData, Added<PaintableSurfaceData>>,
) {
    for mut surface in surfaces.iter_mut() {
        surface.dirty = true;
        surface.shader_dirty = true;
    }
}

/// Convert splatmap weights (layers 0-3) to RGBA8 pixel data for GPU upload.
///
/// Always returns exactly `resolution * resolution * 4` bytes. If `weights`
/// is short (e.g. a freshly-spawned surface before derivation has run),
/// missing texels are filled with the base-layer default (layer 0 = 255,
/// rest = 0) so the texture is valid for GPU upload.
pub fn splatmap_to_rgba8_a(weights: &[[f32; 8]], resolution: u32) -> Vec<u8> {
    let count = (resolution * resolution) as usize;
    let mut pixels = Vec::with_capacity(count * 4);
    for i in 0..count {
        if let Some(w) = weights.get(i) {
            pixels.push((w[0] * 255.0).round() as u8);
            pixels.push((w[1] * 255.0).round() as u8);
            pixels.push((w[2] * 255.0).round() as u8);
            pixels.push((w[3] * 255.0).round() as u8);
        } else {
            pixels.extend_from_slice(&[255, 0, 0, 0]);
        }
    }
    pixels
}

/// Convert splatmap weights (layers 4-7) to RGBA8 pixel data for GPU upload.
///
/// Always returns exactly `resolution * resolution * 4` bytes. Missing
/// texels are filled with zeros (layers 4-7 are off by default).
pub fn splatmap_to_rgba8_b(weights: &[[f32; 8]], resolution: u32) -> Vec<u8> {
    let count = (resolution * resolution) as usize;
    let mut pixels = Vec::with_capacity(count * 4);
    for i in 0..count {
        if let Some(w) = weights.get(i) {
            pixels.push((w[4] * 255.0).round() as u8);
            pixels.push((w[5] * 255.0).round() as u8);
            pixels.push((w[6] * 255.0).round() as u8);
            pixels.push((w[7] * 255.0).round() as u8);
        } else {
            pixels.extend_from_slice(&[0, 0, 0, 0]);
        }
    }
    pixels
}
