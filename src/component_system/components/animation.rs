//! Animation component definitions

use bevy::prelude::*;
use bevy_egui::egui::{self, RichText, Color32};
use serde_json::json;

use crate::component_system::{ComponentCategory, ComponentDefinition, ComponentRegistry};
use crate::shared::AnimationData;
use crate::ui::property_row;

use egui_phosphor::regular::FILM_STRIP;

// ============================================================================
// Component Definitions
// ============================================================================

pub static ANIMATION: ComponentDefinition = ComponentDefinition {
    type_id: "animation",
    display_name: "Animation",
    category: ComponentCategory::Scripting, // Put in scripting category for now
    icon: FILM_STRIP,
    priority: 10,
    add_fn: add_animation,
    remove_fn: remove_animation,
    has_fn: has_animation,
    serialize_fn: serialize_animation,
    deserialize_fn: deserialize_animation,
    inspector_fn: inspect_animation,
    conflicts_with: &[],
    requires: &[],
};

/// Register all animation components
pub fn register(registry: &mut ComponentRegistry) {
    registry.register(&ANIMATION);
}

// ============================================================================
// Animation Component
// ============================================================================

fn add_animation(
    commands: &mut Commands,
    entity: Entity,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) {
    commands.entity(entity).insert(AnimationData::new());
}

fn remove_animation(commands: &mut Commands, entity: Entity) {
    commands.entity(entity).remove::<AnimationData>();
}

fn has_animation(world: &World, entity: Entity) -> bool {
    world.get::<AnimationData>(entity).is_some()
}

fn serialize_animation(world: &World, entity: Entity) -> Option<serde_json::Value> {
    world.get::<AnimationData>(entity).map(|data| {
        json!({
            "clips": data.clips.iter().map(|clip| {
                json!({
                    "name": clip.name,
                    "looping": clip.looping,
                    "speed": clip.speed,
                    "tracks": clip.tracks.iter().map(|track| {
                        json!({
                            "property": format!("{:?}", track.property),
                            "muted": track.muted,
                            "locked": track.locked,
                            "keyframes": track.keyframes.iter().map(|kf| {
                                json!({
                                    "time": kf.time,
                                    "value": match &kf.value {
                                        crate::shared::KeyframeValue::Float(v) => json!({"Float": v}),
                                        crate::shared::KeyframeValue::Vec3(v) => json!({"Vec3": [v.x, v.y, v.z]}),
                                        crate::shared::KeyframeValue::Quat(v) => json!({"Quat": [v.x, v.y, v.z, v.w]}),
                                        crate::shared::KeyframeValue::Color(v) => json!({"Color": v}),
                                    },
                                    "interpolation": format!("{:?}", kf.interpolation),
                                })
                            }).collect::<Vec<_>>()
                        })
                    }).collect::<Vec<_>>()
                })
            }).collect::<Vec<_>>(),
            "active_clip": data.active_clip
        })
    })
}

fn deserialize_animation(
    entity_commands: &mut EntityCommands,
    _value: &serde_json::Value,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) {
    // For now, just add empty animation data
    // Full deserialization would parse the JSON back to AnimationData
    entity_commands.insert(AnimationData::new());
}

fn inspect_animation(
    ui: &mut egui::Ui,
    world: &mut World,
    entity: Entity,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) -> bool {
    let Some(mut anim_data) = world.get_mut::<AnimationData>(entity) else {
        return false;
    };

    let mut changed = false;

    // Show clip count
    property_row(ui, 0, |ui| {
        ui.horizontal(|ui| {
            ui.label("Clips");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(RichText::new(format!("{}", anim_data.clips.len()))
                    .color(Color32::from_rgb(180, 200, 220)));
            });
        });
    });

    // Show active clip
    property_row(ui, 1, |ui| {
        ui.horizontal(|ui| {
            ui.label("Active Clip");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let active_name = anim_data.active_clip
                    .and_then(|i| anim_data.clips.get(i))
                    .map(|c| c.name.as_str())
                    .unwrap_or("None");
                ui.label(RichText::new(active_name)
                    .color(Color32::from_rgb(180, 200, 220)));
            });
        });
    });

    // Info message
    ui.add_space(8.0);
    ui.label(RichText::new("Use the Animation tab in the bottom panel to edit clips and keyframes.")
        .color(Color32::from_rgb(140, 142, 150))
        .small()
        .italics());

    // Add new clip button
    ui.add_space(4.0);
    if ui.button("Add Clip").clicked() {
        let clip_name = format!("Clip {}", anim_data.clips.len() + 1);
        anim_data.add_clip(crate::shared::EditorAnimationClip::new(clip_name));
        changed = true;
    }

    changed
}
