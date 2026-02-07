//! Spot light component definition

use bevy::prelude::*;
use bevy_egui::egui;

use crate::component_system::{ComponentCategory, ComponentRegistry};
use crate::register_component;
use crate::shared::SpotLightData;
use crate::ui::property_row;

use egui_phosphor::regular::FLASHLIGHT;

// ============================================================================
// Custom Add/Remove/Deserialize
// ============================================================================

fn add_spot_light(
    commands: &mut Commands,
    entity: Entity,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) {
    let data = SpotLightData::default();
    commands.entity(entity).insert((
        SpotLight {
            color: Color::srgb(data.color.x, data.color.y, data.color.z),
            intensity: data.intensity,
            range: data.range,
            radius: data.radius,
            inner_angle: data.inner_angle,
            outer_angle: data.outer_angle,
            shadows_enabled: data.shadows_enabled,
            ..default()
        },
        data,
    ));
}

fn remove_spot_light(commands: &mut Commands, entity: Entity) {
    commands
        .entity(entity)
        .remove::<SpotLight>()
        .remove::<SpotLightData>();
}

fn deserialize_spot_light(
    entity_commands: &mut EntityCommands,
    data: &serde_json::Value,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) {
    if let Ok(sl_data) = serde_json::from_value::<SpotLightData>(data.clone()) {
        entity_commands.insert((
            SpotLight {
                color: Color::srgb(sl_data.color.x, sl_data.color.y, sl_data.color.z),
                intensity: sl_data.intensity,
                range: sl_data.range,
                radius: sl_data.radius,
                inner_angle: sl_data.inner_angle,
                outer_angle: sl_data.outer_angle,
                shadows_enabled: sl_data.shadows_enabled,
                ..default()
            },
            sl_data,
        ));
    }
}

// ============================================================================
// Custom Inspector
// ============================================================================

fn inspect_spot_light(
    ui: &mut egui::Ui,
    world: &mut World,
    entity: Entity,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) -> bool {
    let Some(mut data) = world.get_mut::<SpotLightData>(entity) else {
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

    // Inner Angle
    property_row(ui, 3, |ui| {
        ui.horizontal(|ui| {
            ui.label("Inner Angle");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let mut inner_deg = data.inner_angle.to_degrees();
                if ui
                    .add(
                        egui::DragValue::new(&mut inner_deg)
                            .speed(1.0)
                            .range(0.0..=90.0)
                            .suffix("\u{00b0}"),
                    )
                    .changed()
                {
                    data.inner_angle = inner_deg.to_radians();
                    changed = true;
                }
            });
        });
    });

    // Outer Angle
    property_row(ui, 4, |ui| {
        ui.horizontal(|ui| {
            ui.label("Outer Angle");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let mut outer_deg = data.outer_angle.to_degrees();
                if ui
                    .add(
                        egui::DragValue::new(&mut outer_deg)
                            .speed(1.0)
                            .range(0.0..=90.0)
                            .suffix("\u{00b0}"),
                    )
                    .changed()
                {
                    data.outer_angle = outer_deg.to_radians();
                    changed = true;
                }
            });
        });
    });

    // Shadows
    property_row(ui, 5, |ui| {
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
    registry.register_owned(register_component!(SpotLightData {
        type_id: "spot_light",
        display_name: "Spot Light",
        category: ComponentCategory::Lighting,
        icon: FLASHLIGHT,
        priority: 2,
        conflicts_with: ["point_light", "directional_light"],
        custom_inspector: inspect_spot_light,
        custom_add: add_spot_light,
        custom_remove: remove_spot_light,
        custom_deserialize: deserialize_spot_light,
    }));
}
