//! Inspector widget for Transform component

use bevy::prelude::*;
use bevy_egui::egui;

use crate::ui::inline_property;
use super::utils::sanitize_f32;

/// Render the transform inspector
pub fn render_transform_inspector(ui: &mut egui::Ui, transform: &mut Transform) -> bool {
    let mut changed = false;

    // Large range for transform values
    const POS_MIN: f32 = -1_000_000.0;
    const POS_MAX: f32 = 1_000_000.0;
    const SCALE_MIN: f32 = 0.0001;
    const SCALE_MAX: f32 = 10_000.0;

    // Sanitize position
    sanitize_f32(&mut transform.translation.x, POS_MIN, POS_MAX, 0.0);
    sanitize_f32(&mut transform.translation.y, POS_MIN, POS_MAX, 0.0);
    sanitize_f32(&mut transform.translation.z, POS_MIN, POS_MAX, 0.0);

    // Sanitize scale
    sanitize_f32(&mut transform.scale.x, SCALE_MIN, SCALE_MAX, 1.0);
    sanitize_f32(&mut transform.scale.y, SCALE_MIN, SCALE_MAX, 1.0);
    sanitize_f32(&mut transform.scale.z, SCALE_MIN, SCALE_MAX, 1.0);

    // Sanitize rotation quaternion if invalid
    if !transform.rotation.is_finite() || transform.rotation.length_squared() < 0.0001 {
        transform.rotation = Quat::IDENTITY;
    }

    // Position
    inline_property(ui, 0, &crate::locale::t("common.position"), |ui| {
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

    // Sanitize rotation angles
    sanitize_f32(&mut rot[0], -360.0, 360.0, 0.0);
    sanitize_f32(&mut rot[1], -360.0, 360.0, 0.0);
    sanitize_f32(&mut rot[2], -360.0, 360.0, 0.0);

    inline_property(ui, 1, &crate::locale::t("common.rotation"), |ui| {
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
    inline_property(ui, 2, &crate::locale::t("common.scale"), |ui| {
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
