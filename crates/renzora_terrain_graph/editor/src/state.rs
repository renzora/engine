//! The open graph: one `.tgraph` document, edited in memory.
//!
//! [`TerrainGraphEditorState::graph`] is the working copy both panels read and
//! write. [`observe_edits`] notices that it changed (against a shadow, the way
//! the effect editor's observer does) and records the undo step and the dirty
//! flag in one place, whichever panel made the edit. Previews re-evaluate off
//! the same change (see [`crate::preview`]).
//!
//! Saving is explicit (Save, Ctrl+S), as for materials and effects. A Build
//! from the editor uses the working copy, unsaved edits included, when the
//! terrain's generator names the open file: what you see in the previews is
//! what builds.

use std::collections::HashMap;

use bevy::prelude::*;
use renzora::core::CurrentProject;
use renzora_editor_framework::{DocTabKind, EditorContext, EditorSelection};
use renzora_terrain::data::TerrainData;
use renzora_terrain_graph::{TerrainGenerator, TerrainGraph};

#[derive(Resource, Default)]
pub struct TerrainGraphEditorState {
    /// Project-relative path of the open `.tgraph`. `None` until one opens.
    pub path: Option<String>,
    pub graph: TerrainGraph,
    pub selected_node: Option<u64>,
    pub dirty: bool,
    /// Why the file could not be opened, shown in place of the editor.
    pub load_error: Option<String>,
    /// The last preview's problems, by node.
    pub node_errors: HashMap<u64, String>,
    /// The last preview's problems that belong to no node.
    pub graph_errors: Vec<String>,
}

impl TerrainGraphEditorState {
    pub fn is_open(&self) -> bool {
        self.path.is_some() && self.load_error.is_none()
    }

    pub fn file_name(&self) -> String {
        self.path
            .as_deref()
            .and_then(|p| std::path::Path::new(p).file_name().and_then(|n| n.to_str()))
            .unwrap_or_default()
            .to_string()
    }

    /// One line for the error strip.
    pub fn error_summary(&self) -> Option<String> {
        let first = self.graph_errors.first().cloned().or_else(|| {
            let mut ids: Vec<&u64> = self.node_errors.keys().collect();
            ids.sort();
            ids.first().map(|id| {
                let name = self
                    .graph
                    .node(**id)
                    .and_then(|n| renzora_terrain_graph::catalog::node_def(&n.node_type))
                    .map_or("Node", |d| d.display_name);
                format!("{name}: {}", self.node_errors[*id])
            })
        })?;
        let rest = self.graph_errors.len() + self.node_errors.len() - 1;
        Some(if rest > 0 { format!("{first} (+{rest} more)") } else { first })
    }
}

/// What [`observe_edits`] last saw, so it can tell an edit from a switch.
#[derive(Resource, Default)]
pub struct GraphShadow {
    path: Option<String>,
    graph: Option<TerrainGraph>,
}

/// The `.tgraph` the active document tab names, if it names one.
pub fn active_tgraph_path(ctx: Option<&EditorContext>) -> Option<String> {
    match ctx? {
        EditorContext::Asset { path, kind: DocTabKind::TerrainGraph } => Some(path.clone()),
        _ => None,
    }
}

/// Open the `.tgraph` the active document names whenever that changes,
/// saving the outgoing one first if it has unsaved edits.
pub fn load_active_document(world: &mut World) {
    let Some(path) = active_tgraph_path(world.get_resource::<EditorContext>()) else { return };
    if world.resource::<TerrainGraphEditorState>().path.as_deref() == Some(path.as_str()) {
        return;
    }
    if world.resource::<TerrainGraphEditorState>().dirty {
        save_document(world);
    }
    open_document(world, &path);
}

pub fn open_document(world: &mut World, path: &str) {
    let loaded = renzora_terrain_graph::world::load_graph(world, path);
    let mut st = world.resource_mut::<TerrainGraphEditorState>();
    st.path = Some(path.to_string());
    st.selected_node = None;
    st.dirty = false;
    st.node_errors.clear();
    st.graph_errors.clear();
    match loaded {
        Ok(g) => {
            st.graph = g;
            st.load_error = None;
        }
        Err(e) => {
            warn!("[terrain_graph] could not open {path}: {e}");
            st.graph = TerrainGraph::default();
            st.load_error = Some(e);
        }
    }
    let graph = st.graph.clone();
    let mut shadow = world.resource_mut::<GraphShadow>();
    shadow.path = Some(path.to_string());
    shadow.graph = Some(graph);
}

