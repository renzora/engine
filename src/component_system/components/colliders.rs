//! Collision shape component definitions (box, sphere, capsule)

use bevy::prelude::*;
use bevy_egui::egui;

use crate::component_system::{ComponentCategory, ComponentRegistry};
use crate::register_component;
use crate::shared::{CollisionShapeData, CollisionShapeType};
use crate::ui::property_row;

use egui_phosphor::regular::{CUBE, GLOBE, PILL};

// ============================================================================
// Custom Add Functions
// ============================================================================

fn add_box_collider(
    commands: &mut Commands,
    entity: Entity,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) {
    commands.entity(entity).insert(CollisionShapeData {
        shape_type: CollisionShapeType::Box,
        ..Default::default()
    });
}

fn add_sphere_collider(
    commands: &mut Commands,
    entity: Entity,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) {
    commands.entity(entity).insert(CollisionShapeData {
        shape_type: CollisionShapeType::Sphere,
        radius: 0.5,
        ..Default::default()
    });
}

fn add_capsule_collider(
    commands: &mut Commands,
    entity: Entity,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) {
    commands.entity(entity).insert(CollisionShapeData {
        shape_type: CollisionShapeType::Capsule,
        radius: 0.3,
        half_height: 0.5,
        ..Default::default()
    });
}

// ============================================================================
// Custom Inspectors
// ============================================================================

fn inspect_box_collider(
    ui: &mut egui::Ui,
    world: &mut World,
    entity: Entity,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) -> bool {
    let Some(mut data) = world.get_mut::<CollisionShapeData>(entity) else {
        return false;
    };
    if data.shape_type != CollisionShapeType::Box {
        return false;
    }
    let mut changed = false;

    // Half Extents
    property_row(ui, 0, |ui| {
        ui.horizontal(|ui| {
            ui.label("Half Extents");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.horizontal(|ui| {
                    ui.label("Z");
                    if ui
                        .add(egui::DragValue::new(&mut data.half_extents.z).speed(0.01))
                        .changed()
                    {
                        changed = true;
                    }
                    ui.label("Y");
                    if ui
                        .add(egui::DragValue::new(&mut data.half_extents.y).speed(0.01))
                        .changed()
                    {
                        changed = true;
                    }
                    ui.label("X");
                    if ui
                        .add(egui::DragValue::new(&mut data.half_extents.x).speed(0.01))
                        .changed()
                    {
                        changed = true;
                    }
                });
            });
        });
    });

    // Friction
    property_row(ui, 1, |ui| {
        ui.horizontal(|ui| {
            ui.label("Friction");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add(
                        egui::DragValue::new(&mut data.friction)
                            .speed(0.01)
                            .range(0.0..=1.0),
                    )
                    .changed()
                {
                    changed = true;
                }
            });
        });
    });

    // Restitution
    property_row(ui, 2, |ui| {
        ui.horizontal(|ui| {
            ui.label("Restitution");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add(
                        egui::DragValue::new(&mut data.restitution)
                            .speed(0.01)
                            .range(0.0..=1.0),
                    )
                    .changed()
                {
                    changed = true;
                }
            });
        });
    });

    // Is Sensor
    property_row(ui, 3, |ui| {
        ui.horizontal(|ui| {
            ui.label("Is Sensor");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.checkbox(&mut data.is_sensor, "").changed() {
                    changed = true;
                }
            });
        });
    });

    changed
}

