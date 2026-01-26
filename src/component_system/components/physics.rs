//! Physics component definitions

use bevy::prelude::*;
use bevy_egui::egui;
use serde_json::json;

use crate::component_system::{ComponentCategory, ComponentDefinition, ComponentRegistry};
use crate::shared::{CollisionShapeData, CollisionShapeType, PhysicsBodyData, PhysicsBodyType};
use crate::ui::property_row;

use egui_phosphor::regular::{ATOM, CUBE, GLOBE, PILL};

// ============================================================================
// Component Definitions
// ============================================================================

pub static RIGID_BODY: ComponentDefinition = ComponentDefinition {
    type_id: "rigid_body",
    display_name: "Rigid Body",
    category: ComponentCategory::Physics,
    icon: ATOM,
    priority: 0,
    add_fn: add_rigid_body,
    remove_fn: remove_rigid_body,
    has_fn: has_rigid_body,
    serialize_fn: serialize_rigid_body,
    deserialize_fn: deserialize_rigid_body,
    inspector_fn: inspect_rigid_body,
    conflicts_with: &[],
    requires: &[],
};

pub static BOX_COLLIDER: ComponentDefinition = ComponentDefinition {
    type_id: "box_collider",
    display_name: "Box Collider",
    category: ComponentCategory::Physics,
    icon: CUBE,
    priority: 1,
    add_fn: add_box_collider,
    remove_fn: remove_box_collider,
    has_fn: has_box_collider,
    serialize_fn: serialize_box_collider,
    deserialize_fn: deserialize_box_collider,
    inspector_fn: inspect_box_collider,
    conflicts_with: &["sphere_collider", "capsule_collider"],
    requires: &[],
};

pub static SPHERE_COLLIDER: ComponentDefinition = ComponentDefinition {
    type_id: "sphere_collider",
    display_name: "Sphere Collider",
    category: ComponentCategory::Physics,
    icon: GLOBE,
    priority: 2,
    add_fn: add_sphere_collider,
    remove_fn: remove_sphere_collider,
    has_fn: has_sphere_collider,
    serialize_fn: serialize_sphere_collider,
    deserialize_fn: deserialize_sphere_collider,
    inspector_fn: inspect_sphere_collider,
    conflicts_with: &["box_collider", "capsule_collider"],
    requires: &[],
};

pub static CAPSULE_COLLIDER: ComponentDefinition = ComponentDefinition {
    type_id: "capsule_collider",
    display_name: "Capsule Collider",
    category: ComponentCategory::Physics,
    icon: PILL,
    priority: 3,
    add_fn: add_capsule_collider,
    remove_fn: remove_capsule_collider,
    has_fn: has_capsule_collider,
    serialize_fn: serialize_capsule_collider,
    deserialize_fn: deserialize_capsule_collider,
    inspector_fn: inspect_capsule_collider,
    conflicts_with: &["box_collider", "sphere_collider"],
    requires: &[],
};

/// Register all physics components
pub fn register(registry: &mut ComponentRegistry) {
    registry.register(&RIGID_BODY);
    registry.register(&BOX_COLLIDER);
    registry.register(&SPHERE_COLLIDER);
    registry.register(&CAPSULE_COLLIDER);
}

// ============================================================================
// Rigid Body
// ============================================================================

fn add_rigid_body(
    commands: &mut Commands,
    entity: Entity,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) {
    commands.entity(entity).insert(PhysicsBodyData::default());
}

fn remove_rigid_body(commands: &mut Commands, entity: Entity) {
    commands.entity(entity).remove::<PhysicsBodyData>();
}

fn has_rigid_body(world: &World, entity: Entity) -> bool {
    world.get::<PhysicsBodyData>(entity).is_some()
}

fn serialize_rigid_body(world: &World, entity: Entity) -> Option<serde_json::Value> {
    let data = world.get::<PhysicsBodyData>(entity)?;
    Some(json!({
        "body_type": format!("{:?}", data.body_type),
        "mass": data.mass,
        "gravity_scale": data.gravity_scale,
        "linear_damping": data.linear_damping,
        "angular_damping": data.angular_damping,
        "lock_rotation_x": data.lock_rotation_x,
        "lock_rotation_y": data.lock_rotation_y,
        "lock_rotation_z": data.lock_rotation_z,
        "lock_translation_x": data.lock_translation_x,
        "lock_translation_y": data.lock_translation_y,
        "lock_translation_z": data.lock_translation_z
    }))
}

