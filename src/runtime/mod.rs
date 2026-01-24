//! Runtime module for standalone game execution
//!
//! This module contains everything needed to run a game exported from the editor
//! without any editor UI or dependencies.

pub mod camera;
pub mod loader;

use bevy::prelude::*;

pub use camera::RuntimeCameraPlugin;
pub use loader::RuntimeLoaderPlugin;

/// Main plugin for the game runtime
pub struct RuntimePlugin;

impl Plugin for RuntimePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((RuntimeLoaderPlugin, RuntimeCameraPlugin));
    }
}
