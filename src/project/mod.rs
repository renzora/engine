mod config;
pub mod editor_state;
mod project;

use bevy::prelude::*;

pub use config::AppConfig;
pub use editor_state::{EditorStateConfig, EditorStateDirty, LoadedEditorState};
pub use project::{CurrentProject, create_project, open_project};

/// Plugin for project management
pub struct ProjectPlugin;

impl Plugin for ProjectPlugin {
    fn build(&self, app: &mut App) {
        // Load config from disk on startup
        app.insert_resource(AppConfig::load());
    }
}
