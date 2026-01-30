//! Inspector widgets for physics nodes

use bevy::prelude::*;
use bevy_egui::egui::{self, RichText};

use crate::gizmo::GizmoState;
use crate::shared::{CollisionShapeData, CollisionShapeType, PhysicsBodyData, PhysicsBodyType};
use crate::ui::inline_property;
use super::utils::sanitize_f32;

/// Render the physics body inspector
pub fn render_physics_body_inspector(ui: &mut egui::Ui, body: &mut PhysicsBodyData) -> bool {
    let mut changed = false;
    let mut row = 0;

    // Sanitize values
    sanitize_f32(&mut body.mass, 0.001, 10000.0, 1.0);
    sanitize_f32(&mut body.gravity_scale, -10.0, 10.0, 1.0);
    sanitize_f32(&mut body.linear_damping, 0.0, 100.0, 0.0);
    sanitize_f32(&mut body.angular_damping, 0.0, 100.0, 0.0);

    // Body Type
    inline_property(ui, row, "Body Type", |ui| {
        ui.label(RichText::new(body.body_type.display_name()).strong());
    });
    row += 1;

    // Only show mass for dynamic bodies
    if body.body_type == PhysicsBodyType::RigidBody {
        changed |= inline_property(ui, row, "Mass", |ui| {
            ui.add(egui::DragValue::new(&mut body.mass).speed(0.1).range(0.001..=10000.0)).changed()
        });
        row += 1;
    }

    // Gravity scale
    changed |= inline_property(ui, row, "Gravity Scale", |ui| {
        ui.add(egui::DragValue::new(&mut body.gravity_scale).speed(0.1).range(-10.0..=10.0)).changed()
    });
    row += 1;

    // Damping (only for dynamic bodies)
    if body.body_type == PhysicsBodyType::RigidBody {
        changed |= inline_property(ui, row, "Linear Damping", |ui| {
            ui.add(egui::DragValue::new(&mut body.linear_damping).speed(0.01).range(0.0..=100.0)).changed()
        });
        row += 1;

        changed |= inline_property(ui, row, "Angular Damping", |ui| {
            ui.add(egui::DragValue::new(&mut body.angular_damping).speed(0.01).range(0.0..=100.0)).changed()
        });
        row += 1;
    }

    // Lock Rotation
    inline_property(ui, row, "Lock Rotation", |ui| {
        if ui.checkbox(&mut body.lock_rotation_x, "X").changed() {
            changed = true;
        }
        if ui.checkbox(&mut body.lock_rotation_y, "Y").changed() {
            changed = true;
        }
        if ui.checkbox(&mut body.lock_rotation_z, "Z").changed() {
            changed = true;
        }
    });
    row += 1;

    // Lock Translation
    inline_property(ui, row, "Lock Translation", |ui| {
        if ui.checkbox(&mut body.lock_translation_x, "X").changed() {
            changed = true;
        }
        if ui.checkbox(&mut body.lock_translation_y, "Y").changed() {
            changed = true;
        }
        if ui.checkbox(&mut body.lock_translation_z, "Z").changed() {
            changed = true;
        }
    });

    changed
}

