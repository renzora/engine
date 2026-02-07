//! Point light component definition

use bevy::prelude::*;
use bevy_egui::egui;

use crate::component_system::{ComponentCategory, ComponentRegistry};
use crate::register_component;
use crate::shared::PointLightData;
use crate::ui::property_row;

use egui_phosphor::regular::LIGHTBULB;

// ============================================================================
// Custom Add/Remove/Deserialize
// ============================================================================

fn add_point_light(
    commands: &mut Commands,
    entity: Entity,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) {
    let data = PointLightData::default();
    commands.entity(entity).insert((
        PointLight {
            color: Color::srgb(data.color.x, data.color.y, data.color.z),
            intensity: data.intensity,
            range: data.range,
            radius: data.radius,
            shadows_enabled: data.shadows_enabled,
            ..default()
        },
        data,
    ));
}

fn remove_point_light(commands: &mut Commands, entity: Entity) {
    commands
        .entity(entity)
        .remove::<PointLight>()
        .remove::<PointLightData>();
}

fn deserialize_point_light(
    entity_commands: &mut EntityCommands,
    data: &serde_json::Value,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) {
    if let Ok(pl_data) = serde_json::from_value::<PointLightData>(data.clone()) {
        entity_commands.insert((
            PointLight {
                color: Color::srgb(pl_data.color.x, pl_data.color.y, pl_data.color.z),
                intensity: pl_data.intensity,
                range: pl_data.range,
                radius: pl_data.radius,
                shadows_enabled: pl_data.shadows_enabled,
                ..default()
            },
            pl_data,
        ));
    }
}

// ============================================================================
// Custom Inspector
// ============================================================================

fn inspect_point_light(
    ui: &mut egui::Ui,
    world: &mut World,
    entity: Entity,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) -> bool {
    let Some(mut data) = world.get_mut::<PointLightData>(entity) else {
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

    // Intensity
    property_row(ui, 1, |ui| {
        ui.horizontal(|ui| {
            ui.label("Intensity");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add(
                        egui::DragValue::new(&mut data.intensity)
                            .speed(10.0)
                            .range(0.0..=f32::MAX),
                    )
                    .changed()
                {
                    changed = true;
                }
            });
        });
    });

    // Range
    property_row(ui, 2, |ui| {
        ui.horizontal(|ui| {
            ui.label("Range");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add(
                        egui::DragValue::new(&mut data.range)
                            .speed(0.1)
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
    property_row(ui, 3, |ui| {
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
    registry.register_owned(register_component!(PointLightData {
        type_id: "point_light",
        display_name: "Point Light",
        category: ComponentCategory::Lighting,
        icon: LIGHTBULB,
        priority: 0,
        conflicts_with: ["directional_light", "spot_light"],
        custom_inspector: inspect_point_light,
        custom_add: add_point_light,
        custom_remove: remove_point_light,
        custom_deserialize: deserialize_point_light,
    }));
}
