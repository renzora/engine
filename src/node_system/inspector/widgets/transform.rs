use bevy::prelude::*;
use bevy_egui::egui::{self, RichText};

/// Render the transform inspector
pub fn render_transform_inspector(ui: &mut egui::Ui, transform: &mut Transform) -> bool {
    let mut changed = false;

    ui.add_space(4.0);

    // Position
    ui.horizontal(|ui| {
        ui.label(RichText::new("Position").strong());
    });
    ui.horizontal(|ui| {
        ui.label("X");
        let mut x = transform.translation.x;
        if ui.add(egui::DragValue::new(&mut x).speed(0.1)).changed() {
            transform.translation.x = x;
            changed = true;
        }

        ui.label("Y");
        let mut y = transform.translation.y;
        if ui.add(egui::DragValue::new(&mut y).speed(0.1)).changed() {
            transform.translation.y = y;
            changed = true;
        }

        ui.label("Z");
        let mut z = transform.translation.z;
        if ui.add(egui::DragValue::new(&mut z).speed(0.1)).changed() {
            transform.translation.z = z;
            changed = true;
        }
    });

    ui.add_space(8.0);

    // Rotation (converted to Euler angles in degrees for display)
    ui.horizontal(|ui| {
        ui.label(RichText::new("Rotation").strong());
    });
    let (rx, ry, rz) = transform.rotation.to_euler(EulerRot::XYZ);
    let mut rot = [rx.to_degrees(), ry.to_degrees(), rz.to_degrees()];

    ui.horizontal(|ui| {
        ui.label("X");
        if ui.add(egui::DragValue::new(&mut rot[0]).speed(1.0).suffix("°")).changed() {
            transform.rotation = Quat::from_euler(
                EulerRot::XYZ,
                rot[0].to_radians(),
                rot[1].to_radians(),
                rot[2].to_radians(),
            );
            changed = true;
        }

        ui.label("Y");
        if ui.add(egui::DragValue::new(&mut rot[1]).speed(1.0).suffix("°")).changed() {
            transform.rotation = Quat::from_euler(
                EulerRot::XYZ,
                rot[0].to_radians(),
                rot[1].to_radians(),
                rot[2].to_radians(),
            );
            changed = true;
        }

        ui.label("Z");
        if ui.add(egui::DragValue::new(&mut rot[2]).speed(1.0).suffix("°")).changed() {
            transform.rotation = Quat::from_euler(
                EulerRot::XYZ,
                rot[0].to_radians(),
                rot[1].to_radians(),
                rot[2].to_radians(),
            );
            changed = true;
        }
    });

    ui.add_space(8.0);

    // Scale
    ui.horizontal(|ui| {
        ui.label(RichText::new("Scale").strong());
    });
    ui.horizontal(|ui| {
        ui.label("X");
        let mut sx = transform.scale.x;
        if ui.add(egui::DragValue::new(&mut sx).speed(0.01)).changed() {
            transform.scale.x = sx;
            changed = true;
        }

        ui.label("Y");
        let mut sy = transform.scale.y;
        if ui.add(egui::DragValue::new(&mut sy).speed(0.01)).changed() {
            transform.scale.y = sy;
            changed = true;
        }

        ui.label("Z");
        let mut sz = transform.scale.z;
        if ui.add(egui::DragValue::new(&mut sz).speed(0.01)).changed() {
            transform.scale.z = sz;
            changed = true;
        }
    });

    ui.add_space(4.0);

    changed
}
