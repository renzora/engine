//! Terrain texturing: one landscape material, painted as layer weights.
//!
//! A terrain root's [`TerrainMaterial`] names a node-graph `.material` built
//! from the Landscape nodes (Layer Coords, Layer Blend, Layer Sample, Slope).
//! The material decides what each layer looks like and how layers mix; the
//! terrain holds how much of each layer every point has, as painted weights
//! ([`TerrainChunkWeights`]). Nothing about the look is baked into the
//! terrain, so editing the material re-textures it at once, and painting never
//! touches the material.
//!
//! # For the editor
//!
//! * [`TerrainLandscape`] on a terrain root: the material's layers in order,
//!   each with the weight channel it is painted in and its coverage (0..1), and
//!   whether the material has loaded ([`LandscapeStatus`]).
//! * [`assign_material`]: point a terrain at a `.material`. Not an undo step
//!   by itself; the call site wraps it in one when it should be.
//! * [`weights::apply_weight_dab`]: one dab of the weight brush over a
//!   terrain's chunks; [`LandscapeStroke`] brackets a stroke so the brush
//!   records what it changed for undo, and saving waits until the stroke ends.
//! * [`migrate::ensure_alpine_material`]: put the default material into a
//!   project.
//!
//! # Rendering
//!
//! [`instance`] gives each terrain its own instance of the shared material
//! with its weight atlas bound, and dresses every chunk with it; [`gpu`] keeps
//! the atlas current with in-place texture writes; [`migrate`] converts scenes
//! saved with the old surface or overlay paint.

pub mod gpu;
pub mod instance;
pub mod migrate;
pub mod weights;

use std::collections::HashMap;

use bevy::prelude::*;

pub use instance::{
    assign_channels, read_material_layers, LandscapeLayer, LandscapeStatus, TerrainLandscape, TerrainMaterial,
    TerrainMaterialProbe, TerrainMaterialState,
};
pub use weights::{TerrainChunkWeights, WeightDab, WEIGHT_CHANNELS};

/// A weight-brush stroke in progress.
///
/// The brush calls [`LandscapeStroke::record`] before it first changes a
/// chunk during a stroke, so undo holds only the chunks the stroke touched
/// (on a large terrain, a copy of every chunk's weights per stroke would be
/// tens of megabytes). While `active`, changed chunks are not re-encoded for
/// saving; that happens once, when the stroke ends.
#[derive(Resource, Default)]
pub struct LandscapeStroke {
    pub active: bool,
    /// Each touched chunk's weights before the stroke: `(resolution,
    /// texels)`, empty texels for a chunk that had never been painted.
    pub before: HashMap<Entity, (u32, Vec<weights::Texel>)>,
}

impl LandscapeStroke {
    /// Remember `chunk`'s weights before the stroke first changes them.
    pub fn record(&mut self, chunk: Entity, weights: &TerrainChunkWeights) {
        self.before
            .entry(chunk)
            .or_insert_with(|| (weights.resolution, weights.texels.clone()));
    }
}

/// Point `terrain` at the landscape material at `path` (project-relative), or
/// clear it with an empty path. The terrain keeps its painted weights either
/// way: layers are matched to them by name.
pub fn assign_material(world: &mut World, terrain: Entity, path: &str) -> Result<(), String> {
    if world.get::<crate::data::TerrainData>(terrain).is_none() {
        return Err(format!("{terrain} is not a terrain"));
    }
    let mut e = world.entity_mut(terrain);
    match e.get_mut::<TerrainMaterial>() {
        Some(mut m) => {
            if m.path != path {
                m.path = path.to_string();
            }
        }
        None => {
            e.insert(TerrainMaterial::new(path));
        }
    }
    Ok(())
}

pub struct LandscapePlugin;

impl Plugin for LandscapePlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<TerrainMaterial>()
            .register_type::<TerrainChunkWeights>()
            .init_resource::<LandscapeStroke>()
            .init_resource::<gpu::TerrainWeightPatches>()
            .add_systems(
                Update,
                (
                    migrate::migrate_old_terrains.run_if(in_editor_with_project),
                    instance::retry_failed_materials,
                    instance::follow_terrain_materials,
                    instance::ensure_chunk_weights,
                    instance::sync_weight_atlases,
                    instance::build_instances,
                    instance::dress_landscape_chunks.after(crate::mesh::rehydrate_terrain_chunks),
                    instance::keep_twins_untraced,
                    instance::encode_weights,
                )
                    .chain(),
            );
    }

    fn finish(&self, app: &mut App) {
        gpu::install_render_upload(app);
    }
}

/// Migration writes project files, so it waits for the editor and a project.
fn in_editor_with_project(session: Option<Res<renzora::EditorSession>>, project: Option<Res<renzora::CurrentProject>>) -> bool {
    session.is_some_and(|s| s.is_editor()) && project.is_some()
}
