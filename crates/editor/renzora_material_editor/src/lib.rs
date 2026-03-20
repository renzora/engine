//! Material Editor — visual node graph for authoring PBR materials.
//!
//! Selection-driven: selecting a mesh entity in the viewport loads its material
//! into the graph editor. Edits auto-save to disk.

mod graph_editor;
mod graph_panel;
mod inspector;
pub mod preview;
mod thumbnails;

use bevy::prelude::*;
use renzora_core::CurrentProject;
use renzora_editor::AppEditorExt;
use renzora_material::graph::{MaterialDomain, MaterialGraph};
use renzora_material::material_ref::MaterialRef;
use renzora_material::resolver::{MaterialCache, MaterialResolved};

/// What the material editor is currently doing.
#[derive(Clone, Debug)]
pub enum MaterialEditMode {
    /// No mesh entity selected (or selected entity has no mesh).
    Inactive,
    /// Entity has no MaterialRef yet — showing empty graph, will save on first edit.
    Pending { entity: Entity },
    /// Editing an existing .material file.
    Existing { path: String, entity: Entity },
}

impl Default for MaterialEditMode {
    fn default() -> Self {
        Self::Inactive
    }
}

/// Persistent editor state for the material editor.
#[derive(Resource)]
pub struct MaterialEditorState {
    /// The material graph currently being edited.
    pub graph: MaterialGraph,
    /// Which entity we're editing (follows EditorSelection).
    pub editing_entity: Option<Entity>,
    /// Current edit mode (Inactive / Pending / Existing).
    pub edit_mode: MaterialEditMode,
    /// Currently selected node (for the inspector).
    pub selected_node: Option<u64>,
    /// Last compiled WGSL (for preview / display).
    pub compiled_wgsl: Option<String>,
    /// Compilation errors (shown in UI).
    pub compile_errors: Vec<String>,
    /// True when graph has unsaved changes.
    pub is_dirty: bool,
}

impl Default for MaterialEditorState {
    fn default() -> Self {
        Self {
            graph: MaterialGraph::new("New Material", MaterialDomain::Surface),
            editing_entity: None,
            edit_mode: MaterialEditMode::Inactive,
            selected_node: None,
            compiled_wgsl: None,
            compile_errors: Vec::new(),
            is_dirty: false,
        }
    }
}

pub struct MaterialEditorPlugin;

impl Plugin for MaterialEditorPlugin {
    fn build(&self, app: &mut App) {
        info!("[editor] MaterialEditorPlugin");
        app.init_resource::<MaterialEditorState>();
        app.register_type::<Mesh3d>();
        app.add_plugins(preview::MaterialPreviewPlugin);
        app.add_plugins(thumbnails::NodeThumbnailPlugin);
        app.add_systems(Update, sync_hierarchy_filter_for_materials);
        app.register_panel(graph_panel::MaterialGraphPanel);
        app.register_panel(inspector::MaterialInspectorPanel);
        app.register_panel(preview::MaterialPreviewPanel);
    }
}

/// When the Materials layout is active, only show mesh entities in the hierarchy.
fn sync_hierarchy_filter_for_materials(
    layout_mgr: Res<renzora_ui::layouts::LayoutManager>,
    mut filter: ResMut<renzora_editor::HierarchyFilter>,
) {
    let is_materials = layout_mgr.active_name() == "Materials";
    if is_materials {
        // Use the full reflect short_path — Bevy registers Mesh3d under "bevy_mesh" crate
        let desired = renzora_editor::HierarchyFilter::OnlyWithComponents(vec!["Mesh3d"]);
        if *filter != desired {
            *filter = desired;
        }
    }
}

/// Save the current material graph to disk and invalidate the resolver cache.
/// Called from the Apply button in the graph panel toolbar.
pub fn apply_material(world: &mut World) {
    let (path, graph_json) = {
        let state = world.resource::<MaterialEditorState>();
        let path = match &state.edit_mode {
            MaterialEditMode::Existing { path, .. } => path.clone(),
            _ => return,
        };
        let json = match serde_json::to_string_pretty(&state.graph) {
            Ok(j) => j,
            Err(e) => {
                warn!("[material_editor] Failed to serialize graph: {}", e);
                return;
            }
        };
        (path, json)
    };

    let fs_path = {
        let project = world.get_resource::<CurrentProject>();
        if let Some(p) = project {
            p.resolve_path(&format!("assets/{}", path)).to_string_lossy().to_string()
        } else {
            path.clone()
        }
    };

    if let Some(parent) = std::path::Path::new(&fs_path).parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    if let Err(e) = std::fs::write(&fs_path, &graph_json) {
        warn!("[material_editor] Save failed: {}", e);
        return;
    }
    info!("[material_editor] Saved {}", path);

    // Invalidate resolver cache so the mesh picks up the new material
    world.resource_mut::<MaterialCache>().invalidate(&path);

    // Remove MaterialResolved from entities using this path so resolver re-processes them
    let entities: Vec<Entity> = world
        .query_filtered::<(Entity, &MaterialRef), With<MaterialResolved>>()
        .iter(world)
        .filter(|(_, mr)| mr.0 == path)
        .map(|(e, _)| e)
        .collect();
    for entity in entities {
        world.entity_mut(entity).remove::<MaterialResolved>();
    }

    world.resource_mut::<MaterialEditorState>().is_dirty = false;
}
