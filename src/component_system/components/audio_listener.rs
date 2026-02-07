//! Audio component definitions

use bevy::prelude::*;
use bevy_egui::egui;
use serde::{Deserialize, Serialize};

use crate::component_system::{ComponentCategory, ComponentRegistry};
use crate::register_component;

use egui_phosphor::regular::SPEAKER_HIGH;

// ============================================================================
// Data Types
// ============================================================================

#[derive(Component, Default, Clone, Debug, Reflect, Serialize, Deserialize)]
pub struct AudioListenerMarker;

// ============================================================================
// Custom Inspectors
// ============================================================================

fn inspect_audio_listener(
    ui: &mut egui::Ui, _world: &mut World, _entity: Entity,
    _meshes: &mut Assets<Mesh>, _materials: &mut Assets<StandardMaterial>,
) -> bool {
    ui.label("Audio listener for 3D spatial audio.");
    ui.label("Attach to the player or camera.");
    false
}

// ============================================================================
// Registration
// ============================================================================

pub fn register(registry: &mut ComponentRegistry) {
    registry.register_owned(register_component!(AudioListenerMarker {
        type_id: "audio_listener",
        display_name: "Audio Listener",
        category: ComponentCategory::Audio,
        icon: SPEAKER_HIGH,
        priority: 0,
        custom_inspector: inspect_audio_listener,
    }));
}
