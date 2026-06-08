//! Cloth physics distribution plugin.
//!
//! Wraps the vendored `bevy_silk` verlet cloth engine and registers it with the
//! Renzora runtime via `renzora::add!`. Built as a `cdylib` and dlopen'd from
//! `plugins/` at startup — the same distribution-plugin model the postprocess
//! effects use, so a shipped game gets cloth only if this plugin sits in
//! `plugins/`.
//!
//! Add a `bevy_silk::prelude::ClothBuilder` to any entity with a mesh to turn
//! it into cloth (see the `bevy_silk` docs for pinning / stick-generation).

use bevy::prelude::*;

/// Runtime-scope plugin that installs `bevy_silk`'s cloth simulation.
#[derive(Default)]
pub struct ClothPlugin;

impl Plugin for ClothPlugin {
    fn build(&self, app: &mut App) {
        info!("[runtime] ClothPlugin (bevy_silk verlet cloth)");
        app.add_plugins(bevy_silk::prelude::ClothPlugin);
    }
}

renzora::add!(ClothPlugin);
