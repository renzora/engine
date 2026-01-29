//! Runtime Scripting System
//!
//! Self-contained scripting for the standalone runtime.
//! Uses Rhai for script execution without editor dependencies.

use bevy::prelude::*;
use std::path::PathBuf;

// Self-contained modules for runtime
mod context;
mod commands;
mod resources;
mod systems;
mod engine;
mod executor;
mod rhai_api;

// Re-export what we need
pub use engine::RuntimeScriptEngine;
pub use resources::*;

/// Marker resource indicating we're in runtime mode (not editor)
#[derive(Resource, Default)]
pub struct RuntimeMode;

/// System sets for runtime scripting
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum RuntimeScriptingSet {
    /// Pre-script systems (input, timers, collisions)
    PreScript,
    /// Script execution
    ScriptExecution,
    /// Command processing
    CommandProcessing,
    /// Debug draw
    DebugDraw,
}

/// Plugin for runtime scripting
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
        // Initialize script engine with scripts folder
        let mut engine = RuntimeScriptEngine::new();
        engine.set_scripts_folder(self.scripts_folder.clone());

        info!("[RuntimeScripting] Scripts folder: {:?}", self.scripts_folder);
        let available = engine.get_available_scripts();
        info!("[RuntimeScripting] Found {} scripts", available.len());

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
            .init_resource::<AnimationCommandQueue>()
            .init_resource::<ActiveTweens>()
            .init_resource::<HealthCommandQueue>()
            // Configure system set ordering
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
                    systems::update_script_input,
                    systems::update_script_timers,
                    systems::collect_collision_events,
                ).in_set(RuntimeScriptingSet::PreScript),
            )
            // Script execution
            .add_systems(
                Update,
                executor::run_runtime_scripts.in_set(RuntimeScriptingSet::ScriptExecution),
            )
            // Command processing
            .add_systems(
                Update,
                (
                    systems::process_physics_commands,
                    systems::process_audio_commands,
                    systems::update_audio_fades,
                    systems::process_rendering_commands,
                    systems::process_camera_commands,
                    systems::apply_camera_effects,
                    systems::process_animation_commands,
                    systems::update_tweens,
                    systems::process_health_commands,
                ).in_set(RuntimeScriptingSet::CommandProcessing),
            )
            // Debug draw
            .add_systems(
                Update,
                (
                    systems::tick_debug_draws,
                    systems::render_debug_draws,
                ).in_set(RuntimeScriptingSet::DebugDraw),
            );
    }
}
