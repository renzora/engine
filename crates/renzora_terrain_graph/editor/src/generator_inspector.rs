//! Parsed generator graphs for the Terrain component's generator card.
//!
//! The card lives in the Terrain component's Manage tab
//! (`renzora_terrain_editor::panel::manage`), which replaced this module's
//! own Terrain Generator inspector section. It needs each generator's graph
//! for the exposed parameters and the node count, and a binding runs every
//! frame with only read access, so [`GraphFiles`] keeps each generator's graph
//! parsed and re-reads it only when the file's modified time moves.

use std::collections::HashMap;

use bevy::prelude::*;
use renzora_terrain_graph::{TerrainGenerator, TerrainGraph};

use crate::state::TerrainGraphEditorState;

/// Parsed graphs by project-relative path, with the file time they were read at.
#[derive(Resource, Default)]
pub struct GraphFiles {
    map: HashMap<String, (Option<std::time::SystemTime>, TerrainGraph)>,
}

impl GraphFiles {
    pub fn get(&self, path: &str) -> Option<&TerrainGraph> {
        self.map.get(path).map(|(_, g)| g)
    }
}

/// Keep [`GraphFiles`] current for every generator in the scene. The open
/// document's working copy wins over its file, so the card follows unsaved
/// edits too.
pub fn refresh_graph_files(world: &mut World) {
    let mut q = world.query::<&TerrainGenerator>();
    let paths: Vec<String> = q.iter(world).map(|g| g.graph.clone()).filter(|p| !p.is_empty()).collect();
    let open = world.get_resource::<TerrainGraphEditorState>().filter(|s| s.is_open()).and_then(|s| s.path.clone().map(|p| (p, s.graph.clone())));
    let root = world.get_resource::<renzora::core::CurrentProject>().cloned();
    // Read first and write only what changed: taking the resource mutably
    // every frame would mark it changed every frame, and every binding that
    // reads it would re-run with it.
    let mut updates: Vec<(String, Option<std::time::SystemTime>, TerrainGraph)> = Vec::new();
    let mut stale: Vec<String> = Vec::new();
    {
        let files = world.resource::<GraphFiles>();
        for k in files.map.keys() {
            if !paths.contains(k) {
                stale.push(k.clone());
            }
        }
        for path in &paths {
            if let Some((p, g)) = &open {
                if p == path {
                    if files.map.get(path).map(|(_, old)| old) != Some(g) {
                        updates.push((path.clone(), None, g.clone()));
                    }
                    continue;
                }
            }
            let fs = root.as_ref().map_or_else(|| std::path::PathBuf::from(path), |r| r.resolve_path(path));
            let modified = std::fs::metadata(&fs).and_then(|m| m.modified()).ok();
            if files.map.get(path).is_some_and(|(m, _)| *m == modified && modified.is_some()) {
                continue;
            }
            if let Some(g) = std::fs::read_to_string(&fs).ok().and_then(|t| TerrainGraph::parse(&t).ok()) {
                if files.map.get(path).map(|(_, old)| old) != Some(&g) || files.map.get(path).map(|(m, _)| *m) != Some(modified) {
                    updates.push((path.clone(), modified, g));
                }
            }
        }
    }
    if stale.is_empty() && updates.is_empty() {
        return;
    }
    let mut files = world.resource_mut::<GraphFiles>();
    for k in stale {
        files.map.remove(&k);
    }
    for (path, modified, g) in updates {
        files.map.insert(path, (modified, g));
    }
}
