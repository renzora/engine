//! Building a terrain from its graph, against a live `World`.
//!
//! Two callers, one commit. The editor builds in the background (it gathers
//! a request here with [`build_request`], evaluates on a task, and lands the
//! result with [`commit`]); the MCP tool builds synchronously with
//! [`build_now`], which is the same three steps on the calling thread. Either
//! way the commit is one step on the scene's undo stack.
//!
//! Undo goes through the contract crate's `renzora::undo`, which records
//! nothing when there is no undo system (a game), so this module needs no
//! editor crate.

use std::path::PathBuf;
use std::sync::Arc;

use bevy::prelude::*;
use renzora::undo::{UndoCommand, UndoContext};
use renzora_terrain::data::{TerrainChunkData, TerrainChunkOf, TerrainData};

use crate::apply::{apply_generator, gather_generator_layer, terrain_grid, TerrainGeneratorLayer};
use crate::component::{TerrainGenerator, TerrainGraphOutput};
use crate::eval::{evaluate, fold_seed, EvalCache, EvalControl, EvalRequest, EvalResult, EvalScope};
use crate::format::TerrainGraph;

/// The open project's folder, which relative graph and heightmap paths
/// resolve from.
pub fn project_root(world: &World) -> Option<PathBuf> {
    world.get_resource::<renzora::core::CurrentProject>().map(|p| p.path.clone())
}

/// Read and parse a `.tgraph`, project-relative or absolute.
pub fn load_graph(world: &World, path: &str) -> Result<TerrainGraph, String> {
    if path.trim().is_empty() {
        return Err("the terrain has no graph; pick a .tgraph".into());
    }
    let fs = match world.get_resource::<renzora::core::CurrentProject>() {
        Some(p) => p.resolve_path(path),
        None => PathBuf::from(path),
    };
    let text = std::fs::read_to_string(&fs).map_err(|e| format!("{}: {e}", fs.display()))?;
    TerrainGraph::parse(&text).map_err(|e| format!("{path}: {e}"))
}

/// This terrain's chunk entities.
pub fn terrain_chunks(world: &mut World, terrain: Entity) -> Vec<Entity> {
    let mut q = world.query::<(Entity, &TerrainChunkOf)>();
    q.iter(world).filter(|(_, of)| of.0 == terrain).map(|(e, _)| e).collect()
}

/// Everything a full-resolution build of `terrain` needs, ready to move onto
/// a background task.
pub fn build_request(world: &mut World, terrain: Entity, generator: &TerrainGenerator) -> Result<EvalRequest, String> {
    let data = world.get::<TerrainData>(terrain).cloned().ok_or("not a terrain")?;
    let chunks = terrain_chunks(world, terrain);
    let current = {
        let pairs: Vec<(&TerrainChunkData, Option<&TerrainGeneratorLayer>)> = chunks
            .iter()
            .filter_map(|e| world.get::<TerrainChunkData>(*e).map(|c| (c, world.get::<TerrainGeneratorLayer>(*e))))
            .collect();
        gather_generator_layer(&data, &pairs)
    };
    let mut req = EvalRequest::new(terrain_grid(&data), fold_seed(generator.seed));
    req.overrides = generator.override_pairs();
    req.current = Some(Arc::new(current));
    req.root = project_root(world);
    req.scope = EvalScope::Outputs;
    Ok(req)
}

/// A terrain's heights as a build changes them: every chunk's base and
/// generator layer, and the height range (a build may grow it).
#[derive(Clone)]
struct Snapshot {
    terrain: Entity,
    range: (f32, f32),
    chunks: Vec<(Entity, Vec<f32>, Vec<f32>)>,
}

fn snapshot(world: &World, terrain: Entity, chunks: &[Entity]) -> Snapshot {
    let range = world.get::<TerrainData>(terrain).map_or((0.0, 1.0), |d| (d.min_height, d.max_height));
    Snapshot {
        terrain,
        range,
        chunks: chunks
            .iter()
            .filter_map(|e| {
                let c = world.get::<TerrainChunkData>(*e)?;
                let layer = world.get::<TerrainGeneratorLayer>(*e).map(|l| l.heights.clone()).unwrap_or_default();
                Some((*e, c.base_heights.clone(), layer))
            })
            .collect(),
    }
}