fn deserialize_rigid_body(
    entity_commands: &mut EntityCommands,
    data: &serde_json::Value,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) {
    let body_type_str = data
        .get("body_type")
        .and_then(|v| v.as_str())
        .unwrap_or("RigidBody");

    let body_type = match body_type_str {
        "StaticBody" => PhysicsBodyType::StaticBody,
        "KinematicBody" => PhysicsBodyType::KinematicBody,
        _ => PhysicsBodyType::RigidBody,
    };

    entity_commands.insert(PhysicsBodyData {
        body_type,
        mass: data.get("mass").and_then(|v| v.as_f64()).unwrap_or(1.0) as f32,
        gravity_scale: data
            .get("gravity_scale")
            .and_then(|v| v.as_f64())
            .unwrap_or(1.0) as f32,
        linear_damping: data
            .get("linear_damping")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0) as f32,
        angular_damping: data
            .get("angular_damping")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.05) as f32,
        lock_rotation_x: data
            .get("lock_rotation_x")
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
        lock_rotation_y: data
            .get("lock_rotation_y")
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
        lock_rotation_z: data
            .get("lock_rotation_z")
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
        lock_translation_x: data
            .get("lock_translation_x")
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
        lock_translation_y: data
            .get("lock_translation_y")
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
        lock_translation_z: data
            .get("lock_translation_z")
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
    });
}

fn inspect_rigid_body(
    ui: &mut egui::Ui,
    world: &mut World,
    entity: Entity,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) -> bool {
    let Some(mut data) = world.get_mut::<PhysicsBodyData>(entity) else {
        return false;
    };
    let mut changed = false;

    // Body Type
    property_row(ui, 0, |ui| {
        ui.horizontal(|ui| {
            ui.label("Body Type");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let body_types = [
                    (PhysicsBodyType::RigidBody, "Dynamic"),
                    (PhysicsBodyType::StaticBody, "Static"),
                    (PhysicsBodyType::KinematicBody, "Kinematic"),
                ];

                let current_name = body_types
                    .iter()
                    .find(|(t, _)| *t == data.body_type)
                    .map(|(_, n)| *n)
                    .unwrap_or("Dynamic");

                egui::ComboBox::from_id_salt("body_type")
                    .selected_text(current_name)
                    .show_ui(ui, |ui| {
                        for (body_type, name) in body_types.iter() {
                            if ui
                                .selectable_value(&mut data.body_type, *body_type, *name)
                                .changed()
                            {
                                changed = true;
                            }
                        }
                    });
            });
        });
    });

    // Mass (only for dynamic bodies)
    if data.body_type == PhysicsBodyType::RigidBody {
        property_row(ui, 1, |ui| {
            ui.horizontal(|ui| {
                ui.label("Mass");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .add(
                            egui::DragValue::new(&mut data.mass)
                                .speed(0.1)
                                .range(0.001..=f32::MAX),
                        )
                        .changed()
                    {
                        changed = true;
                    }
                });
            });
        });

        // Gravity Scale
        property_row(ui, 2, |ui| {
            ui.horizontal(|ui| {
                ui.label("Gravity Scale");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .add(
                            egui::DragValue::new(&mut data.gravity_scale)
                                .speed(0.1)
                                .range(-10.0..=10.0),
                        )
                        .changed()
                    {
                        changed = true;
                    }
                });
            });
        });
    }

    // Linear Damping
    property_row(ui, 3, |ui| {
        ui.horizontal(|ui| {
            ui.label("Linear Damping");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add(
                        egui::DragValue::new(&mut data.linear_damping)
                            .speed(0.01)
                            .range(0.0..=10.0),
                    )
                    .changed()
                {
                    changed = true;
                }
            });
        });
    });

    // Angular Damping
    property_row(ui, 4, |ui| {
        ui.horizontal(|ui| {
            ui.label("Angular Damping");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add(
                        egui::DragValue::new(&mut data.angular_damping)
                            .speed(0.01)
                            .range(0.0..=10.0),
                    )
                    .changed()
                {
                    changed = true;
                }
            });
        });
    });

    // Lock Rotation section
    ui.add_space(4.0);
    ui.label("Lock Rotation");
    property_row(ui, 5, |ui| {
        ui.horizontal(|ui| {
            if ui.checkbox(&mut data.lock_rotation_x, "X").changed() {
                changed = true;
            }
            if ui.checkbox(&mut data.lock_rotation_y, "Y").changed() {
                changed = true;
            }
            if ui.checkbox(&mut data.lock_rotation_z, "Z").changed() {
                changed = true;
            }
        });
    });

    // Lock Translation section
    ui.add_space(4.0);
    ui.label("Lock Translation");
    property_row(ui, 6, |ui| {
        ui.horizontal(|ui| {
            if ui.checkbox(&mut data.lock_translation_x, "X").changed() {
                changed = true;
            }
            if ui.checkbox(&mut data.lock_translation_y, "Y").changed() {
                changed = true;
            }
            if ui.checkbox(&mut data.lock_translation_z, "Z").changed() {
                changed = true;
            }
        });
    });

    changed
}

// ============================================================================
// Box Collider
// ============================================================================

fn add_box_collider(
    commands: &mut Commands,
    entity: Entity,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) {
    commands.entity(entity).insert(CollisionShapeData {
        shape_type: CollisionShapeType::Box,
        half_extents: Vec3::splat(0.5),
        ..default()
    });
}

fn remove_box_collider(commands: &mut Commands, entity: Entity) {
    commands.entity(entity).remove::<CollisionShapeData>();
}

