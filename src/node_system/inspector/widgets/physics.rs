//! Inspector widgets for physics nodes

use bevy_egui::egui;

use crate::node_system::components::{CollisionShapeData, CollisionShapeType, PhysicsBodyData, PhysicsBodyType};

/// Render the physics body inspector
pub fn render_physics_body_inspector(ui: &mut egui::Ui, body: &mut PhysicsBodyData) -> bool {
    let mut changed = false;

    ui.add_space(4.0);

    // Body Type (display only - changing type requires different node)
    ui.horizontal(|ui| {
        ui.label("Body Type");
        ui.label(egui::RichText::new(body.body_type.display_name()).strong());
    });

    ui.add_space(4.0);

    // Only show mass for dynamic bodies
    if body.body_type == PhysicsBodyType::RigidBody {
        ui.horizontal(|ui| {
            ui.label("Mass");
            if ui
                .add(egui::DragValue::new(&mut body.mass).speed(0.1).range(0.001..=10000.0))
                .changed()
            {
                changed = true;
            }
        });
        ui.add_space(4.0);
    }

    // Gravity scale
    ui.horizontal(|ui| {
        ui.label("Gravity Scale");
        if ui
            .add(egui::DragValue::new(&mut body.gravity_scale).speed(0.1).range(-10.0..=10.0))
            .changed()
        {
            changed = true;
        }
    });

    ui.add_space(4.0);

    // Damping (only for dynamic bodies)
    if body.body_type == PhysicsBodyType::RigidBody {
        ui.collapsing("Damping", |ui| {
            ui.horizontal(|ui| {
                ui.label("Linear");
                if ui
                    .add(egui::DragValue::new(&mut body.linear_damping).speed(0.01).range(0.0..=100.0))
                    .changed()
                {
                    changed = true;
                }
            });

            ui.horizontal(|ui| {
                ui.label("Angular");
                if ui
                    .add(egui::DragValue::new(&mut body.angular_damping).speed(0.01).range(0.0..=100.0))
                    .changed()
                {
                    changed = true;
                }
            });
        });

        ui.add_space(4.0);
    }

    // Axis locks
    ui.collapsing("Axis Locks", |ui| {
        ui.label("Lock Rotation");
        ui.horizontal(|ui| {
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

        ui.add_space(4.0);

        ui.label("Lock Translation");
        ui.horizontal(|ui| {
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
    });

    ui.add_space(4.0);

    changed
}

/// Render the collision shape inspector
pub fn render_collision_shape_inspector(ui: &mut egui::Ui, shape: &mut CollisionShapeData) -> bool {
    let mut changed = false;

    ui.add_space(4.0);

    // Shape Type (display only - changing shape type requires different node)
    ui.horizontal(|ui| {
        ui.label("Shape Type");
        ui.label(egui::RichText::new(shape.shape_type.display_name()).strong());
    });

    ui.add_space(4.0);

    // Shape-specific parameters
    match shape.shape_type {
        CollisionShapeType::Box => {
            ui.label("Half Extents");

            ui.horizontal(|ui| {
                ui.label("X");
                if ui
                    .add(egui::DragValue::new(&mut shape.half_extents.x).speed(0.01).range(0.001..=1000.0))
                    .changed()
                {
                    changed = true;
                }
            });

            ui.horizontal(|ui| {
                ui.label("Y");
                if ui
                    .add(egui::DragValue::new(&mut shape.half_extents.y).speed(0.01).range(0.001..=1000.0))
                    .changed()
                {
                    changed = true;
                }
            });

            ui.horizontal(|ui| {
                ui.label("Z");
                if ui
                    .add(egui::DragValue::new(&mut shape.half_extents.z).speed(0.01).range(0.001..=1000.0))
                    .changed()
                {
                    changed = true;
                }
            });
        }
        CollisionShapeType::Sphere => {
            ui.horizontal(|ui| {
                ui.label("Radius");
                if ui
                    .add(egui::DragValue::new(&mut shape.radius).speed(0.01).range(0.001..=1000.0))
                    .changed()
                {
                    changed = true;
                }
            });
        }
        CollisionShapeType::Capsule | CollisionShapeType::Cylinder => {
            ui.horizontal(|ui| {
                ui.label("Radius");
                if ui
                    .add(egui::DragValue::new(&mut shape.radius).speed(0.01).range(0.001..=1000.0))
                    .changed()
                {
                    changed = true;
                }
            });

            ui.horizontal(|ui| {
                ui.label("Half Height");
                if ui
                    .add(egui::DragValue::new(&mut shape.half_height).speed(0.01).range(0.001..=1000.0))
                    .changed()
                {
                    changed = true;
                }
            });
        }
    }

    ui.add_space(8.0);
    ui.separator();
    ui.label("Material Properties");
    ui.add_space(4.0);

    // Friction
    ui.horizontal(|ui| {
        ui.label("Friction");
        if ui
            .add(egui::Slider::new(&mut shape.friction, 0.0..=2.0))
            .changed()
        {
            changed = true;
        }
    });

    ui.add_space(4.0);

    // Restitution (bounciness)
    ui.horizontal(|ui| {
        ui.label("Restitution");
        if ui
            .add(egui::Slider::new(&mut shape.restitution, 0.0..=1.0))
            .changed()
        {
            changed = true;
        }
    });

    ui.add_space(4.0);

    // Is Sensor
    ui.horizontal(|ui| {
        ui.label("Is Sensor");
        if ui.checkbox(&mut shape.is_sensor, "").changed() {
            changed = true;
        }
        ui.label("(triggers events, no collision)");
    });

    ui.add_space(4.0);

    changed
}
