//! Physics plugin wrapper for Avian 3D
//!
//! Provides a shared physics setup that works for both editor and runtime.
//! In the editor, physics starts paused and is activated during play mode.
//! In the runtime, physics runs normally from the start.

#[cfg(feature = "physics")]
use avian3d::prelude::*;
use bevy::prelude::*;

/// Plugin that sets up Avian 3D physics for the engine.
///
/// # Usage
///
/// For the editor (physics starts paused):
/// ```rust,ignore
/// app.add_plugins(RenzoraPhysicsPlugin::new(true));
/// ```
///
/// For the runtime (physics runs immediately):
/// ```rust,ignore
/// app.add_plugins(RenzoraPhysicsPlugin::new(false));
/// ```
pub struct RenzoraPhysicsPlugin {
    /// Whether physics should start paused (editor) or running (runtime)
    start_paused: bool,
}

impl RenzoraPhysicsPlugin {
    /// Create a new physics plugin
    ///
    /// # Arguments
    /// * `start_paused` - If true, physics starts paused (for editor). If false, physics runs immediately (for runtime).
    pub fn new(start_paused: bool) -> Self {
        Self { start_paused }
    }
}

impl Plugin for RenzoraPhysicsPlugin {
    #[cfg(feature = "physics")]
    fn build(&self, app: &mut App) {
        // Add the Avian physics plugins
        app.add_plugins(PhysicsPlugins::default());

        // Register physics components for reflection (used in scene serialization)
        // Note: Avian's types need to be registered if we want them in scenes
        // For now we use our own wrapper types (PhysicsBodyData, CollisionShapeData)
        // which are converted to Avian components at runtime

        // Pause physics on startup if requested (editor mode)
        if self.start_paused {
            app.add_systems(Startup, pause_physics);
        }
    }

    #[cfg(not(feature = "physics"))]
    fn build(&self, _app: &mut App) {
        // Physics feature not enabled - no-op
    }
}

/// System to pause physics on startup (used in editor)
#[cfg(feature = "physics")]
fn pause_physics(mut time: ResMut<Time<Physics>>) {
    time.pause();
}

/// Pause the physics simulation
#[cfg(feature = "physics")]
pub fn physics_pause(time: &mut ResMut<Time<Physics>>) {
    time.pause();
}

/// Resume the physics simulation
#[cfg(feature = "physics")]
pub fn physics_unpause(time: &mut ResMut<Time<Physics>>) {
    time.unpause();
}

/// Check if physics is paused
#[cfg(feature = "physics")]
pub fn physics_is_paused(time: &Res<Time<Physics>>) -> bool {
    time.is_paused()
}

// Stub implementations when physics is disabled
#[cfg(not(feature = "physics"))]
pub fn physics_pause(_: &mut ()) {}

#[cfg(not(feature = "physics"))]
pub fn physics_unpause(_: &mut ()) {}

#[cfg(not(feature = "physics"))]
pub fn physics_is_paused(_: &()) -> bool {
    true
}
