mod api;
mod component;
mod registry;
mod rhai_engine;
mod rhai_context;
mod rhai_commands;
mod rhai_api;
mod runtime;
mod builtin_scripts;
pub mod resources;
pub mod systems;

pub use api::*;
pub use component::*;
pub use registry::*;
pub use rhai_engine::*;
pub use rhai_context::*;
pub use rhai_commands::*;
pub use runtime::*;
pub use resources::*;
pub use systems::*;

use bevy::prelude::*;
use crate::core::{AppState, PlayModeState, PlayState};
use crate::core::resources::console::{console_log, LogLevel};
use crate::project::CurrentProject;

/// System sets for ordering scripting systems
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum ScriptingSet {
    /// Pre-script systems (input update, timer update, folder detection)
    PreScript,
    /// Script execution (run_scripts, run_rhai_scripts)
    ScriptExecution,
    /// Post-script command processing (physics, audio, rendering, camera)
    CommandProcessing,
    /// Debug draw systems
    DebugDraw,
    /// Cleanup systems (run when play mode stops)
    Cleanup,
}

pub struct ScriptingPlugin;

impl Plugin for ScriptingPlugin {
    fn build(&self, app: &mut App) {
        // Initialize registry with built-in scripts
        let mut registry = ScriptRegistry::new();
        builtin_scripts::register_builtin_scripts(&mut registry);

        app.insert_resource(registry)
            .insert_resource(RhaiScriptEngine::new())
            .init_resource::<ScriptInput>()
            // Initialize scripting resources
            .init_resource::<PhysicsCommandQueue>()
            .init_resource::<RaycastResults>()
            .init_resource::<ScriptTimers>()
            .init_resource::<DebugDrawQueue>()
            .init_resource::<AudioCommandQueue>()
            .init_resource::<AudioState>()
            .init_resource::<RenderingCommandQueue>()
            .init_resource::<CameraCommandQueue>()
            .init_resource::<ScriptCameraState>()
            .init_resource::<ScriptCollisionEvents>()
            .init_resource::<SceneCommandQueue>()
            .init_resource::<AnimationCommandQueue>()
            .init_resource::<ActiveTweens>()
            .init_resource::<SpriteAnimationCommandQueue>()
            .init_resource::<HealthCommandQueue>()
            // Configure system set ordering
            .configure_sets(
                Update,
                (
                    ScriptingSet::PreScript,
                    ScriptingSet::ScriptExecution,
                    ScriptingSet::CommandProcessing,
                    ScriptingSet::DebugDraw,
                    ScriptingSet::Cleanup,
                )
                    .chain()
                    .run_if(in_state(AppState::Editor)),
            )
            // Pre-script systems
            .add_systems(
                Update,
                (
                    update_rhai_scripts_folder,
                    update_script_input,
                    reset_scripts_on_play_start,
                    update_script_timers,
                )
                    .in_set(ScriptingSet::PreScript),
            )
            // Collision event collection (separate to avoid tuple size limits)
            .add_systems(
                Update,
                (collect_collision_events,).in_set(ScriptingSet::PreScript),
            )
            // Script execution systems - run_scripts before run_rhai_scripts
            .add_systems(
                Update,
                (run_scripts,).in_set(ScriptingSet::ScriptExecution),
            )
            .add_systems(
                Update,
                (run_rhai_scripts,)
                    .in_set(ScriptingSet::ScriptExecution)
                    .after(run_scripts),
            )
            // Post-script command processing systems
            .add_systems(
                Update,
                (
                    process_physics_commands,
                    process_audio_commands,
                    update_audio_fades,
                    process_rendering_commands,
                    process_camera_commands,
                    apply_camera_effects,
                    process_prefab_spawns,
                    process_animation_commands,
                    update_animation_playback,
                    update_tweens,
                    process_sprite_animation_commands,
                    update_sprite_animations,
                    process_health_commands,
                )
                    .in_set(ScriptingSet::CommandProcessing),
            )
            // Debug draw systems
            .add_systems(
                Update,
                (tick_debug_draws, render_debug_draws)
                    .in_set(ScriptingSet::DebugDraw),
            )
            // Cleanup systems (run after everything else)
            .add_systems(
                Update,
                (
                    clear_timers_on_stop,
                    clear_debug_draws_on_stop,
                    cleanup_audio_on_stop,
                    reset_camera_on_stop,
                    clear_collisions_on_stop,
                    clear_scene_queue_on_stop,
                    despawn_runtime_prefabs_on_stop,
                    clear_animation_on_stop,
                )
                    .in_set(ScriptingSet::Cleanup),
            );
    }
}

/// System to reset script runtime state when play mode starts
fn reset_scripts_on_play_start(
    play_mode: Res<PlayModeState>,
    mut scripts: Query<&mut ScriptComponent>,
    mut last_play_state: Local<PlayState>,
) {
    // Detect transition from Editing to Playing
    if *last_play_state != PlayState::Playing && play_mode.state == PlayState::Playing {
        let count = scripts.iter().count();
        if count > 0 {
            console_log(LogLevel::Info, "Script", format!("Play mode started - initializing {} script(s)", count));
        }
        for mut script in scripts.iter_mut() {
            script.runtime_state.initialized = false;
            script.runtime_state.has_error = false; // Reset errors on play start
        }
    }
    *last_play_state = play_mode.state;
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
            bevy::log::info!("[Scripting] Scripts folder set to: {:?}", scripts_folder);
            rhai_engine.set_scripts_folder(scripts_folder.clone());

            // Log available scripts
            let available = rhai_engine.get_available_scripts();
            bevy::log::info!("[Scripting] Found {} scripts: {:?}", available.len(), available.iter().map(|(n, _)| n).collect::<Vec<_>>());
        }
    }
}