fn has_box_collider(world: &World, entity: Entity) -> bool {
    world
        .get::<CollisionShapeData>(entity)
        .map(|d| d.shape_type == CollisionShapeType::Box)
        .unwrap_or(false)
}

fn serialize_box_collider(world: &World, entity: Entity) -> Option<serde_json::Value> {
    let data = world.get::<CollisionShapeData>(entity)?;
    if data.shape_type != CollisionShapeType::Box {
        return None;
    }
    Some(json!({
        "half_extents": [data.half_extents.x, data.half_extents.y, data.half_extents.z],
        "friction": data.friction,
        "restitution": data.restitution,
        "is_sensor": data.is_sensor
    }))
}

fn deserialize_box_collider(
    entity_commands: &mut EntityCommands,
    data: &serde_json::Value,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) {
    let half_extents = data
        .get("half_extents")
        .and_then(|h| h.as_array())
        .map(|arr| {
            Vec3::new(
                arr.first().and_then(|v| v.as_f64()).unwrap_or(0.5) as f32,
                arr.get(1).and_then(|v| v.as_f64()).unwrap_or(0.5) as f32,
                arr.get(2).and_then(|v| v.as_f64()).unwrap_or(0.5) as f32,
            )
        })
        .unwrap_or(Vec3::splat(0.5));

    entity_commands.insert(CollisionShapeData {
        shape_type: CollisionShapeType::Box,
        half_extents,
        friction: data
            .get("friction")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.5) as f32,
        restitution: data
            .get("restitution")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0) as f32,
        is_sensor: data
            .get("is_sensor")
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
        ..default()
    });
}

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

// ============================================================================
// Sphere Collider
// ============================================================================

fn add_sphere_collider(
    commands: &mut Commands,
    entity: Entity,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) {
    commands.entity(entity).insert(CollisionShapeData {
        shape_type: CollisionShapeType::Sphere,
        radius: 0.5,
        ..default()
    });
}

fn remove_sphere_collider(commands: &mut Commands, entity: Entity) {
    commands.entity(entity).remove::<CollisionShapeData>();
}

fn has_sphere_collider(world: &World, entity: Entity) -> bool {
    world
        .get::<CollisionShapeData>(entity)
        .map(|d| d.shape_type == CollisionShapeType::Sphere)
        .unwrap_or(false)
}

fn serialize_sphere_collider(world: &World, entity: Entity) -> Option<serde_json::Value> {
    let data = world.get::<CollisionShapeData>(entity)?;
    if data.shape_type != CollisionShapeType::Sphere {
        return None;
    }
    Some(json!({
        "radius": data.radius,
        "friction": data.friction,
        "restitution": data.restitution,
        "is_sensor": data.is_sensor
    }))
}

fn deserialize_sphere_collider(
    entity_commands: &mut EntityCommands,
    data: &serde_json::Value,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) {
    entity_commands.insert(CollisionShapeData {
        shape_type: CollisionShapeType::Sphere,
        radius: data
            .get("radius")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.5) as f32,
        friction: data
            .get("friction")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.5) as f32,
        restitution: data
            .get("restitution")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0) as f32,
        is_sensor: data
            .get("is_sensor")
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
        ..default()
    });
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

// ============================================================================
// Capsule Collider
// ============================================================================

fn add_capsule_collider(
    commands: &mut Commands,
    entity: Entity,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) {
    commands.entity(entity).insert(CollisionShapeData {
        shape_type: CollisionShapeType::Capsule,
        radius: 0.5,
        half_height: 0.5,
        ..default()
    });
}

fn remove_capsule_collider(commands: &mut Commands, entity: Entity) {
    commands.entity(entity).remove::<CollisionShapeData>();
}

fn has_capsule_collider(world: &World, entity: Entity) -> bool {
    world
        .get::<CollisionShapeData>(entity)
        .map(|d| d.shape_type == CollisionShapeType::Capsule)
        .unwrap_or(false)
}

fn serialize_capsule_collider(world: &World, entity: Entity) -> Option<serde_json::Value> {
    let data = world.get::<CollisionShapeData>(entity)?;
    if data.shape_type != CollisionShapeType::Capsule {
        return None;
    }
    Some(json!({
        "radius": data.radius,
        "half_height": data.half_height,
        "friction": data.friction,
        "restitution": data.restitution,
        "is_sensor": data.is_sensor
    }))
}

fn deserialize_capsule_collider(
    entity_commands: &mut EntityCommands,
    data: &serde_json::Value,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) {
    entity_commands.insert(CollisionShapeData {
        shape_type: CollisionShapeType::Capsule,
        radius: data
            .get("radius")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.5) as f32,
        half_height: data
            .get("half_height")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.5) as f32,
        friction: data
            .get("friction")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.5) as f32,
        restitution: data
            .get("restitution")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0) as f32,
        is_sensor: data
            .get("is_sensor")
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
        ..default()
    });
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
