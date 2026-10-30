//! Terrain graphs: a node graph that builds a terrain's base heights.
//!
//! A `.tgraph` file ([`format`]) is a recipe: primitives (noise, mountain
//! ranges, dunes, craters, an imported heightmap...), shape nodes that combine
//! and adjust them, simulations (erosion, rivers, lakes, snow), data nodes that
//! measure masks (slope, height, flow...), and outputs: the terrain's heights
//! and any number of named masks for paint layers and foliage. The node set is
//! [`catalog`]; [`eval`] runs a graph on the CPU at any resolution,
//! deterministically for a seed, in the background, cancellable, caching every
//! node's result.
//!
//! A terrain points at a graph with [`TerrainGenerator`]. Building it writes
//! the terrain's generator layer, and hand sculpting lives on a layer above
//! that ([`apply`]), so a rebuild never throws sculpting away.
//!
//! This crate has no editor dependency: it is the format, the evaluator, the
//! components, the apply step, and the world-level build and commit
//! ([`world`]) the editor's background build and the MCP tool share. The node editor, background builds, undo
//! and previews are `renzora_terrain_graph_editor`. A shipped game carries the
//! components (a saved terrain names its graph) and builds nothing: its
//! heights are already in the scene.

pub mod apply;
pub mod catalog;
pub mod component;
pub mod eval;
pub mod field;
pub mod format;
pub mod presets;
pub mod world;

use bevy::prelude::*;

pub use apply::{apply_generator, gather_generator_layer, reset_sculpt, terrain_grid, ApplyReport, TerrainGeneratorLayer};
pub use component::{
    exposed_params, BuildState, BuildStatus, BuildTerrainGraph, ExposedValue, GeneratorOverride, TerrainGenerator,
    TerrainGraphBuilds, TerrainGraphOutput,
};
pub use eval::{evaluate, Cancelled, EvalCache, EvalControl, EvalRequest, EvalResult, EvalScope};
pub use field::{Field, Grid};
pub use format::{ParamValue, TerrainGraph};

/// The file extension, without the dot.
pub const EXTENSION: &str = "tgraph";

#[derive(Default)]
pub struct TerrainGraphPlugin;

impl Plugin for TerrainGraphPlugin {
    fn build(&self, app: &mut App) {
        info!("[runtime] TerrainGraphPlugin");
        app.init_asset::<TerrainGraph>()
            .init_asset_loader::<format::TerrainGraphLoader>()
            .register_type::<TerrainGenerator>()
            .register_type::<GeneratorOverride>()
            .register_type::<TerrainGeneratorLayer>()
            .add_message::<BuildTerrainGraph>()
            .init_resource::<TerrainGraphBuilds>();
    }
}

renzora::add!(TerrainGraphPlugin);