fn inspect_sphere_collider(
    ui: &mut egui::Ui,
    world: &mut World,
    entity: Entity,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) -> bool {
    let Some(mut data) = world.get_mut::<CollisionShapeData>(entity) else {
        return false;
    };
    if data.shape_type != CollisionShapeType::Sphere {
        return false;
    }
    let mut changed = false;

    // Radius
    property_row(ui, 0, |ui| {
        ui.horizontal(|ui| {
            ui.label("Radius");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add(
                        egui::DragValue::new(&mut data.radius)
                            .speed(0.01)
                            .range(0.001..=f32::MAX),
                    )
                    .changed()
                {
                    changed = true;
                }
            });
        });
    });

    // Friction
    property_row(ui, 1, |ui| {
        ui.horizontal(|ui| {
            ui.label("Friction");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add(
                        egui::DragValue::new(&mut data.friction)
                            .speed(0.01)
                            .range(0.0..=1.0),
                    )
                    .changed()
                {
                    changed = true;
                }
            });
        });
    });

    // Restitution
    property_row(ui, 2, |ui| {
        ui.horizontal(|ui| {
            ui.label("Restitution");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add(
                        egui::DragValue::new(&mut data.restitution)
                            .speed(0.01)
                            .range(0.0..=1.0),
                    )
                    .changed()
                {
                    changed = true;
                }
            });
        });
    });

    // Is Sensor
    property_row(ui, 3, |ui| {
        ui.horizontal(|ui| {
            ui.label("Is Sensor");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.checkbox(&mut data.is_sensor, "").changed() {
                    changed = true;
                }
            });
        });
    });

    changed
}

fn inspect_capsule_collider(
    ui: &mut egui::Ui,
    world: &mut World,
    entity: Entity,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) -> bool {
    let Some(mut data) = world.get_mut::<CollisionShapeData>(entity) else {
        return false;
    };
    if data.shape_type != CollisionShapeType::Capsule {
        return false;
    }
    let mut changed = false;

    // Radius
    property_row(ui, 0, |ui| {
        ui.horizontal(|ui| {
            ui.label("Radius");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add(
                        egui::DragValue::new(&mut data.radius)
                            .speed(0.01)
                            .range(0.001..=f32::MAX),
                    )
                    .changed()
                {
                    changed = true;
                }
            });
        });
    });

    // Half Height
    property_row(ui, 1, |ui| {
        ui.horizontal(|ui| {
            ui.label("Half Height");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add(
                        egui::DragValue::new(&mut data.half_height)
                            .speed(0.01)
                            .range(0.001..=f32::MAX),
                    )
                    .changed()
                {
                    changed = true;
                }
            });
        });
    });

    // Friction
    property_row(ui, 2, |ui| {
        ui.horizontal(|ui| {
            ui.label("Friction");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add(
                        egui::DragValue::new(&mut data.friction)
                            .speed(0.01)
                            .range(0.0..=1.0),
                    )
                    .changed()
                {
                    changed = true;
                }
            });
        });
    });

    // Restitution
    property_row(ui, 3, |ui| {
        ui.horizontal(|ui| {
            ui.label("Restitution");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add(
                        egui::DragValue::new(&mut data.restitution)
                            .speed(0.01)
                            .range(0.0..=1.0),
                    )
                    .changed()
                {
                    changed = true;
                }
            });
        });
    });

    // Is Sensor
    property_row(ui, 4, |ui| {
        ui.horizontal(|ui| {
            ui.label("Is Sensor");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.checkbox(&mut data.is_sensor, "").changed() {
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
    registry.register_owned(register_component!(CollisionShapeData {
        type_id: "box_collider",
        display_name: "Box Collider",
        category: ComponentCategory::Physics,
        icon: CUBE,
        priority: 1,
        conflicts_with: ["sphere_collider", "capsule_collider"],
        custom_inspector: inspect_box_collider,
        custom_add: add_box_collider,
    }));

    registry.register_owned(register_component!(CollisionShapeData {
        type_id: "sphere_collider",
        display_name: "Sphere Collider",
        category: ComponentCategory::Physics,
        icon: GLOBE,
        priority: 2,
        conflicts_with: ["box_collider", "capsule_collider"],
        custom_inspector: inspect_sphere_collider,
        custom_add: add_sphere_collider,
    }));

    registry.register_owned(register_component!(CollisionShapeData {
        type_id: "capsule_collider",
        display_name: "Capsule Collider",
        category: ComponentCategory::Physics,
        icon: PILL,
        priority: 3,
        conflicts_with: ["box_collider", "sphere_collider"],
        custom_inspector: inspect_capsule_collider,
        custom_add: add_capsule_collider,
    }));
}
