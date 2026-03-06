//! Gamepad debug panel — visualizes controller input (sticks, triggers, buttons).

mod render;
mod state;

use std::sync::RwLock;

use bevy::prelude::*;
use bevy_egui::egui;
use egui_phosphor::regular;

use renzora_editor::{AppEditorExt, EditorPanel, PanelLocation};
use renzora_theme::ThemeManager;

use state::{GamepadDebugState, update_gamepad_debug_state};

// ---------------------------------------------------------------------------
// Panel
// ---------------------------------------------------------------------------

struct GamepadPanel {
    _state: RwLock<()>,
}

impl Default for GamepadPanel {
    fn default() -> Self {
        Self {
            _state: RwLock::new(()),
        }
    }
}

impl EditorPanel for GamepadPanel {
    fn id(&self) -> &str {
        "gamepad"
    }

    fn title(&self) -> &str {
        "Gamepad"
    }

    fn icon(&self) -> Option<&str> {
        Some(regular::GAME_CONTROLLER)
    }

    fn ui(&self, ui: &mut egui::Ui, world: &World) {
        let theme = match world.get_resource::<ThemeManager>() {
            Some(tm) => tm.active_theme.clone(),
            None => return,
        };

        let Some(gamepad_state) = world.get_resource::<GamepadDebugState>() else {
            return;
        };

        render::render_gamepad_content(ui, gamepad_state, &theme);
    }

    fn closable(&self) -> bool {
        true
    }

    fn default_location(&self) -> PanelLocation {
        PanelLocation::Right
    }
}

// ---------------------------------------------------------------------------
// Plugin
// ---------------------------------------------------------------------------

/// Plugin that registers the gamepad debug panel and its update system.
pub struct GamepadPlugin;

impl Plugin for GamepadPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<GamepadDebugState>();
        use renzora_editor::SplashState;
        app.add_systems(Update, update_gamepad_debug_state.run_if(in_state(SplashState::Editor)));
        app.register_panel(GamepadPanel::default());
    }
}
