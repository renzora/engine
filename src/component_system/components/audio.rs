//! Audio component definitions

use bevy::prelude::*;
use bevy_egui::egui;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::component_system::{ComponentCategory, ComponentDefinition, ComponentRegistry};

use egui_phosphor::regular::{SPEAKER_HIGH, SPEAKER_SIMPLE_HIGH};

// ============================================================================
// Audio Listener Component
// ============================================================================

#[derive(Component, Default, Clone)]
pub struct AudioListenerMarker;

pub static AUDIO_LISTENER: ComponentDefinition = ComponentDefinition {
    type_id: "audio_listener",
    display_name: "Audio Listener",
    category: ComponentCategory::Audio,
    icon: SPEAKER_HIGH,
    priority: 0,
    add_fn: add_audio_listener,
    remove_fn: remove_audio_listener,
    has_fn: has_audio_listener,
    serialize_fn: serialize_audio_listener,
    deserialize_fn: deserialize_audio_listener,
    inspector_fn: inspect_audio_listener,
    conflicts_with: &[],
    requires: &[],
};

// ============================================================================
// Audio Source Component
// ============================================================================

#[derive(Component, Clone, Serialize, Deserialize)]
pub struct AudioSourceData {
    pub audio_path: String,
    pub volume: f32,
    pub pitch: f32,
    pub looping: bool,
    pub play_on_start: bool,
    pub spatial: bool,
    pub min_distance: f32,
    pub max_distance: f32,
}

impl Default for AudioSourceData {
    fn default() -> Self {
        Self {
            audio_path: String::new(),
            volume: 1.0,
            pitch: 1.0,
            looping: false,
            play_on_start: false,
            spatial: true,
            min_distance: 1.0,
            max_distance: 50.0,
        }
    }
}

pub static AUDIO_SOURCE: ComponentDefinition = ComponentDefinition {
    type_id: "audio_source",
    display_name: "Audio Source",
    category: ComponentCategory::Audio,
    icon: SPEAKER_SIMPLE_HIGH,
    priority: 1,
    add_fn: add_audio_source,
    remove_fn: remove_audio_source,
    has_fn: has_audio_source,
    serialize_fn: serialize_audio_source,
    deserialize_fn: deserialize_audio_source,
    inspector_fn: inspect_audio_source,
    conflicts_with: &[],
    requires: &[],
};

/// Register all audio components
pub fn register(registry: &mut ComponentRegistry) {
    registry.register(&AUDIO_LISTENER);
    registry.register(&AUDIO_SOURCE);
}

// ============================================================================
// Audio Listener Implementation
// ============================================================================

fn add_audio_listener(
    commands: &mut Commands,
    entity: Entity,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) {
    commands.entity(entity).insert(AudioListenerMarker);
}

fn remove_audio_listener(commands: &mut Commands, entity: Entity) {
    commands.entity(entity).remove::<AudioListenerMarker>();
}

fn has_audio_listener(world: &World, entity: Entity) -> bool {
    world.get::<AudioListenerMarker>(entity).is_some()
}

fn serialize_audio_listener(_world: &World, _entity: Entity) -> Option<serde_json::Value> {
    Some(json!({}))
}

fn deserialize_audio_listener(
    entity_commands: &mut EntityCommands,
    _data: &serde_json::Value,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) {
    entity_commands.insert(AudioListenerMarker);
}

fn inspect_audio_listener(
    ui: &mut egui::Ui,
    _world: &mut World,
    _entity: Entity,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) -> bool {
    ui.label("Audio listener for 3D spatial audio.");
    ui.label("Attach to the player or camera.");
    false
}

// ============================================================================
// Audio Source Implementation
// ============================================================================

fn add_audio_source(
    commands: &mut Commands,
    entity: Entity,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) {
    commands.entity(entity).insert(AudioSourceData::default());
}

fn remove_audio_source(commands: &mut Commands, entity: Entity) {
    commands.entity(entity).remove::<AudioSourceData>();
}

fn has_audio_source(world: &World, entity: Entity) -> bool {
    world.get::<AudioSourceData>(entity).is_some()
}

fn serialize_audio_source(world: &World, entity: Entity) -> Option<serde_json::Value> {
    let data = world.get::<AudioSourceData>(entity)?;
    Some(json!({
        "audio_path": data.audio_path,
        "volume": data.volume,
        "pitch": data.pitch,
        "looping": data.looping,
        "play_on_start": data.play_on_start,
        "spatial": data.spatial,
        "min_distance": data.min_distance,
        "max_distance": data.max_distance,
    }))
}

fn deserialize_audio_source(
    entity_commands: &mut EntityCommands,
    data: &serde_json::Value,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) {
    let audio_data = AudioSourceData {
        audio_path: data.get("audio_path").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        volume: data.get("volume").and_then(|v| v.as_f64()).unwrap_or(1.0) as f32,
        pitch: data.get("pitch").and_then(|v| v.as_f64()).unwrap_or(1.0) as f32,
        looping: data.get("looping").and_then(|v| v.as_bool()).unwrap_or(false),
        play_on_start: data.get("play_on_start").and_then(|v| v.as_bool()).unwrap_or(false),
        spatial: data.get("spatial").and_then(|v| v.as_bool()).unwrap_or(true),
        min_distance: data.get("min_distance").and_then(|v| v.as_f64()).unwrap_or(1.0) as f32,
        max_distance: data.get("max_distance").and_then(|v| v.as_f64()).unwrap_or(50.0) as f32,
    };
    entity_commands.insert(audio_data);
}

fn inspect_audio_source(
    ui: &mut egui::Ui,
    world: &mut World,
    entity: Entity,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) -> bool {
    let mut changed = false;

    if let Some(mut data) = world.get_mut::<AudioSourceData>(entity) {
        ui.horizontal(|ui| {
            ui.label("Audio File:");
            if ui.text_edit_singleline(&mut data.audio_path).changed() {
                changed = true;
            }
        });

        ui.add_space(4.0);

        ui.horizontal(|ui| {
            ui.label("Volume:");
            if ui.add(egui::Slider::new(&mut data.volume, 0.0..=2.0)).changed() {
                changed = true;
            }
        });

        ui.horizontal(|ui| {
            ui.label("Pitch:");
            if ui.add(egui::Slider::new(&mut data.pitch, 0.1..=3.0)).changed() {
                changed = true;
            }
        });

        ui.add_space(4.0);

        if ui.checkbox(&mut data.looping, "Loop").changed() {
            changed = true;
        }

        if ui.checkbox(&mut data.play_on_start, "Play on Start").changed() {
            changed = true;
        }

        ui.add_space(4.0);
        ui.separator();
        ui.label("Spatial Audio");

        if ui.checkbox(&mut data.spatial, "3D Spatial").changed() {
            changed = true;
        }

        if data.spatial {
            ui.horizontal(|ui| {
                ui.label("Min Distance:");
                if ui.add(egui::DragValue::new(&mut data.min_distance).speed(0.1).range(0.1..=100.0)).changed() {
                    changed = true;
                }
            });

            ui.horizontal(|ui| {
                ui.label("Max Distance:");
                if ui.add(egui::DragValue::new(&mut data.max_distance).speed(0.5).range(1.0..=500.0)).changed() {
                    changed = true;
                }
            });
        }
    }

    changed
}
