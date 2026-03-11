pub mod graph;
pub mod nodes;
pub mod interpreter;

use bevy::prelude::*;

pub use graph::{BlueprintGraph, BlueprintNode, BlueprintConnection, PinType, PinDir, PinValue, PinTemplate, BlueprintNodeDef};
pub use nodes::{ALL_NODES, node_def, categories, nodes_in_category};

pub struct BlueprintPlugin;

impl Plugin for BlueprintPlugin {
    fn build(&self, app: &mut App) {
        info!("[runtime] BlueprintPlugin");
        app.register_type::<BlueprintGraph>()
            .register_type::<BlueprintNode>()
            .register_type::<BlueprintConnection>()
            .add_systems(
                Update,
                interpreter::run_blueprints
                    .run_if(blueprints_should_run)
                    .before(renzora_scripting::ScriptingSet::CommandProcessing),
            );
    }
}

/// Run condition: blueprints execute when scripts would.
fn blueprints_should_run(
    play_mode: Option<Res<renzora_core::PlayModeState>>,
) -> bool {
    match play_mode {
        Some(pm) => pm.is_scripts_running(),
        None => true,
    }
}
