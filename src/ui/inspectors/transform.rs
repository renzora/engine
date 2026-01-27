//! Inspector widget for Transform component

use bevy::prelude::*;
use bevy_egui::egui;

use crate::ui::inline_property;

/// Render the transform inspector
pub fn render_transform_inspector(ui: &mut egui::Ui, transform: &mut Transform) -> bool {
    let mut changed = false;

    // Position
    inline_property(ui, 0, "Position", |ui| {
        if ui.add(egui::DragValue::new(&mut transform.translation.x).speed(0.1).prefix("X ")).changed() {
            changed = true;
        }
        if ui.add(egui::DragValue::new(&mut transform.translation.y).speed(0.1).prefix("Y ")).changed() {
            changed = true;
        }
        if ui.add(egui::DragValue::new(&mut transform.translation.z).speed(0.1).prefix("Z ")).changed() {
            changed = true;
        }
    });

    // Rotation (converted to Euler angles in degrees for display)
    let (rx, ry, rz) = transform.rotation.to_euler(EulerRot::XYZ);
    let mut rot = [rx.to_degrees(), ry.to_degrees(), rz.to_degrees()];

    inline_property(ui, 1, "Rotation", |ui| {
        if ui.add(egui::DragValue::new(&mut rot[0]).speed(1.0).prefix("X ")).changed() {
            transform.rotation = Quat::from_euler(EulerRot::XYZ, rot[0].to_radians(), rot[1].to_radians(), rot[2].to_radians());
            changed = true;
        }
        if ui.add(egui::DragValue::new(&mut rot[1]).speed(1.0).prefix("Y ")).changed() {
            transform.rotation = Quat::from_euler(EulerRot::XYZ, rot[0].to_radians(), rot[1].to_radians(), rot[2].to_radians());
            changed = true;
        }
        if ui.add(egui::DragValue::new(&mut rot[2]).speed(1.0).prefix("Z ")).changed() {
            transform.rotation = Quat::from_euler(EulerRot::XYZ, rot[0].to_radians(), rot[1].to_radians(), rot[2].to_radians());
            changed = true;
        }
    });

    // Scale
    inline_property(ui, 2, "Scale", |ui| {
        if ui.add(egui::DragValue::new(&mut transform.scale.x).speed(0.01).prefix("X ")).changed() {
            changed = true;
        }
        if ui.add(egui::DragValue::new(&mut transform.scale.y).speed(0.01).prefix("Y ")).changed() {
            changed = true;
        }
        if ui.add(egui::DragValue::new(&mut transform.scale.z).speed(0.01).prefix("Z ")).changed() {
            changed = true;
        }
    });

    changed
}
