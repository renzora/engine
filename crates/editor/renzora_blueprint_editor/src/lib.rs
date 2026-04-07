//! Blueprint Editor — visual node graph for entity logic.

mod graph_editor;
mod graph_panel;
mod properties_panel;

use bevy::prelude::*;
use renzora::editor::AppEditorExt;

/// Tracks which entity's blueprint is currently open in the editor.
#[derive(Resource, Default)]
pub struct BlueprintEditorState {
    /// The entity whose blueprint is being edited (None = no entity selected).
    pub editing_entity: Option<Entity>,
    /// Currently selected node (for a future inspector panel).
    pub selected_node: Option<u64>,
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

renzora::add!(BlueprintEditorPlugin);
