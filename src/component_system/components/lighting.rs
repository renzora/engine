//! Lighting component definitions

use bevy::prelude::*;
use bevy_egui::egui;
use serde_json::json;

use crate::component_system::{ComponentCategory, ComponentDefinition, ComponentRegistry};
use crate::ui::property_row;

// ============================================================================
// Component Definitions
// ============================================================================

pub static POINT_LIGHT: ComponentDefinition = ComponentDefinition {
    type_id: "point_light",
    display_name: "Point Light",
    category: ComponentCategory::Lighting,
    icon: "\u{e90f}", // Lightbulb
    priority: 0,
    add_fn: add_point_light,
    remove_fn: remove_point_light,
    has_fn: has_point_light,
    serialize_fn: serialize_point_light,
    deserialize_fn: deserialize_point_light,
    inspector_fn: inspect_point_light,
    conflicts_with: &["directional_light", "spot_light"],
    requires: &[],
};

pub static DIRECTIONAL_LIGHT: ComponentDefinition = ComponentDefinition {
    type_id: "directional_light",
    display_name: "Directional Light",
    category: ComponentCategory::Lighting,
    icon: "\u{e9b3}", // Sun
    priority: 1,
    add_fn: add_directional_light,
    remove_fn: remove_directional_light,
    has_fn: has_directional_light,
    serialize_fn: serialize_directional_light,
    deserialize_fn: deserialize_directional_light,
    inspector_fn: inspect_directional_light,
    conflicts_with: &["point_light", "spot_light"],
    requires: &[],
};

pub static SPOT_LIGHT: ComponentDefinition = ComponentDefinition {
    type_id: "spot_light",
    display_name: "Spot Light",
    category: ComponentCategory::Lighting,
    icon: "\u{e91a}", // Flashlight
    priority: 2,
    add_fn: add_spot_light,
    remove_fn: remove_spot_light,
    has_fn: has_spot_light,
    serialize_fn: serialize_spot_light,
    deserialize_fn: deserialize_spot_light,
    inspector_fn: inspect_spot_light,
    conflicts_with: &["point_light", "directional_light"],
    requires: &[],
};

/// Register all lighting components
pub fn register(registry: &mut ComponentRegistry) {
    registry.register(&POINT_LIGHT);
    registry.register(&DIRECTIONAL_LIGHT);
    registry.register(&SPOT_LIGHT);
}

// ============================================================================
// Point Light
// ============================================================================

fn add_point_light(
    commands: &mut Commands,
    entity: Entity,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) {
    commands.entity(entity).insert(PointLight {
        color: Color::WHITE,
        intensity: 800.0,
        range: 20.0,
        shadows_enabled: false,
        ..default()
    });
}

fn remove_point_light(commands: &mut Commands, entity: Entity) {
    commands.entity(entity).remove::<PointLight>();
}

fn has_point_light(world: &World, entity: Entity) -> bool {
    world.get::<PointLight>(entity).is_some()
}

fn serialize_point_light(world: &World, entity: Entity) -> Option<serde_json::Value> {
    let light = world.get::<PointLight>(entity)?;
    let srgba = light.color.to_srgba();
    Some(json!({
        "color": [srgba.red, srgba.green, srgba.blue],
        "intensity": light.intensity,
        "range": light.range,
        "shadows_enabled": light.shadows_enabled
    }))
}

fn deserialize_point_light(
    entity_commands: &mut EntityCommands,
    data: &serde_json::Value,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) {
    let color = data
        .get("color")
        .and_then(|c| c.as_array())
        .map(|arr| {
            Color::srgb(
                arr.first().and_then(|v| v.as_f64()).unwrap_or(1.0) as f32,
                arr.get(1).and_then(|v| v.as_f64()).unwrap_or(1.0) as f32,
                arr.get(2).and_then(|v| v.as_f64()).unwrap_or(1.0) as f32,
            )
        })
        .unwrap_or(Color::WHITE);

    let intensity = data
        .get("intensity")
        .and_then(|v| v.as_f64())
        .unwrap_or(800.0) as f32;

    let range = data.get("range").and_then(|v| v.as_f64()).unwrap_or(20.0) as f32;

    let shadows_enabled = data
        .get("shadows_enabled")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    entity_commands.insert(PointLight {
        color,
        intensity,
        range,
        shadows_enabled,
        ..default()
    });
}

