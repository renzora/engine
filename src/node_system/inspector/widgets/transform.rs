use bevy::prelude::*;
use bevy_egui::egui::{self, RichText};

use crate::ui::property_row;

/// Render the transform inspector
pub fn render_transform_inspector(ui: &mut egui::Ui, transform: &mut Transform) -> bool {
    let mut changed = false;

    // Position
    property_row(ui, 0, |ui| {
        ui.horizontal(|ui| {
            ui.label(RichText::new("Position").strong());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let mut z = transform.translation.z;
                let mut y = transform.translation.y;
                let mut x = transform.translation.x;

                if ui.add(egui::DragValue::new(&mut z).speed(0.1).prefix("Z ")).changed() {
                    transform.translation.z = z;
                    changed = true;
                }
                if ui.add(egui::DragValue::new(&mut y).speed(0.1).prefix("Y ")).changed() {
                    transform.translation.y = y;
                    changed = true;
                }
                if ui.add(egui::DragValue::new(&mut x).speed(0.1).prefix("X ")).changed() {
                    transform.translation.x = x;
                    changed = true;
                }
            });
        });
    });

    // Rotation (converted to Euler angles in degrees for display)
    let (rx, ry, rz) = transform.rotation.to_euler(EulerRot::XYZ);
    let mut rot = [rx.to_degrees(), ry.to_degrees(), rz.to_degrees()];

    property_row(ui, 1, |ui| {
        ui.horizontal(|ui| {
            ui.label(RichText::new("Rotation").strong());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.add(egui::DragValue::new(&mut rot[2]).speed(1.0).prefix("Z ").suffix("°")).changed() {
                    transform.rotation = Quat::from_euler(
                        EulerRot::XYZ,
                        rot[0].to_radians(),
                        rot[1].to_radians(),
                        rot[2].to_radians(),
                    );
                    changed = true;
                }
                if ui.add(egui::DragValue::new(&mut rot[1]).speed(1.0).prefix("Y ").suffix("°")).changed() {
                    transform.rotation = Quat::from_euler(
                        EulerRot::XYZ,
                        rot[0].to_radians(),
                        rot[1].to_radians(),
                        rot[2].to_radians(),
                    );
                    changed = true;
                }
                if ui.add(egui::DragValue::new(&mut rot[0]).speed(1.0).prefix("X ").suffix("°")).changed() {
                    transform.rotation = Quat::from_euler(
                        EulerRot::XYZ,
                        rot[0].to_radians(),
                        rot[1].to_radians(),
                        rot[2].to_radians(),
                    );
                    changed = true;
                }
            });
        });
    });

    // Scale
    property_row(ui, 2, |ui| {
        ui.horizontal(|ui| {
            ui.label(RichText::new("Scale").strong());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let mut sz = transform.scale.z;
                let mut sy = transform.scale.y;
                let mut sx = transform.scale.x;

                if ui.add(egui::DragValue::new(&mut sz).speed(0.01).prefix("Z ")).changed() {
                    transform.scale.z = sz;
                    changed = true;
                }
                if ui.add(egui::DragValue::new(&mut sy).speed(0.01).prefix("Y ")).changed() {
                    transform.scale.y = sy;
                    changed = true;
                }
                if ui.add(egui::DragValue::new(&mut sx).speed(0.01).prefix("X ")).changed() {
                    transform.scale.x = sx;
                    changed = true;
                }
            });
        });
    });

    changed
}
