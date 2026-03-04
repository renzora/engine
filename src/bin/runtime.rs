//! Standalone game runtime — no editor UI.
//!
//! The runtime camera renders directly to the window.

use bevy::prelude::*;
use renzora_runtime::RuntimePlugin;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(RuntimePlugin)
        .run();
}