fn inspect_point_light(
    ui: &mut egui::Ui,
    world: &mut World,
    entity: Entity,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) -> bool {
    let Some(mut light) = world.get_mut::<PointLight>(entity) else {
        return false;
    };
    let mut changed = false;

    // Color
    property_row(ui, 0, |ui| {
        ui.horizontal(|ui| {
            ui.label("Color");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let color_srgba = light.color.to_srgba();
                let mut color = egui::Color32::from_rgb(
                    (color_srgba.red * 255.0) as u8,
                    (color_srgba.green * 255.0) as u8,
                    (color_srgba.blue * 255.0) as u8,
                );
                if ui.color_edit_button_srgba(&mut color).changed() {
                    light.color = Color::srgb(
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
                let mut intensity = light.intensity;
                if ui
                    .add(
                        egui::DragValue::new(&mut intensity)
                            .speed(10.0)
                            .range(0.0..=f32::MAX),
                    )
                    .changed()
                {
                    light.intensity = intensity;
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
                let mut range = light.range;
                if ui
                    .add(
                        egui::DragValue::new(&mut range)
                            .speed(0.1)
                            .range(0.0..=f32::MAX),
                    )
                    .changed()
                {
                    light.range = range;
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
                if ui.checkbox(&mut light.shadows_enabled, "").changed() {
                    changed = true;
                }
            });
        });
    });

    changed
}

// ============================================================================
// Directional Light
// ============================================================================

fn add_directional_light(
    commands: &mut Commands,
    entity: Entity,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) {
    commands.entity(entity).insert(DirectionalLight {
        color: Color::WHITE,
        illuminance: 10000.0,
        shadows_enabled: true,
        ..default()
    });
}

fn remove_directional_light(commands: &mut Commands, entity: Entity) {
    commands.entity(entity).remove::<DirectionalLight>();
}

fn has_directional_light(world: &World, entity: Entity) -> bool {
    world.get::<DirectionalLight>(entity).is_some()
}

fn serialize_directional_light(world: &World, entity: Entity) -> Option<serde_json::Value> {
    let light = world.get::<DirectionalLight>(entity)?;
    let srgba = light.color.to_srgba();
    Some(json!({
        "color": [srgba.red, srgba.green, srgba.blue],
        "illuminance": light.illuminance,
        "shadows_enabled": light.shadows_enabled
    }))
}

fn deserialize_directional_light(
    entity_commands: &mut EntityCommands,
    data: &serde_json::Value,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) {
    let color = data
        .get("color")
        .and_then(|c| c.as_array())
        .map(|arr| {
            Color::srgb(
                arr.first().and_then(|v| v.as_f64()).unwrap_or(1.0) as f32,
                arr.get(1).and_then(|v| v.as_f64()).unwrap_or(1.0) as f32,
                arr.get(2).and_then(|v| v.as_f64()).unwrap_or(1.0) as f32,
            )
        })
        .unwrap_or(Color::WHITE);

    let illuminance = data
        .get("illuminance")
        .and_then(|v| v.as_f64())
        .unwrap_or(10000.0) as f32;

    let shadows_enabled = data
        .get("shadows_enabled")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);

    entity_commands.insert(DirectionalLight {
        color,
        illuminance,
        shadows_enabled,
        ..default()
    });
}

fn inspect_directional_light(
    ui: &mut egui::Ui,
    world: &mut World,
    entity: Entity,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) -> bool {
    let Some(mut light) = world.get_mut::<DirectionalLight>(entity) else {
        return false;
    };
    let mut changed = false;

    // Color
    property_row(ui, 0, |ui| {
        ui.horizontal(|ui| {
            ui.label("Color");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let color_srgba = light.color.to_srgba();
                let mut color = egui::Color32::from_rgb(
                    (color_srgba.red * 255.0) as u8,
                    (color_srgba.green * 255.0) as u8,
                    (color_srgba.blue * 255.0) as u8,
                );
                if ui.color_edit_button_srgba(&mut color).changed() {
                    light.color = Color::srgb(
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
                let mut illuminance = light.illuminance;
                if ui
                    .add(
                        egui::DragValue::new(&mut illuminance)
                            .speed(100.0)
                            .range(0.0..=f32::MAX),
                    )
                    .changed()
                {
                    light.illuminance = illuminance;
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
                if ui.checkbox(&mut light.shadows_enabled, "").changed() {
                    changed = true;
                }
            });
        });
    });

    changed
}

// ============================================================================
// Spot Light
// ============================================================================

fn add_spot_light(
    commands: &mut Commands,
    entity: Entity,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) {
    commands.entity(entity).insert(SpotLight {
        color: Color::WHITE,
        intensity: 800.0,
        range: 20.0,
        inner_angle: 0.3,
        outer_angle: 0.5,
        shadows_enabled: false,
        ..default()
    });
}

fn remove_spot_light(commands: &mut Commands, entity: Entity) {
    commands.entity(entity).remove::<SpotLight>();
}

fn has_spot_light(world: &World, entity: Entity) -> bool {
    world.get::<SpotLight>(entity).is_some()
}

fn serialize_spot_light(world: &World, entity: Entity) -> Option<serde_json::Value> {
    let light = world.get::<SpotLight>(entity)?;
    let srgba = light.color.to_srgba();
    Some(json!({
        "color": [srgba.red, srgba.green, srgba.blue],
        "intensity": light.intensity,
        "range": light.range,
        "inner_angle": light.inner_angle,
        "outer_angle": light.outer_angle,
        "shadows_enabled": light.shadows_enabled
    }))
}

fn deserialize_spot_light(
    entity_commands: &mut EntityCommands,
    data: &serde_json::Value,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) {
    let color = data
        .get("color")
        .and_then(|c| c.as_array())
        .map(|arr| {
            Color::srgb(
                arr.first().and_then(|v| v.as_f64()).unwrap_or(1.0) as f32,
                arr.get(1).and_then(|v| v.as_f64()).unwrap_or(1.0) as f32,
                arr.get(2).and_then(|v| v.as_f64()).unwrap_or(1.0) as f32,
            )
        })
        .unwrap_or(Color::WHITE);

    let intensity = data
        .get("intensity")
        .and_then(|v| v.as_f64())
        .unwrap_or(800.0) as f32;

    let range = data.get("range").and_then(|v| v.as_f64()).unwrap_or(20.0) as f32;

    let inner_angle = data
        .get("inner_angle")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.3) as f32;

    let outer_angle = data
        .get("outer_angle")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.5) as f32;

    let shadows_enabled = data
        .get("shadows_enabled")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    entity_commands.insert(SpotLight {
        color,
        intensity,
        range,
        inner_angle,
        outer_angle,
        shadows_enabled,
        ..default()
    });
}

