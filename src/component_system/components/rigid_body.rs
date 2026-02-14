//! Rigid body component definition

use bevy::prelude::*;
use bevy_egui::egui;

use crate::component_system::{ComponentCategory, ComponentRegistry, PropertyValue, PropertyValueType};
use crate::register_component;
use crate::component_system::{PhysicsBodyData, PhysicsBodyType};
use crate::ui::property_row;

use egui_phosphor::regular::ATOM;

// ============================================================================
// Custom Inspector
// ============================================================================

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

fn rigid_body_property_meta() -> Vec<(&'static str, PropertyValueType)> {
    vec![
        ("mass", PropertyValueType::Float),
        ("gravity_scale", PropertyValueType::Float),
        ("linear_damping", PropertyValueType::Float),
        ("angular_damping", PropertyValueType::Float),
    ]
}

fn rigid_body_get_props(world: &World, entity: Entity) -> Vec<(&'static str, PropertyValue)> {
    let Some(data) = world.get::<PhysicsBodyData>(entity) else { return vec![] };
    vec![
        ("mass", PropertyValue::Float(data.mass)),
        ("gravity_scale", PropertyValue::Float(data.gravity_scale)),
        ("linear_damping", PropertyValue::Float(data.linear_damping)),
        ("angular_damping", PropertyValue::Float(data.angular_damping)),
    ]
}

fn rigid_body_set_prop(world: &mut World, entity: Entity, prop: &str, val: &PropertyValue) -> bool {
    let Some(mut data) = world.get_mut::<PhysicsBodyData>(entity) else { return false };
    match prop {
        "mass" => { if let PropertyValue::Float(v) = val { data.mass = *v; true } else { false } }
        "gravity_scale" => { if let PropertyValue::Float(v) = val { data.gravity_scale = *v; true } else { false } }
        "linear_damping" => { if let PropertyValue::Float(v) = val { data.linear_damping = *v; true } else { false } }
        "angular_damping" => { if let PropertyValue::Float(v) = val { data.angular_damping = *v; true } else { false } }
        _ => false,
    }
}

// ============================================================================
// Registration
// ============================================================================

pub fn register(registry: &mut ComponentRegistry) {
    registry.register_owned(register_component!(PhysicsBodyData {
        type_id: "rigid_body",
        display_name: "Rigid Body",
        category: ComponentCategory::Physics,
        icon: ATOM,
        priority: 0,
        custom_inspector: inspect_rigid_body,
        custom_script_properties: rigid_body_get_props,
        custom_script_set: rigid_body_set_prop,
        custom_script_meta: rigid_body_property_meta,
    }));
}
