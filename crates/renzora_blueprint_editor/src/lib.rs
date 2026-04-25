//! Blueprint Editor — visual node graph for entity logic.

mod graph_editor;
mod graph_panel;
mod properties_panel;

use bevy::prelude::*;
use renzora_blueprint::BlueprintGraph;
use renzora_editor_framework::AppEditorExt;

/// Tracks what the blueprint editor is currently focused on. Two modes:
///
/// - **Scene mode** (the default): `editing_entity` follows `EditorSelection`,
///   and the graph being edited is the `BlueprintGraph` component on that
///   entity. `editing_file_path` and `file_graph` are `None`.
/// - **Asset mode**: a `.blueprint` file is open in a document tab. The
///   graph lives in `file_graph` and saves write back to `editing_file_path`.
///   `editing_entity` is `None`.
#[derive(Resource, Default)]
pub struct BlueprintEditorState {
    /// Scene mode: the entity whose `BlueprintGraph` component is being edited.
    pub editing_entity: Option<Entity>,
    /// Asset mode: project-relative path to the `.blueprint` file in the
    /// active doc tab.
    pub editing_file_path: Option<String>,
    /// Asset mode: the graph loaded from `editing_file_path`. Edits mutate
    /// this and trigger a save.
    pub file_graph: Option<BlueprintGraph>,
    /// Currently selected node (for the properties panel).
    pub selected_node: Option<u64>,
    /// Asset mode: whether `file_graph` has unsaved changes.
    pub is_dirty: bool,
}

#[derive(Default)]
pub struct BlueprintEditorPlugin;

impl Plugin for BlueprintEditorPlugin {
    fn build(&self, app: &mut App) {
        info!("[editor] BlueprintEditorPlugin");
        app.init_resource::<BlueprintEditorState>();
        app.register_panel(graph_panel::BlueprintGraphPanel::default());
        app.register_panel(properties_panel::BlueprintPropertiesPanel::default());
    }
}

renzora::add!(BlueprintEditorPlugin, Editor);
