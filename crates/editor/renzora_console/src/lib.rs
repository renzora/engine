//! Console panel crate for the Renzora editor.

pub mod render;
pub mod state;

pub use state::*;

use std::sync::{Arc, Mutex, RwLock};

use bevy::prelude::*;
use bevy_egui::egui;

use renzora_editor::{AppEditorExt, EditorPanel, PanelLocation};
use renzora_theme::ThemeManager;

use crate::render::render_console_content;

// ---------------------------------------------------------------------------
// Shared state bridge (same Arc/Mutex pattern as renzora_mixer)
// ---------------------------------------------------------------------------

/// Bridge resource: panel writes here, system reads back.
#[derive(Resource, Clone)]
struct ConsoleBridge {
    pending: Arc<Mutex<Option<ConsoleState>>>,
}

impl Default for ConsoleBridge {
    fn default() -> Self {
        Self {
            pending: Arc::new(Mutex::new(None)),
        }
    }
}

// ---------------------------------------------------------------------------
// EditorPanel implementation
// ---------------------------------------------------------------------------

pub struct ConsolePanel {
    bridge: Arc<Mutex<Option<ConsoleState>>>,
    local: RwLock<ConsoleState>,
}

impl ConsolePanel {
    fn new(bridge: Arc<Mutex<Option<ConsoleState>>>) -> Self {
        Self {
            bridge,
            local: RwLock::new(ConsoleState::default()),
        }
    }
}

impl EditorPanel for ConsolePanel {
    fn id(&self) -> &str {
        "console"
    }

    fn title(&self) -> &str {
        "Console"
    }

    fn icon(&self) -> Option<&str> {
        Some(egui_phosphor::regular::TERMINAL)
    }

    fn ui(&self, ui: &mut egui::Ui, world: &World) {
        // Read current ConsoleState from world into local copy.
        if let Some(console) = world.get_resource::<ConsoleState>() {
            if let Ok(mut local) = self.local.write() {
                local.entries = console.entries.clone();
                local.shared_buffer = console.shared_buffer.clone();
                local.show_info = console.show_info;
                local.show_success = console.show_success;
                local.show_warnings = console.show_warnings;
                local.show_errors = console.show_errors;
                local.auto_scroll = console.auto_scroll;
                local.search_filter = console.search_filter.clone();
                local.category_filter = console.category_filter.clone();
                local.input_buffer = console.input_buffer.clone();
                local.command_history = console.command_history.clone();
                local.history_index = console.history_index;
                local.saved_input = console.saved_input.clone();
                local.focus_input = console.focus_input;
            }
        }

        // Read theme.
        let theme = if let Some(tm) = world.get_resource::<ThemeManager>() {
            tm.active_theme.clone()
        } else {
            return;
        };

        // Render mutably into local snapshot.
        if let Ok(mut local) = self.local.write() {
            render_console_content(ui, &mut local, &theme);
        }

        // Push modified state so the sync system can apply it.
        if let Ok(mut pending) = self.bridge.lock() {
            if let Ok(local) = self.local.read() {
                let mut state = ConsoleState::default();
                state.entries = local.entries.clone();
                state.shared_buffer = local.shared_buffer.clone();
                state.show_info = local.show_info;
                state.show_success = local.show_success;
                state.show_warnings = local.show_warnings;
                state.show_errors = local.show_errors;
                state.auto_scroll = local.auto_scroll;
                state.search_filter = local.search_filter.clone();
                state.category_filter = local.category_filter.clone();
                state.input_buffer = local.input_buffer.clone();
                state.command_history = local.command_history.clone();
                state.history_index = local.history_index;
                state.saved_input = local.saved_input.clone();
                state.focus_input = local.focus_input;
                *pending = Some(state);
            }
        }
    }

    fn closable(&self) -> bool {
        true
    }

    fn min_size(&self) -> [f32; 2] {
        [200.0, 100.0]
    }

    fn default_location(&self) -> PanelLocation {
        PanelLocation::Bottom
    }
}

// ---------------------------------------------------------------------------
// Systems
// ---------------------------------------------------------------------------

/// Drain the global shared buffer into ConsoleState each frame.
fn drain_log_buffer(mut console: ResMut<ConsoleState>, time: Res<Time>) {
    console.drain_shared_buffer(time.elapsed_secs_f64());
}

/// Drain script log messages into the console.
fn drain_script_logs(
    mut console: ResMut<ConsoleState>,
    mut log_buffer: ResMut<renzora_scripting::systems::ScriptLogBuffer>,
) {
    for entry in log_buffer.entries.drain(..) {
        let level = match entry.level.as_str() {
            "warn" => LogLevel::Warning,
            "error" => LogLevel::Error,
            _ => LogLevel::Info,
        };
        console.log(level, "Script", entry.message);
    }
}

/// Apply pending console mutations from the panel bridge back to the real resource.
fn sync_console_bridge(bridge: Res<ConsoleBridge>, mut console: ResMut<ConsoleState>) {
    if let Ok(mut pending) = bridge.pending.lock() {
        if let Some(snap) = pending.take() {
            console.entries = snap.entries;
            console.show_info = snap.show_info;
            console.show_success = snap.show_success;
            console.show_warnings = snap.show_warnings;
            console.show_errors = snap.show_errors;
            console.auto_scroll = snap.auto_scroll;
            console.search_filter = snap.search_filter;
            console.category_filter = snap.category_filter;
            console.input_buffer = snap.input_buffer;
            console.command_history = snap.command_history;
            console.history_index = snap.history_index;
            console.saved_input = snap.saved_input;
            console.focus_input = snap.focus_input;
        }
    }
}

// ---------------------------------------------------------------------------
// Plugin
// ---------------------------------------------------------------------------

pub struct ConsolePlugin;

impl Plugin for ConsolePlugin {
    fn build(&self, app: &mut App) {
        info!("[editor] ConsolePlugin");
        app.insert_resource(ConsoleState::default());

        let bridge = ConsoleBridge::default();
        let arc = bridge.pending.clone();

        app.insert_resource(bridge);
        use renzora_editor::SplashState;
        app.add_systems(Update, (drain_log_buffer, drain_script_logs, sync_console_bridge).run_if(in_state(SplashState::Editor)));

        app.register_panel(ConsolePanel::new(arc));
    }
}
