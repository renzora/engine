//! Material Editor — visual node graph for authoring PBR materials.

mod graph_editor;
mod graph_panel;
mod inspector;
pub mod preview;
mod thumbnails;

use bevy::prelude::*;
use renzora_editor::AppEditorExt;
use renzora_material::graph::{MaterialDomain, MaterialGraph};

/// Persistent editor state for the material editor.
#[derive(Resource)]
pub struct MaterialEditorState {
    /// The material graph currently being edited.
    pub graph: MaterialGraph,
    /// File path if loaded from / saved to disk.
    pub file_path: Option<String>,
    /// Dirty flag — graph has been modified since last save.
    pub is_modified: bool,
    /// Currently selected node (for the inspector).
    pub selected_node: Option<u64>,
    /// Last compiled WGSL (for preview / display).
    pub compiled_wgsl: Option<String>,
    /// Compilation errors (shown in UI).
    pub compile_errors: Vec<String>,
}

impl Default for MaterialEditorState {
    fn default() -> Self {
        Self {
            graph: MaterialGraph::new("New Material", MaterialDomain::Surface),
            file_path: None,
            is_modified: false,
            selected_node: None,
            compiled_wgsl: None,
            compile_errors: Vec::new(),
        }
    }
}

pub struct MaterialEditorPlugin;

impl Plugin for MaterialEditorPlugin {
    fn build(&self, app: &mut App) {
        info!("[editor] MaterialEditorPlugin");
        app.init_resource::<MaterialEditorState>();
        app.add_plugins(preview::MaterialPreviewPlugin);
        app.add_plugins(thumbnails::NodeThumbnailPlugin);
        app.register_panel(graph_panel::MaterialGraphPanel);
        app.register_panel(inspector::MaterialInspectorPanel);
        app.register_panel(preview::MaterialPreviewPanel);
    }
}