/// Write the open graph to its file.
pub fn save_document(world: &mut World) {
    let (path, text) = {
        let st = world.resource::<TerrainGraphEditorState>();
        let Some(path) = st.path.clone() else { return };
        // Never overwrite a file we failed to read with an empty graph.
        if st.load_error.is_some() {
            return;
        }
        (path, st.graph.to_json())
    };
    let Some(fs_path) = world.get_resource::<CurrentProject>().map(|p| p.resolve_path(&path)) else { return };
    if let Some(parent) = fs_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Err(e) = std::fs::write(&fs_path, &text) {
        renzora::console_log::console_warn("Terrain Graph", format!("{path}: save failed: {e}"));
        return;
    }
    world
        .get_resource_or_insert_with(renzora::core::project_files::SelfWrites::default)
        .record(&fs_path, text.as_bytes());
    info!("[terrain_graph] saved {path}");
    world.resource_mut::<TerrainGraphEditorState>().dirty = false;
}

/// Turn graph changes into undo steps and the dirty mark. A graph is a few
/// dozen nodes, so comparing every frame is cheaper than a missed change.
pub fn observe_edits(world: &mut World) {
    let (path, changed) = {
        let st = world.resource::<TerrainGraphEditorState>();
        let shadow = world.resource::<GraphShadow>();
        if shadow.path != st.path || st.path.is_none() {
            return;
        }
        (st.path.clone().unwrap_or_default(), shadow.graph.as_ref() != Some(&st.graph))
    };
    if !changed {
        return;
    }
    let before = world.resource::<GraphShadow>().graph.clone().unwrap_or_default();
    let after = world.resource::<TerrainGraphEditorState>().graph.clone();
    let ctx = renzora_undo::active_context(world);
    renzora_undo::record(
        world,
        ctx,
        Box::new(renzora_undo::SnapshotCmd {
            label: "Terrain Graph".to_string(),
            before: (path.clone(), before),
            after: (path.clone(), after.clone()),
            restore: restore_snapshot,
            // One step per gesture: a scrub is many frames of edits.
            merge_key: Some(format!("terrain-graph:{path}")),
        }),
    );
    world.resource_mut::<TerrainGraphEditorState>().dirty = true;
    world.resource_mut::<GraphShadow>().graph = Some(after);
}

/// The `restore` of a graph undo step. A step for a file that is no longer
/// open opens it first rather than writing one graph into another.
fn restore_snapshot(world: &mut World, snapshot: &(String, TerrainGraph)) {
    let (path, graph) = snapshot;
    let open = world.resource::<TerrainGraphEditorState>().path.clone();
    if open.as_deref() != Some(path.as_str()) {
        if world.resource::<TerrainGraphEditorState>().dirty {
            save_document(world);
        }
        open_document(world, path);
        if world.resource::<TerrainGraphEditorState>().load_error.is_some() {
            return;
        }
    }
    {
        let mut st = world.resource_mut::<TerrainGraphEditorState>();
        st.graph = graph.clone();
        st.dirty = true;
        if st.selected_node.is_some_and(|id| st.graph.node(id).is_none()) {
            st.selected_node = None;
        }
    }
    world.resource_mut::<GraphShadow>().graph = Some(graph.clone());
}

/// The terrain the open graph previews against and builds into: one whose
/// generator already names this file, else the selected terrain, else the
/// only one. Previews take its extent, seed and overrides; Current Terrain
/// reads its ground.
pub fn target_terrain(world: &mut World) -> Option<Entity> {
    let path = world.resource::<TerrainGraphEditorState>().path.clone();
    let mut q = world.query::<(Entity, &TerrainData, Option<&TerrainGenerator>)>();
    let all: Vec<(Entity, Option<String>)> = q.iter(world).map(|(e, _, g)| (e, g.map(|g| g.graph.clone()))).collect();
    if let Some(path) = &path {
        if let Some((e, _)) = all.iter().find(|(_, g)| g.as_deref() == Some(path.as_str())) {
            return Some(*e);
        }
    }
    let selected = world.get_resource::<EditorSelection>().and_then(|s| s.get());
    if let Some(sel) = selected.filter(|s| all.iter().any(|(e, _)| e == s)) {
        return Some(sel);
    }
    match all.as_slice() {
        [(e, _)] => Some(*e),
        _ => None,
    }
}
