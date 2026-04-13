//! Renzora Lifecycle — project-level node graph for boot sequence, scene flow, and networking.

pub mod graph;
pub mod interpreter;
pub mod io;
pub mod nodes;
pub mod state;

pub use graph::LifecycleGraph;
pub use state::LifecycleRuntimeState;

use bevy::prelude::*;

pub struct LifecyclePlugin;

impl Plugin for LifecyclePlugin {
    fn build(&self, app: &mut App) {
        info!("[runtime] LifecyclePlugin");

        app.init_resource::<LifecycleGraph>();
        app.init_resource::<LifecycleRuntimeState>();

        app.add_systems(Startup, load_lifecycle_graph);
        app.add_systems(
            Update,
            (
                reload_lifecycle_on_project_change,
                interpreter::reset_lifecycle_on_play_start,
                interpreter::run_lifecycle
                    .run_if(lifecycle_should_run),
                interpreter::detect_scene_loaded
                    .run_if(lifecycle_should_run),
            )
                .chain(),
        );
    }
}

/// Load lifecycle.json from the project root at startup.
fn load_lifecycle_graph(world: &mut World) {
    let graph = world
        .get_resource::<renzora_core::CurrentProject>()
        .and_then(|project| {
            let path = project.path.join("lifecycle.json");
            info!("[lifecycle] Loading from {}", path.display());
            io::load_lifecycle(&path)
        })
        .unwrap_or_default();

    if graph.has_game_start() {
        world.insert_resource(renzora_core::LifecycleHandlesBoot);
    }

    world.insert_resource(graph);
}

/// Reload lifecycle.json when the project changes (e.g. opening a new project in the editor).
fn reload_lifecycle_on_project_change(
    project: Option<Res<renzora_core::CurrentProject>>,
    mut graph: ResMut<LifecycleGraph>,
    mut runtime: ResMut<LifecycleRuntimeState>,
    mut cmds: Commands,
) {
    let Some(project) = project else { return };
    if !project.is_changed() {
        return;
    }

    let path = project.path.join("lifecycle.json");
    let new_graph = io::load_lifecycle(&path).unwrap_or_default();

    info!(
        "[lifecycle] Project changed — reloaded ({} nodes)",
        new_graph.nodes.len()
    );

    if new_graph.has_game_start() {
        cmds.insert_resource(renzora_core::LifecycleHandlesBoot);
    } else {
        cmds.remove_resource::<renzora_core::LifecycleHandlesBoot>();
    }

    *graph = new_graph;
    *runtime = LifecycleRuntimeState::default();
}

/// Run condition: lifecycle executes when scripts would.
fn lifecycle_should_run(play_mode: Option<Res<renzora_core::PlayModeState>>) -> bool {
    match play_mode {
        Some(pm) => pm.is_scripts_running(),
        None => true,
    }
}
