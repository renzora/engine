//! Directional light component definition

use bevy::prelude::*;
use bevy_egui::egui;

use crate::component_system::{ComponentCategory, ComponentRegistry};
use crate::register_component;
use crate::shared::DirectionalLightData;
use crate::ui::property_row;

use egui_phosphor::regular::SUN;

// ============================================================================
// Custom Add/Remove/Deserialize
// ============================================================================

fn add_directional_light(
    commands: &mut Commands,
    entity: Entity,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) {
    let data = DirectionalLightData::default();
    commands.entity(entity).insert((
        DirectionalLight {
            color: Color::srgb(data.color.x, data.color.y, data.color.z),
            illuminance: data.illuminance,
            shadows_enabled: data.shadows_enabled,
            ..default()
        },
        data,
    ));
}

fn remove_directional_light(commands: &mut Commands, entity: Entity) {
    commands
        .entity(entity)
        .remove::<DirectionalLight>()
        .remove::<DirectionalLightData>();
}

fn deserialize_directional_light(
    entity_commands: &mut EntityCommands,
    data: &serde_json::Value,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) {
    if let Ok(dl_data) = serde_json::from_value::<DirectionalLightData>(data.clone()) {
        entity_commands.insert((
            DirectionalLight {
                color: Color::srgb(dl_data.color.x, dl_data.color.y, dl_data.color.z),
                illuminance: dl_data.illuminance,
                shadows_enabled: dl_data.shadows_enabled,
                ..default()
            },
            dl_data,
        ));
    }
}

// ============================================================================
// Custom Inspector
// ============================================================================

fn inspect_directional_light(
    ui: &mut egui::Ui,
    world: &mut World,
    entity: Entity,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) -> bool {
    let Some(mut data) = world.get_mut::<DirectionalLightData>(entity) else {
        return false;
    };
    let mut changed = false;

    // Color
    property_row(ui, 0, |ui| {
        ui.horizontal(|ui| {
            ui.label("Color");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let mut color = egui::Color32::from_rgb(
                    (data.color.x * 255.0) as u8,
                    (data.color.y * 255.0) as u8,
                    (data.color.z * 255.0) as u8,
                );
                if ui.color_edit_button_srgba(&mut color).changed() {
                    data.color = Vec3::new(
                        color.r() as f32 / 255.0,
                        color.g() as f32 / 255.0,
                        color.b() as f32 / 255.0,
                    );
                    changed = true;
                }
            });
        });
    });

    // Illuminance
    property_row(ui, 1, |ui| {
        ui.horizontal(|ui| {
            ui.label("Illuminance");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add(
                        egui::DragValue::new(&mut data.illuminance)
                            .speed(100.0)
                            .range(0.0..=f32::MAX),
                    )
                    .changed()
                {
                    changed = true;
                }
            });
        });
    });

    // Shadows
    property_row(ui, 2, |ui| {
        ui.horizontal(|ui| {
            ui.label("Shadows");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.checkbox(&mut data.shadows_enabled, "").changed() {
                    changed = true;
                }
            });
        });
    });

    changed
}

// ============================================================================
// Registration
// ============================================================================

pub fn register(registry: &mut ComponentRegistry) {
    registry.register_owned(register_component!(DirectionalLightData {
        type_id: "directional_light",
        display_name: "Directional Light",
        category: ComponentCategory::Lighting,
        icon: SUN,
        priority: 1,
        conflicts_with: ["point_light", "spot_light", "sun"],
        custom_inspector: inspect_directional_light,
        custom_add: add_directional_light,
        custom_remove: remove_directional_light,
        custom_deserialize: deserialize_directional_light,
    }));
}
