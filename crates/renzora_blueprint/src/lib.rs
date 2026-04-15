pub mod graph;
pub mod nodes;
pub mod interpreter;
pub mod compiler;

use bevy::prelude::*;

pub use graph::{BlueprintGraph, BlueprintNode, BlueprintConnection, PinType, PinDir, PinValue, PinTemplate, BlueprintNodeDef};
pub use nodes::{ALL_NODES, node_def, categories, nodes_in_category};

pub struct BlueprintPlugin;

impl Plugin for BlueprintPlugin {
    fn build(&self, app: &mut App) {
        info!("[runtime] BlueprintPlugin");
        // Ensure shared resources exist (they may also be init'd by scripting).
        app.init_resource::<renzora::TransformWriteQueue>()
            .init_resource::<renzora::CharacterCommandQueue>()
            .init_resource::<renzora::ScriptInput>()
            .init_resource::<interpreter::BlueprintSceneLoadTracker>()
            .register_type::<BlueprintGraph>()
            .register_type::<BlueprintNode>()
            .register_type::<BlueprintConnection>()
            .add_systems(
                Update,
                (
                    reset_blueprint_runtime_on_play_start,
                    interpreter::run_blueprints
                        .run_if(blueprints_should_run),
                )
                    .chain(),
            );
    }
}

/// Reset all blueprint runtime state when play mode starts, so On Ready fires again.
fn reset_blueprint_runtime_on_play_start(
    play_mode: Option<Res<renzora::PlayModeState>>,
    mut states: Query<&mut interpreter::BlueprintRuntimeState>,
    mut was_running: Local<bool>,
) {
    let running = play_mode.as_ref().map(|pm| pm.is_scripts_running()).unwrap_or(false);
    if running && !*was_running {
        for mut state in &mut states {
            *state = interpreter::BlueprintRuntimeState::default();
        }
    }
    *was_running = running;
}

/// Run condition: blueprints execute when scripts would.
fn blueprints_should_run(
    play_mode: Option<Res<renzora::PlayModeState>>,
) -> bool {
    match play_mode {
        Some(pm) => pm.is_scripts_running(),
        None => true,
    }
}