/// Render the collision shape inspector
/// Returns (changed, should the edit mode button be handled)
pub fn render_collision_shape_inspector(
    ui: &mut egui::Ui,
    shape: &mut CollisionShapeData,
    entity: Entity,
    gizmo_state: &mut GizmoState,
) -> bool {
    let mut changed = false;
    let mut row = 0;

    // Sanitize values
    const POS_RANGE: f32 = 10000.0;
    sanitize_f32(&mut shape.offset.x, -POS_RANGE, POS_RANGE, 0.0);
    sanitize_f32(&mut shape.offset.y, -POS_RANGE, POS_RANGE, 0.0);
    sanitize_f32(&mut shape.offset.z, -POS_RANGE, POS_RANGE, 0.0);
    sanitize_f32(&mut shape.half_extents.x, 0.001, 1000.0, 0.5);
    sanitize_f32(&mut shape.half_extents.y, 0.001, 1000.0, 0.5);
    sanitize_f32(&mut shape.half_extents.z, 0.001, 1000.0, 0.5);
    sanitize_f32(&mut shape.radius, 0.001, 1000.0, 0.5);
    sanitize_f32(&mut shape.half_height, 0.001, 1000.0, 0.5);
    sanitize_f32(&mut shape.friction, 0.0, 2.0, 0.5);
    sanitize_f32(&mut shape.restitution, 0.0, 1.0, 0.0);

    // Edit Collider button
    let is_editing = gizmo_state.collider_edit.entity == Some(entity);
    inline_property(ui, row, "", |ui| {
        if is_editing {
            if ui.button("Done Editing").clicked() {
                gizmo_state.collider_edit.stop_editing();
            }
            ui.label(RichText::new("Editing...").italics().color(egui::Color32::from_rgb(100, 180, 100)));
        } else {
            if ui.button("Edit Collider").clicked() {
                gizmo_state.collider_edit.start_editing(entity);
            }
        }
    });
    row += 1;

    // Shape Type
    inline_property(ui, row, "Shape Type", |ui| {
        ui.label(RichText::new(shape.shape_type.display_name()).strong());
    });
    row += 1;

    // Offset
    inline_property(ui, row, "Offset", |ui| {
        if ui.add(egui::DragValue::new(&mut shape.offset.x).speed(0.01).prefix("X ")).changed() {
            changed = true;
        }
        if ui.add(egui::DragValue::new(&mut shape.offset.y).speed(0.01).prefix("Y ")).changed() {
            changed = true;
        }
        if ui.add(egui::DragValue::new(&mut shape.offset.z).speed(0.01).prefix("Z ")).changed() {
            changed = true;
        }
    });
    row += 1;

    // Shape-specific parameters
    match shape.shape_type {
        CollisionShapeType::Box => {
            inline_property(ui, row, "Half Extents", |ui| {
                if ui.add(egui::DragValue::new(&mut shape.half_extents.x).speed(0.01).range(0.001..=1000.0).prefix("X ")).changed() {
                    changed = true;
                }
                if ui.add(egui::DragValue::new(&mut shape.half_extents.y).speed(0.01).range(0.001..=1000.0).prefix("Y ")).changed() {
                    changed = true;
                }
                if ui.add(egui::DragValue::new(&mut shape.half_extents.z).speed(0.01).range(0.001..=1000.0).prefix("Z ")).changed() {
                    changed = true;
                }
            });
            row += 1;
        }
        CollisionShapeType::Sphere => {
            changed |= inline_property(ui, row, "Radius", |ui| {
                ui.add(egui::DragValue::new(&mut shape.radius).speed(0.01).range(0.001..=1000.0)).changed()
            });
            row += 1;
        }
        CollisionShapeType::Capsule | CollisionShapeType::Cylinder => {
            changed |= inline_property(ui, row, "Radius", |ui| {
                ui.add(egui::DragValue::new(&mut shape.radius).speed(0.01).range(0.001..=1000.0)).changed()
            });
            row += 1;

            changed |= inline_property(ui, row, "Half Height", |ui| {
                ui.add(egui::DragValue::new(&mut shape.half_height).speed(0.01).range(0.001..=1000.0)).changed()
            });
            row += 1;
        }
    }

    // Friction
    changed |= inline_property(ui, row, "Friction", |ui| {
        ui.add(egui::Slider::new(&mut shape.friction, 0.0..=2.0)).changed()
    });
    row += 1;

    // Restitution
    changed |= inline_property(ui, row, "Restitution", |ui| {
        ui.add(egui::Slider::new(&mut shape.restitution, 0.0..=1.0)).changed()
    });
    row += 1;

    // Is Sensor
    changed |= inline_property(ui, row, "Is Sensor", |ui| {
        ui.checkbox(&mut shape.is_sensor, "").changed()
    });

    changed
}
