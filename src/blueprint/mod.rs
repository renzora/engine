//! Blueprint Visual Scripting System
//!
//! A node-based visual scripting editor that compiles to Rhai scripts.
//! Blueprints can be attached to entities and executed at runtime.

pub mod canvas;
mod codegen;
mod component;
mod graph;
pub mod interactions;
pub mod nodes;
pub mod serialization;

pub use canvas::*;
pub use codegen::*;
pub use component::*;
pub use graph::*;
pub use interactions::*;
pub use serialization::*;

use bevy::prelude::*;
use crate::core::AppState;
use crate::project::CurrentProject;

/// Plugin for the blueprint visual scripting system
pub struct BlueprintPlugin;

impl Plugin for BlueprintPlugin {
    fn build(&self, app: &mut App) {
        // Initialize node registry
        let mut registry = nodes::NodeRegistry::new();
        nodes::register_all_nodes(&mut registry);

        app.insert_resource(registry)
            .init_resource::<BlueprintEditorState>()
            .init_resource::<BlueprintCanvasState>()
            .add_systems(
                Update,
                (
                    update_blueprints_folder,
                    run_blueprint_scripts,
                )
                    .chain()
                    .run_if(in_state(AppState::Editor)),
            );
    }
}

/// Resource tracking the blueprints folder for the current project
#[derive(Resource, Default)]
pub struct BlueprintsFolder(pub Option<std::path::PathBuf>);

/// System to update the blueprints folder when project changes
fn update_blueprints_folder(
    current_project: Option<Res<CurrentProject>>,
    mut blueprints_folder: Local<BlueprintsFolder>,
    mut last_project_path: Local<Option<std::path::PathBuf>>,
) {
    let current_path = current_project.as_ref().map(|p| p.path.clone());

    if *last_project_path != current_path {
        *last_project_path = current_path.clone();

        if let Some(project_path) = current_path {
            let folder = project_path.join("blueprints");
            // Create blueprints folder if it doesn't exist
            let _ = std::fs::create_dir_all(&folder);
            blueprints_folder.0 = Some(folder);
        }
    }
}

/// System to execute blueprints on entities with BlueprintComponent
fn run_blueprint_scripts(
    world: &mut World,
) {
    // Blueprint execution happens through the Rhai engine
    // We compile the blueprint to Rhai code and execute it
    // This is handled in the play mode system
}
