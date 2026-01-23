//! Inspector widgets for physics nodes

use bevy_egui::egui;

use crate::node_system::components::{CollisionShapeData, CollisionShapeType, PhysicsBodyData, PhysicsBodyType};
use crate::ui::property_row;

/// Render the physics body inspector
pub fn render_physics_body_inspector(ui: &mut egui::Ui, body: &mut PhysicsBodyData) -> bool {
    let mut changed = false;
    let mut row = 0;

    // Body Type
    property_row(ui, row, |ui| {
        ui.horizontal(|ui| {
            ui.label("Body Type");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(egui::RichText::new(body.body_type.display_name()).strong());
            });
        });
    });
    row += 1;

    // Only show mass for dynamic bodies
    if body.body_type == PhysicsBodyType::RigidBody {
        property_row(ui, row, |ui| {
            ui.horizontal(|ui| {
                ui.label("Mass");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .add(egui::DragValue::new(&mut body.mass).speed(0.1).range(0.001..=10000.0))
                        .changed()
                    {
                        changed = true;
                    }
                });
            });
        });
        row += 1;
    }

    // Gravity scale
    property_row(ui, row, |ui| {
        ui.horizontal(|ui| {
            ui.label("Gravity Scale");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add(egui::DragValue::new(&mut body.gravity_scale).speed(0.1).range(-10.0..=10.0))
                    .changed()
                {
                    changed = true;
                }
            });
        });
    });
    row += 1;

    // Damping (only for dynamic bodies)
    if body.body_type == PhysicsBodyType::RigidBody {
        property_row(ui, row, |ui| {
            ui.horizontal(|ui| {
                ui.label("Linear Damping");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .add(egui::DragValue::new(&mut body.linear_damping).speed(0.01).range(0.0..=100.0))
                        .changed()
                    {
                        changed = true;
                    }
                });
            });
        });
        row += 1;

        property_row(ui, row, |ui| {
            ui.horizontal(|ui| {
                ui.label("Angular Damping");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .add(egui::DragValue::new(&mut body.angular_damping).speed(0.01).range(0.0..=100.0))
                        .changed()
                    {
                        changed = true;
                    }
                });
            });
        });
        row += 1;
    }

    // Lock Rotation
    property_row(ui, row, |ui| {
        ui.horizontal(|ui| {
            ui.label("Lock Rotation");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.checkbox(&mut body.lock_rotation_z, "Z").changed() {
                    changed = true;
                }
                if ui.checkbox(&mut body.lock_rotation_y, "Y").changed() {
                    changed = true;
                }
                if ui.checkbox(&mut body.lock_rotation_x, "X").changed() {
                    changed = true;
                }
            });
        });
    });
    row += 1;

    // Lock Translation
    property_row(ui, row, |ui| {
        ui.horizontal(|ui| {
            ui.label("Lock Translation");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.checkbox(&mut body.lock_translation_z, "Z").changed() {
                    changed = true;
                }
                if ui.checkbox(&mut body.lock_translation_y, "Y").changed() {
                    changed = true;
                }
                if ui.checkbox(&mut body.lock_translation_x, "X").changed() {
                    changed = true;
                }
            });
        });
    });

    changed
}

/// Render the collision shape inspector
pub fn render_collision_shape_inspector(ui: &mut egui::Ui, shape: &mut CollisionShapeData) -> bool {
    let mut changed = false;
    let mut row = 0;

    // Shape Type
    property_row(ui, row, |ui| {
        ui.horizontal(|ui| {
            ui.label("Shape Type");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(egui::RichText::new(shape.shape_type.display_name()).strong());
            });
        });
    });
    row += 1;

    // Shape-specific parameters
    match shape.shape_type {
        CollisionShapeType::Box => {
            property_row(ui, row, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Half Extents");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.add(egui::DragValue::new(&mut shape.half_extents.z).speed(0.01).range(0.001..=1000.0).prefix("Z ")).changed() {
                            changed = true;
                        }
                        if ui.add(egui::DragValue::new(&mut shape.half_extents.y).speed(0.01).range(0.001..=1000.0).prefix("Y ")).changed() {
                            changed = true;
                        }
                        if ui.add(egui::DragValue::new(&mut shape.half_extents.x).speed(0.01).range(0.001..=1000.0).prefix("X ")).changed() {
                            changed = true;
                        }
                    });
                });
            });
            row += 1;
        }
        CollisionShapeType::Sphere => {
            property_row(ui, row, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Radius");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.add(egui::DragValue::new(&mut shape.radius).speed(0.01).range(0.001..=1000.0)).changed() {
                            changed = true;
                        }
                    });
                });
            });
            row += 1;
        }
        CollisionShapeType::Capsule | CollisionShapeType::Cylinder => {
            property_row(ui, row, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Radius");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.add(egui::DragValue::new(&mut shape.radius).speed(0.01).range(0.001..=1000.0)).changed() {
                            changed = true;
                        }
                    });
                });
            });
            row += 1;

            property_row(ui, row, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Half Height");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.add(egui::DragValue::new(&mut shape.half_height).speed(0.01).range(0.001..=1000.0)).changed() {
                            changed = true;
                        }
                    });
                });
            });
            row += 1;
        }
    }

    // Friction
    property_row(ui, row, |ui| {
        ui.horizontal(|ui| {
            ui.label("Friction");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.add(egui::Slider::new(&mut shape.friction, 0.0..=2.0)).changed() {
                    changed = true;
                }
            });
        });
    });
    row += 1;

    // Restitution
    property_row(ui, row, |ui| {
        ui.horizontal(|ui| {
            ui.label("Restitution");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.add(egui::Slider::new(&mut shape.restitution, 0.0..=1.0)).changed() {
                    changed = true;
                }
            });
        });
    });
    row += 1;

    // Is Sensor
    property_row(ui, row, |ui| {
        ui.horizontal(|ui| {
            ui.label("Is Sensor");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.checkbox(&mut shape.is_sensor, "").changed() {
                    changed = true;
                }
            });
        });
    });

    changed
}