fn inspect_spot_light(
    ui: &mut egui::Ui,
    world: &mut World,
    entity: Entity,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) -> bool {
    let Some(mut light) = world.get_mut::<SpotLight>(entity) else {
        return false;
    };
    let mut changed = false;

    // Color
    property_row(ui, 0, |ui| {
        ui.horizontal(|ui| {
            ui.label("Color");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let color_srgba = light.color.to_srgba();
                let mut color = egui::Color32::from_rgb(
                    (color_srgba.red * 255.0) as u8,
                    (color_srgba.green * 255.0) as u8,
                    (color_srgba.blue * 255.0) as u8,
                );
                if ui.color_edit_button_srgba(&mut color).changed() {
                    light.color = Color::srgb(
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
                let mut intensity = light.intensity;
                if ui
                    .add(
                        egui::DragValue::new(&mut intensity)
                            .speed(10.0)
                            .range(0.0..=f32::MAX),
                    )
                    .changed()
                {
                    light.intensity = intensity;
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
                let mut range = light.range;
                if ui
                    .add(
                        egui::DragValue::new(&mut range)
                            .speed(0.1)
                            .range(0.0..=f32::MAX),
                    )
                    .changed()
                {
                    light.range = range;
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
                let mut inner_deg = light.inner_angle.to_degrees();
                if ui
                    .add(
                        egui::DragValue::new(&mut inner_deg)
                            .speed(1.0)
                            .range(0.0..=90.0)
                            .suffix("\u{00b0}"),
                    )
                    .changed()
                {
                    light.inner_angle = inner_deg.to_radians();
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
                let mut outer_deg = light.outer_angle.to_degrees();
                if ui
                    .add(
                        egui::DragValue::new(&mut outer_deg)
                            .speed(1.0)
                            .range(0.0..=90.0)
                            .suffix("\u{00b0}"),
                    )
                    .changed()
                {
                    light.outer_angle = outer_deg.to_radians();
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
                if ui.checkbox(&mut light.shadows_enabled, "").changed() {
                    changed = true;
                }
            });
        });
    });

    changed
}