fn restore(world: &mut World, s: &Snapshot) {
    if let Some(mut d) = world.get_mut::<TerrainData>(s.terrain) {
        if (d.min_height, d.max_height) != s.range {
            d.min_height = s.range.0;
            d.max_height = s.range.1;
        }
    }
    for (e, base, layer) in &s.chunks {
        if let Some(mut c) = world.get_mut::<TerrainChunkData>(*e) {
            c.base_heights = base.clone();
            c.dirty = true;
        }
        if let Ok(mut em) = world.get_entity_mut(*e) {
            em.insert(TerrainGeneratorLayer { heights: layer.clone() });
        }
    }
}

/// The undo step of one build.
struct BuildUndo {
    label: String,
    before: Snapshot,
    after: Snapshot,
}

impl UndoCommand for BuildUndo {
    fn label(&self) -> &str {
        &self.label
    }

    fn execute(&mut self, world: &mut World) {
        restore(world, &self.after);
    }

    fn undo(&mut self, world: &mut World) {
        restore(world, &self.before);
    }
}

/// Land a finished build in the terrain: the generator layer replaced, the
/// sculpt layer kept, the masks on [`TerrainGraphOutput`], one undo step.
/// Returns a one-line summary.
pub fn commit(world: &mut World, terrain: Entity, result: &EvalResult, label: &str) -> Result<String, String> {
    let Some(height) = result.height.as_ref() else {
        return Err(result
            .errors
            .first()
            .map(|(_, e)| e.clone())
            .unwrap_or_else(|| "the graph has no Terrain Output".into()));
    };
    let mut data = world.get::<TerrainData>(terrain).cloned().ok_or("not a terrain")?;
    let chunks = terrain_chunks(world, terrain);
    if chunks.is_empty() {
        return Err("the terrain has no chunks yet".into());
    }
    // A chunk built for the first time needs somewhere to keep its layer.
    for e in &chunks {
        if world.get::<TerrainGeneratorLayer>(*e).is_none() {
            world.entity_mut(*e).insert(TerrainGeneratorLayer::default());
        }
    }
    let before = snapshot(world, terrain, &chunks);
    let report = {
        let mut q = world.query::<(Entity, &mut TerrainChunkData, &mut TerrainGeneratorLayer)>();
        let mut pairs: Vec<(&mut TerrainChunkData, &mut TerrainGeneratorLayer)> = q
            .iter_mut(world)
            .filter(|(e, ..)| chunks.contains(e))
            .map(|(_, c, l)| (c.into_inner(), l.into_inner()))
            .collect();
        apply_generator(&mut data, &mut pairs, height)
    };
    if let Some(mut d) = world.get_mut::<TerrainData>(terrain) {
        if (d.min_height, d.max_height) != (data.min_height, data.max_height) {
            d.min_height = data.min_height;
            d.max_height = data.max_height;
        }
    }
    let after = snapshot(world, terrain, &chunks);
    renzora::undo::record(world, UndoContext::Scene, Box::new(BuildUndo { label: label.to_string(), before, after }));
    renzora::undo::seal(world, &UndoContext::Scene);

    world.entity_mut(terrain).insert(TerrainGraphOutput {
        grid: result.grid,
        masks: result.masks.iter().map(|(n, f)| (n.clone(), Arc::new(f.clone()))).collect(),
    });

    let (lo, hi) = height.min_max();
    let mut summary = format!(
        "built {} chunks, {:.0} to {:.0} m, {} mask{}",
        report.chunks,
        lo,
        hi,
        result.masks.len(),
        if result.masks.len() == 1 { "" } else { "s" }
    );
    if report.kept_sculpt > 0 {
        summary.push_str(", sculpting kept");
    }
    if let Some((a, b)) = report.range_grown {
        summary.push_str(&format!(", height range grown to {a:.0}..{b:.0} m"));
    }
    let problems: Vec<&str> = result.errors.iter().map(|(_, e)| e.as_str()).collect();
    if !problems.is_empty() {
        summary.push_str(&format!(" ({})", problems.join("; ")));
    }
    Ok(summary)
}

/// Build `terrain` from its [`TerrainGenerator`] on this thread: gather,
/// evaluate at full resolution, commit. What the MCP tool runs.
pub fn build_now(world: &mut World, terrain: Entity) -> Result<String, String> {
    let generator = world.get::<TerrainGenerator>(terrain).cloned().ok_or("the terrain has no Terrain Generator")?;
    let graph = load_graph(world, &generator.graph)?;
    let req = build_request(world, terrain, &generator)?;
    let result = evaluate(&graph, &req, &mut EvalCache::with_capacity(0), &EvalControl::new()).map_err(|_| "cancelled".to_string())?;
    commit(world, terrain, &result, "Build Terrain Graph")
}
