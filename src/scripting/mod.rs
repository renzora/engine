mod api;
mod component;
mod registry;
mod rhai_engine;
mod rhai_context;
mod rhai_commands;
mod rhai_api;
pub(crate) mod entity_data_store;
mod runtime;
pub mod resources;
pub mod systems;

#[cfg(test)]
mod tests;

pub use api::*;
pub use component::*;
pub use registry::*;
pub use rhai_engine::*;
pub use rhai_context::*;
pub use rhai_commands::*;
pub use runtime::{run_rhai_scripts, RuntimeMode, ScriptCommandQueues, ScriptComponentQueries, DeferredPropertyWrites, apply_deferred_property_writes, populate_entity_data_store};
pub use resources::*;
pub use systems::*;

use bevy::prelude::*;
use crate::core::{AppState, PlayModeState, PlayState};
use crate::core::resources::console::{console_log, LogLevel};
use crate::project::CurrentProject;
use std::path::PathBuf;

/// System sets for ordering scripting systems
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum ScriptingSet {
    /// Pre-script systems (input update, timer update, folder detection)
    PreScript,
    /// Script execution (run_rhai_scripts)
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
        app.insert_resource(ScriptRegistry::new())
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
            .init_resource::<ParticleScriptCommandQueue>()
            .init_resource::<DeferredPropertyWrites>()
            .init_resource::<ScriptCameraYaw>()
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
                    update_script_camera_yaw,
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
            // Entity data store population (exclusive system for registry-based properties)
            .add_systems(
                Update,
                populate_entity_data_store.in_set(ScriptingSet::PreScript),
            )
            // Script execution
            .add_systems(
                Update,
                (run_rhai_scripts,).in_set(ScriptingSet::ScriptExecution),
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
                    process_particle_script_commands,
                )
                    .in_set(ScriptingSet::CommandProcessing),
            )
            // Deferred property writes (exclusive system for cross-entity set() calls)
            .add_systems(
                Update,
                apply_deferred_property_writes.in_set(ScriptingSet::CommandProcessing),
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
                    clear_raycast_results_on_stop,
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
            for entry in script.scripts.iter_mut() {
                entry.runtime_state.initialized = false;
                entry.runtime_state.has_error = false;
            }
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

// =============================================================================
// RUNTIME SCRIPTING PLUGIN
// =============================================================================

/// System sets for runtime scripting (no editor state dependencies)
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum RuntimeScriptingSet {
    /// Pre-script systems
    PreScript,
    /// Script execution
    ScriptExecution,
    /// Command processing
    CommandProcessing,
    /// Debug draw
    DebugDraw,
}

/// Plugin for runtime scripting (exported games, no editor)
///
/// This is a standalone version of ScriptingPlugin that doesn't depend on
/// editor-specific state like PlayModeState or AppState.
pub struct RuntimeScriptingPlugin {
    /// Path to the scripts folder
    pub scripts_folder: PathBuf,
}

impl RuntimeScriptingPlugin {
    pub fn new(scripts_folder: impl Into<PathBuf>) -> Self {
        Self {
            scripts_folder: scripts_folder.into(),
        }
    }
}

impl Plugin for RuntimeScriptingPlugin {
    fn build(&self, app: &mut App) {
        // Initialize Rhai engine with scripts folder
        let mut engine = RhaiScriptEngine::new();
        engine.set_scripts_folder(self.scripts_folder.clone());

        // Log available scripts
        let available = engine.get_available_scripts();
        bevy::log::info!("[RuntimeScripting] Scripts folder: {:?}", self.scripts_folder);
        bevy::log::info!("[RuntimeScripting] Found {} scripts: {:?}",
            available.len(),
            available.iter().map(|(n, _)| n).collect::<Vec<_>>()
        );

        app.insert_resource(engine)
            .insert_resource(RuntimeMode)
            .init_resource::<ScriptInput>()
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
            .init_resource::<ParticleScriptCommandQueue>()
            .init_resource::<DeferredPropertyWrites>()
            .init_resource::<ScriptCameraYaw>()
            // Configure system set ordering (no run_if conditions - always runs)
            .configure_sets(
                Update,
                (
                    RuntimeScriptingSet::PreScript,
                    RuntimeScriptingSet::ScriptExecution,
                    RuntimeScriptingSet::CommandProcessing,
                    RuntimeScriptingSet::DebugDraw,
                ).chain(),
            )
            // Pre-script systems
            .add_systems(
                Update,
                (
                    update_script_input,
                    update_script_camera_yaw,
                    update_script_timers,
                    collect_collision_events,
                ).in_set(RuntimeScriptingSet::PreScript),
            )
            // Initialize scripts on first run
            .add_systems(
                Update,
                initialize_runtime_scripts.in_set(RuntimeScriptingSet::PreScript),
            )
            // Entity data store population (exclusive system for registry-based properties)
            .add_systems(
                Update,
                populate_entity_data_store.in_set(RuntimeScriptingSet::PreScript),
            )
            // Script execution
            .add_systems(
                Update,
                run_rhai_scripts.in_set(RuntimeScriptingSet::ScriptExecution),
            )
            // Command processing
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
                    process_particle_script_commands,
                ).in_set(RuntimeScriptingSet::CommandProcessing),
            )
            // Deferred property writes (exclusive system for cross-entity set() calls)
            .add_systems(
                Update,
                apply_deferred_property_writes.in_set(RuntimeScriptingSet::CommandProcessing),
            )
            // Debug draw
            .add_systems(
                Update,
                (tick_debug_draws, render_debug_draws).in_set(RuntimeScriptingSet::DebugDraw),
            );
    }
}

/// System to initialize scripts when running in runtime mode
fn initialize_runtime_scripts(
    mut scripts: Query<&mut ScriptComponent>,
    mut initialized: Local<bool>,
) {
    if *initialized {
        return;
    }

    let count = scripts.iter().count();
    if count > 0 {
        bevy::log::info!("[RuntimeScripting] Initializing {} script(s)", count);
        for mut script in scripts.iter_mut() {
            for entry in script.scripts.iter_mut() {
                entry.runtime_state.initialized = false;
                entry.runtime_state.has_error = false;
            }
        }
        *initialized = true;
    }
}
