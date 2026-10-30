//! The old overlay paint layers, kept only so scenes saved with them load.
//!
//! A terrain used to be painted with a stack of [`PaintLayer`]s: a coverage
//! mask and a material each, drawn as see-through overlay meshes laid on the
//! ground. Painting now writes layer weights into the terrain's landscape
//! material instead (`crate::landscape`), and `landscape::migrate` converts a
//! saved [`Painter`] into those weights the first time the scene is opened in
//! the editor, then removes it. Nothing builds overlays any more.
//!
//! The types stay registered for reflection, with their field defaults,
//! because a scene that holds a component its loader cannot reconstruct fails
//! to load at all.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// One old paint layer: mask + material path + offset + on/off.
///
/// The mask is row-major over the whole terrain, a square grid of
/// `grid_size()` vertices spanning the terrain's larger side from its corner.
#[derive(Clone, Debug, Reflect, Serialize, Deserialize)]
pub struct PaintLayer {
    pub name: String,
    pub material_path: Option<String>,
    pub mask: Vec<f32>,
    pub coverage_threshold: f32,
    pub height_offset: f32,
    /// Ground covered by one repeat of the material, in metres.
    ///
    /// Scenes painted before this field existed have no value for it, and both
    /// attributes are needed to survive that: `serde` covers the config path,
    /// and `reflect` covers the scene path, which is the one that matters here.
    /// A scene reconstructs its components through `FromReflect`, which fails
    /// *outright* on a field the data doesn't carry unless it is marked
    /// `#[reflect(default)]`.
    #[serde(default = "default_tile_size")]
    #[reflect(default = "default_tile_size")]
    pub tile_size: f32,
    pub enabled: bool,
}

/// Metres of ground per material repeat on an old layer without the field.
pub const DEFAULT_TILE_SIZE: f32 = 2.0;

fn default_tile_size() -> f32 {
    DEFAULT_TILE_SIZE
}

impl PaintLayer {
    /// An unpainted layer over a `grid_cells` square mask.
    pub fn empty(name: impl Into<String>, grid_cells: u32) -> Self {
        Self {
            name: name.into(),
            material_path: None,
            mask: vec![0.0; (grid_cells * grid_cells) as usize],
            coverage_threshold: 0.01,
            height_offset: 0.02,
            tile_size: DEFAULT_TILE_SIZE,
            enabled: true,
        }
    }

    /// Vertices along each side of the mask.
    pub fn grid_size(&self) -> u32 {
        (self.mask.len() as f32).sqrt().round() as u32
    }
}

/// The old stack of paint layers on a terrain. See the module docs.
#[derive(Component, Clone, Debug, Default, Reflect, Serialize, Deserialize)]
#[reflect(Component)]
pub struct Painter {
    pub layers: Vec<PaintLayer>,
    pub active_layer: Option<usize>,
}

/// On an overlay entity an older editor build spawned for a layer. Never
/// saved; migration despawns any that are still about.
#[derive(Component, Clone, Copy, Debug)]
pub struct PainterLayerMesh {
    pub painter: Entity,
    pub layer_index: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Scenes reconstruct their components through `FromReflect`, which fails
    /// outright on an absent field unless it carries `#[reflect(default)]`.
    #[test]
    fn a_layer_without_the_tile_size_field_reconstructs_by_reflection() {
        use bevy::reflect::structs::DynamicStruct;
        use bevy::reflect::FromReflect;

        let mut old = DynamicStruct::default();
        old.insert("name", "Layer 1".to_string());
        old.insert("material_path", Some("water.material".to_string()));
        old.insert("mask", vec![0.0f32; 4]);
        old.insert("coverage_threshold", 0.01f32);
        old.insert("height_offset", 0.02f32);
        old.insert("enabled", true);

        let layer = PaintLayer::from_reflect(&old).expect("a missing tile size must default, not fail the load");
        assert_eq!(layer.name, "Layer 1");
        assert_eq!(layer.tile_size, DEFAULT_TILE_SIZE);
        assert_eq!(layer.grid_size(), 2);
    }
}
