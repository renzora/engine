//! Test component extension — demonstrates how to define custom components
//! and register them with the inspector from an extension crate.
//!
//! Defines two game components (`Health`, `Movement`), spawns test
//! entities, and registers inspector entries so their fields are editable.
//!
//! **Note:** Extensions must use `PostStartup` (not `Startup`) for entity spawning
//! to avoid scheduling conflicts with the editor/egui plugins.

use bevy::prelude::*;
use renzora_editor_framework::{AppEditorExt, Inspectable};

// ── Custom components ──────────────────────────────────────────────────────

/// Health component with current/max HP and a shield flag.
#[derive(Component, Default, Reflect, Inspectable)]
#[inspectable(name = "Health", icon = "HEART", category = "gameplay")]
pub struct Health {
    #[field(speed = 1.0, min = 0.0, max = 10000.0)]
    pub current: f32,
    #[field(speed = 1.0, min = 1.0, max = 10000.0)]
    pub max: f32,
    #[field(name = "Shield")]
    pub has_shield: bool,
}

/// Movement component with speed, jump height, and a grounded flag.
#[derive(Component, Default, Reflect, Inspectable)]
#[inspectable(name = "Movement", icon = "SNEAKER_MOVE", category = "gameplay")]
pub struct Movement {
    #[field(speed = 0.1, min = 0.0, max = 100.0)]
    pub speed: f32,
    #[field(speed = 0.05, min = 0.0, max = 50.0)]
    pub jump_height: f32,
    #[field(name = "Grounded")]
    pub is_grounded: bool,
}

// ── Plugin ─────────────────────────────────────────────────────────────────

/// Test component plugin — registers custom components with the inspector
/// and spawns test entities to demonstrate the system.
#[derive(Default)]
pub struct TestComponentPlugin;

impl Plugin for TestComponentPlugin {
    fn build(&self, app: &mut App) {
        info!("[editor] TestComponentPlugin");
        app.register_inspectable::<Health>();
        app.register_inspectable::<Movement>();
    }
}

renzora::add!(TestComponentPlugin, Editor);
