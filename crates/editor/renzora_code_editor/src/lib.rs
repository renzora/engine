pub mod render;
pub mod state;

pub use state::*;

use std::sync::{Arc, Mutex, RwLock};

use bevy::prelude::*;
use bevy_egui::egui;

use renzora_core::CurrentProject;
use renzora_editor::{AppEditorExt, AssetDragPayload, EditorCommands, EditorPanel, PanelLocation};
use renzora_theme::ThemeManager;

use crate::render::render_code_editor_content;

// ---------------------------------------------------------------------------
// Bridge
// ---------------------------------------------------------------------------

#[derive(Resource, Clone)]
struct CodeEditorBridge {
    pending: Arc<Mutex<Option<CodeEditorState>>>,
}

impl Default for CodeEditorBridge {
    fn default() -> Self {
        Self {
            pending: Arc::new(Mutex::new(None)),
        }
    }
}

// ---------------------------------------------------------------------------
// Panel
// ---------------------------------------------------------------------------

pub struct CodeEditorPanel {
    bridge: Arc<Mutex<Option<CodeEditorState>>>,
    local: RwLock<CodeEditorState>,
}

impl CodeEditorPanel {
    fn new(bridge: Arc<Mutex<Option<CodeEditorState>>>) -> Self {
        Self {
            bridge,
            local: RwLock::new(CodeEditorState::default()),
        }
    }
}

impl EditorPanel for CodeEditorPanel {
    fn id(&self) -> &str {
        "code_editor"
    }

    fn title(&self) -> &str {
        "Code Editor"
    }

    fn icon(&self) -> Option<&str> {
        Some(egui_phosphor::regular::CODE)
    }

    fn ui(&self, ui: &mut egui::Ui, world: &World) {
        // Sync from world
        if let Some(editor_state) = world.get_resource::<CodeEditorState>() {
            if let Ok(mut local) = self.local.write() {
                local.open_files = editor_state.open_files.clone();
                local.active_tab = editor_state.active_tab;
                local.font_size = editor_state.font_size;
            }
        }

        let theme = if let Some(tm) = world.get_resource::<ThemeManager>() {
            tm.active_theme.clone()
        } else {
            return;
        };

        // Get scripts directory from project
        let scripts_dir = world
            .get_resource::<CurrentProject>()
            .map(|p| p.path.join("scripts"));

        // Render
        if let Ok(mut local) = self.local.write() {
            render_code_editor_content(ui, &mut local, &theme, scripts_dir);
        }

        // Accept script file drops
        if let Some(payload) = world.get_resource::<AssetDragPayload>() {
            if payload.is_detached {
                let ext = payload.extension();
                let is_script = matches!(ext.as_str(),
                    "lua" | "rhai" | "rs" | "py" | "js" | "ts" | "wgsl" | "glsl" | "json" | "toml" | "yaml" | "yml" | "txt" | "md"
                );
                if is_script {
                    let panel_rect = ui.min_rect();
                    let hovering = ui.ctx().pointer_hover_pos()
                        .map_or(false, |p| panel_rect.contains(p));
                    if hovering && !ui.ctx().input(|i| i.pointer.any_down()) {
                        let path = payload.path.clone();
                        if let Ok(mut local) = self.local.write() {
                            local.open_file(path);
                        }
                        // Remove the drag payload so the editor system doesn't cancel it
                        if let Some(cmds) = world.get_resource::<EditorCommands>() {
                            cmds.push(|world: &mut bevy::prelude::World| {
                                world.remove_resource::<AssetDragPayload>();
                            });
                        }
                    }
                }
            }
        }

        // Push back
        if let Ok(mut pending) = self.bridge.lock() {
            if let Ok(local) = self.local.read() {
                *pending = Some(CodeEditorState {
                    open_files: local.open_files.clone(),
                    active_tab: local.active_tab,
                    font_size: local.font_size,
                });
            }
        }
    }

    fn closable(&self) -> bool {
        true
    }

    fn min_size(&self) -> [f32; 2] {
        [300.0, 200.0]
    }

    fn default_location(&self) -> PanelLocation {
        PanelLocation::Center
    }
}

// ---------------------------------------------------------------------------
// Systems
// ---------------------------------------------------------------------------

fn sync_code_editor_bridge(bridge: Res<CodeEditorBridge>, mut state: ResMut<CodeEditorState>) {
    if let Ok(mut pending) = bridge.pending.lock() {
        if let Some(snap) = pending.take() {
            state.open_files = snap.open_files;
            state.active_tab = snap.active_tab;
            state.font_size = snap.font_size;
        }
    }
}

// ---------------------------------------------------------------------------
// Plugin
// ---------------------------------------------------------------------------

pub struct CodeEditorPlugin;

impl Plugin for CodeEditorPlugin {
    fn build(&self, app: &mut App) {
        info!("[editor] CodeEditorPlugin");
        app.insert_resource(CodeEditorState::default());

        let bridge = CodeEditorBridge::default();
        let arc = bridge.pending.clone();

        app.insert_resource(bridge);
        use renzora_editor::SplashState;
        app.add_systems(
            Update,
            sync_code_editor_bridge.run_if(in_state(SplashState::Editor)),
        );

        app.register_panel(CodeEditorPanel::new(arc));
    }
}
