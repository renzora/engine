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
    /// Debounced auto-save deadline (elapsed seconds). None = no pending save.
    pub save_timer: Option<f64>,
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
            save_timer: None,
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
        app.add_systems(Update, auto_save_material);
        app.register_panel(graph_panel::MaterialGraphPanel);
        app.register_panel(inspector::MaterialInspectorPanel);
        app.register_panel(preview::MaterialPreviewPanel);
    }
}

/// Debounced auto-save: writes the graph to disk 0.5s after the last edit.
fn auto_save_material(
    time: Res<Time>,
    mut state: ResMut<MaterialEditorState>,
    project: Option<Res<CurrentProject>>,
    mut cache: ResMut<MaterialCache>,
    mut resolved_q: Query<(Entity, &MaterialRef), With<MaterialResolved>>,
    mut commands: Commands,
) {
    let Some(deadline) = state.save_timer else { return; };
    if time.elapsed_secs_f64() < deadline {
        return;
    }
    state.save_timer = None;

    let path = match &state.edit_mode {
        MaterialEditMode::Existing { path, .. } => path.clone(),
        _ => return,
    };

    let Some(project) = project else { return; };
    let fs_path = project.resolve_path(&format!("assets/{}", path));

    if let Some(parent) = fs_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    match serde_json::to_string_pretty(&state.graph) {
        Ok(json) => {
            if let Err(e) = std::fs::write(&fs_path, &json) {
                warn!("[material_editor] Auto-save failed: {}", e);
            } else {
                info!("[material_editor] Auto-saved {}", path);

                // Invalidate resolver cache so entities pick up the new version
                cache.invalidate(&path);
                for (entity, mat_ref) in resolved_q.iter_mut() {
                    if mat_ref.0 == path {
                        commands.entity(entity).remove::<MaterialResolved>();
                    }
                }
            }
        }
        Err(e) => warn!("[material_editor] Failed to serialize graph: {}", e),
    }
}
