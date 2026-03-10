pub mod render;

use std::sync::RwLock;

use bevy::prelude::*;
use bevy_egui::egui;

use renzora_editor::{AppEditorExt, EditorPanel, PanelLocation};
use renzora_scripting::{ScriptEngine, ScriptVariableDefinition};
use renzora_theme::ThemeManager;

use crate::render::render_script_variables_content;

// ---------------------------------------------------------------------------
// Snapshot of variable data (read-only panel, no bridge needed)
// ---------------------------------------------------------------------------

/// Cached variable definitions for the active script tab.
#[derive(Clone, Default)]
struct VariablesSnapshot {
    script_name: String,
    props: Vec<ScriptVariableDefinition>,
}

// ---------------------------------------------------------------------------
// Panel
// ---------------------------------------------------------------------------

pub struct ScriptVariablesPanel {
    local: RwLock<VariablesSnapshot>,
}

impl ScriptVariablesPanel {
    fn new() -> Self {
        Self {
            local: RwLock::new(VariablesSnapshot::default()),
        }
    }
}

impl EditorPanel for ScriptVariablesPanel {
    fn id(&self) -> &str {
        "script_variables"
    }

    fn title(&self) -> &str {
        "Script Variables"
    }

    fn icon(&self) -> Option<&str> {
        Some(egui_phosphor::regular::LIST_DASHES)
    }

    fn ui(&self, ui: &mut egui::Ui, world: &World) {
        let theme = if let Some(tm) = world.get_resource::<ThemeManager>() {
            tm.active_theme.clone()
        } else {
            return;
        };

        // Get the active file from code editor state
        let editor_state = world.get_resource::<renzora_code_editor::CodeEditorState>();
        let engine = world.get_resource::<ScriptEngine>();

        let snapshot = if let (Some(state), Some(engine)) = (editor_state, engine) {
            if let Some(idx) = state.active_tab {
                if let Some(file) = state.open_files.get(idx) {
                    let props = engine.get_script_props(&file.path);
                    VariablesSnapshot {
                        script_name: file.name.clone(),
                        props,
                    }
                } else {
                    VariablesSnapshot::default()
                }
            } else {
                VariablesSnapshot::default()
            }
        } else {
            VariablesSnapshot::default()
        };

        if let Ok(mut local) = self.local.write() {
            *local = snapshot;
        }

        if let Ok(local) = self.local.read() {
            render_script_variables_content(ui, &local.script_name, &local.props, &theme);
        }
    }

    fn closable(&self) -> bool {
        true
    }

    fn min_size(&self) -> [f32; 2] {
        [180.0, 100.0]
    }

    fn default_location(&self) -> PanelLocation {
        PanelLocation::Right
    }
}

// ---------------------------------------------------------------------------
// Plugin
// ---------------------------------------------------------------------------

pub struct ScriptVariablesPlugin;

impl Plugin for ScriptVariablesPlugin {
    fn build(&self, app: &mut App) {
        info!("[editor] ScriptVariablesPlugin");
        app.register_panel(ScriptVariablesPanel::new());
    }
}
