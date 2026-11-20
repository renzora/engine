//! Whole-terrain height operations that record one undo step each: the noise
//! generator (the MCP's `generate_terrain`) and Flatten all (the Manage tab).
//!
//! This was also the Generate scene tool, a rectangle dragged on the terrain
//! with a wireframe preview and a toolbar of noise settings. The Terrain
//! component's generator is a terrain graph now (Manage tab, `.tgraph`), so the
//! scene tool and its toolbar are gone; the generator itself
//! ([`renzora_terrain::generate`]) stays, because scripted callers still
//! generate noise into a region by number.

use bevy::prelude::*;

use renzora_terrain::data::{TerrainChunkData, TerrainChunkOf, TerrainData};
use renzora_terrain::generate::{apply_to_chunk, TerrainGenSettings};

/// One chunk's base heightmap, keyed by the chunk *entity*.
///
/// By entity and not by `(chunk_x, chunk_z)`: a scene can hold more than one
/// terrain, and two of them will happily both have a chunk (0, 0).
#[derive(Clone)]
pub struct GenerateUndo {
    chunks: Vec<(Entity, Vec<f32>)>,
}

fn restore_generated(world: &mut World, entry: &GenerateUndo) {
    for (entity, heights) in &entry.chunks {
        if let Some(mut chunk) = world.get_mut::<TerrainChunkData>(*entity) {
            chunk.base_heights = heights.clone();
            chunk.dirty = true;
        }
    }
}

/// The chunks `terrain` owns.
fn owned_chunks(world: &mut World, terrain: Entity) -> Vec<Entity> {
    let mut q = world.query::<(Entity, &TerrainChunkOf)>();
    q.iter(world).filter(|(_, owner)| owner.0 == terrain).map(|(e, _)| e).collect()
}

fn snapshot(world: &World, owned: &[Entity]) -> GenerateUndo {
    GenerateUndo {
        chunks: owned
            .iter()
            .filter_map(|e| world.get::<TerrainChunkData>(*e).map(|c| (*e, c.base_heights.clone())))
            .collect(),
    }
}

fn record(world: &mut World, label: &str, before: GenerateUndo, after: GenerateUndo) {
    renzora_undo::record(
        world,
        renzora_undo::UndoContext::Scene,
        Box::new(renzora_undo::SnapshotCmd {
            label: label.to_string(),
            before,
            after,
            restore: restore_generated,
            // One press is one step, always: nothing to merge with.
            merge_key: None,
        }),
    );
    renzora_undo::seal(world, &renzora_undo::UndoContext::Scene);
}

/// Run the noise generator (with the current [`TerrainGenSettings`]) over
/// `terrain` and record it as one undo step.
///
/// Exclusive, because it snapshots every chunk twice and calls into
/// `renzora_undo`, neither of which belongs inside a system holding queries
/// over the chunks.
pub fn generate_now(world: &mut World, terrain: Entity) {
    let Some(data) = world.get::<TerrainData>(terrain).cloned() else { return };
    let settings = world.get_resource::<TerrainGenSettings>().cloned().unwrap_or_default();
    let region = settings.region(&data);
    // One pass, so the snapshot and the apply agree on the set even if
    // something else spawns a chunk in between.
    let owned = owned_chunks(world, terrain);
    let before = snapshot(world, &owned);

    // Chunks are independent, so they generate in parallel: on one thread an
    // 8 km terrain (4096 chunks at 129, nine octaves) held the editor for 19 s.
    let changed = std::sync::atomic::AtomicBool::new(false);
    let mut q = world.query::<(&TerrainChunkOf, &mut TerrainChunkData)>();
    q.par_iter_mut(world).for_each(|(of, mut chunk)| {
        if of.0 == terrain && apply_to_chunk(&mut chunk, &data, &settings, &region) {
            changed.store(true, std::sync::atomic::Ordering::Relaxed);
        }
    });
    if !changed.into_inner() {
        return;
    }
    let after = snapshot(world, &owned);
    record(world, "Generate Terrain", before, after);
}

/// Flatten the terrain's sculpt layer to one level: the generator settings'
/// `base`, in the terrain's own metres (0 m by default).
pub fn reset_now(world: &mut World, terrain: Entity) {
    let Some(data) = world.get::<TerrainData>(terrain).cloned() else { return };
    let base = world.get_resource::<TerrainGenSettings>().map(|g| g.base).unwrap_or(0.0);
    let range = data.height_range();
    if range <= 0.0 {
        return;
    }
    let level = ((base - data.min_height) / range).clamp(0.0, 1.0);

    let owned = owned_chunks(world, terrain);
    let before = snapshot(world, &owned);
    let mut changed = false;
    for entity in &owned {
        let Some(mut chunk) = world.get_mut::<TerrainChunkData>(*entity) else { continue };
        if chunk.base_heights.iter().all(|h| (*h - level).abs() < 1e-6) {
            continue;
        }
        chunk.base_heights.fill(level);
        chunk.dirty = true;
        changed = true;
    }
    if !changed {
        return;
    }
    let after = snapshot(world, &owned);
    record(world, "Flatten Terrain", before, after);
}
