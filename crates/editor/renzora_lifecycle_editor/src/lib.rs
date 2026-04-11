//! Lifecycle Editor — visual graph for project-level game flow.

mod graph_panel;
mod monitor_panel;
mod properties_panel;
mod settings_panel;

use bevy::prelude::*;
use renzora::editor::AppEditorExt;

/// Tracks selection state for the lifecycle graph editor.
#[derive(Resource, Default)]
pub struct LifecycleEditorState {
    /// Currently selected node ID in the graph.
    pub selected_node: Option<u64>,
}

#[derive(Default)]
pub struct LifecycleEditorPlugin;

impl Plugin for LifecycleEditorPlugin {
    fn build(&self, app: &mut App) {
        info!("[editor] LifecycleEditorPlugin");
        app.init_resource::<LifecycleEditorState>();
        app.register_panel(graph_panel::LifecycleGraphPanel);
        app.register_panel(properties_panel::LifecyclePropertiesPanel);
        app.register_panel(settings_panel::LifecycleSettingsPanel);
        app.register_panel(monitor_panel::LifecycleMonitorPanel);
    }
}

renzora::add!(LifecycleEditorPlugin, Editor);
