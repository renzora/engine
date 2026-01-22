mod api;
mod component;
mod registry;
mod rhai_engine;
mod runtime;
mod builtin_scripts;

pub use api::*;
pub use component::*;
pub use registry::*;
pub use rhai_engine::*;
pub use runtime::*;

use bevy::prelude::*;
use crate::core::AppState;
use crate::project::CurrentProject;

pub struct ScriptingPlugin;

impl Plugin for ScriptingPlugin {
    fn build(&self, app: &mut App) {
        // Initialize registry with built-in scripts
        let mut registry = ScriptRegistry::new();
        builtin_scripts::register_builtin_scripts(&mut registry);

        app.insert_resource(registry)
            .insert_resource(RhaiScriptEngine::new())
            .init_resource::<ScriptInput>()
            .add_systems(
                Update,
                (
                    update_rhai_scripts_folder,
                    update_script_input,
                    run_scripts,
                    run_rhai_scripts,
                )
                    .chain()
                    .run_if(in_state(AppState::Editor)),
            );
    }
}

/// System to update the Rhai scripts folder when project changes
fn update_rhai_scripts_folder(
    current_project: Option<Res<CurrentProject>>,
    mut rhai_engine: ResMut<RhaiScriptEngine>,
    mut last_project_path: Local<Option<std::path::PathBuf>>,
) {
    let current_path = current_project.as_ref().map(|p| p.path.clone());

    if *last_project_path != current_path {
        *last_project_path = current_path.clone();

        if let Some(project_path) = current_path {
            let scripts_folder = project_path.join("scripts");
            // Create scripts folder if it doesn't exist
            let _ = std::fs::create_dir_all(&scripts_folder);
            rhai_engine.set_scripts_folder(scripts_folder);
        }
    }
}
