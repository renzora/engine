//! Inspector widget for camera nodes

use bevy_egui::egui::{self, Color32, RichText, TextureId, Vec2};

use crate::shared::{CameraNodeData, CameraRigData};
use crate::ui::inline_property;
use super::utils::sanitize_f32;

/// Render the camera rig inspector
pub fn render_camera_rig_inspector(
    ui: &mut egui::Ui,
    rig_data: &mut CameraRigData,
    preview_texture_id: Option<TextureId>,
) -> bool {
    let mut changed = false;

    // Display the preview texture
    if let Some(texture_id) = preview_texture_id {
        let available_width = ui.available_width();
        let preview_height = available_width * (9.0 / 16.0);

        let image = egui::Image::new(egui::load::SizedTexture::new(
            texture_id,
            [available_width, preview_height],
        ));
        ui.add(image);
    } else {
        // No preview available placeholder
        let available_width = ui.available_width();
        let preview_height = available_width * (9.0 / 16.0);

        egui::Frame::new()
            .fill(Color32::from_rgb(30, 30, 38))
            .show(ui, |ui| {
                ui.set_min_size(Vec2::new(available_width, preview_height));
                ui.centered_and_justified(|ui| {
                    ui.label(
                        RichText::new("Preview loading...")
                            .color(Color32::from_rgb(100, 100, 110)),
                    );
                });
            });
    }

    ui.add_space(4.0);

    // Sanitize values
    sanitize_f32(&mut rig_data.distance, 0.5, 50.0, 5.0);
    sanitize_f32(&mut rig_data.height, -10.0, 20.0, 2.0);
    sanitize_f32(&mut rig_data.horizontal_offset, -10.0, 10.0, 0.0);
    sanitize_f32(&mut rig_data.fov, 10.0, 120.0, 60.0);
    sanitize_f32(&mut rig_data.follow_smoothing, 0.1, 50.0, 5.0);
    sanitize_f32(&mut rig_data.look_smoothing, 0.1, 50.0, 5.0);

    // Rig Settings
    let mut row = 0;

    changed |= inline_property(ui, row, "Distance", |ui| {
        ui.add(egui::DragValue::new(&mut rig_data.distance).speed(0.1).range(0.5..=50.0).suffix(" m")).changed()
    });
    row += 1;

    changed |= inline_property(ui, row, "Height", |ui| {
        ui.add(egui::DragValue::new(&mut rig_data.height).speed(0.1).range(-10.0..=20.0).suffix(" m")).changed()
    });
    row += 1;

    changed |= inline_property(ui, row, "Horizontal Offset", |ui| {
        ui.add(egui::DragValue::new(&mut rig_data.horizontal_offset).speed(0.1).range(-10.0..=10.0).suffix(" m")).changed()
    });
    row += 1;

    changed |= inline_property(ui, row, "Field of View", |ui| {
        ui.add(egui::DragValue::new(&mut rig_data.fov).speed(1.0).range(10.0..=120.0).suffix("°")).changed()
    });
    row += 1;

    changed |= inline_property(ui, row, "Follow Speed", |ui| {
        ui.add(egui::DragValue::new(&mut rig_data.follow_smoothing).speed(0.1).range(0.1..=50.0)).changed()
    });
    row += 1;

    changed |= inline_property(ui, row, "Look Speed", |ui| {
        ui.add(egui::DragValue::new(&mut rig_data.look_smoothing).speed(0.1).range(0.1..=50.0)).changed()
    });
    row += 1;

    changed |= inline_property(ui, row, "Default Camera", |ui| {
        ui.checkbox(&mut rig_data.is_default_camera, "").changed()
    });

    changed
}

/// Render the camera inspector with preview
pub fn render_camera_inspector(
    ui: &mut egui::Ui,
    camera_data: &mut CameraNodeData,
    preview_texture_id: Option<TextureId>,
) -> bool {
    let mut changed = false;

    // Display the preview texture
    if let Some(texture_id) = preview_texture_id {
        let available_width = ui.available_width();
        let preview_height = available_width * (9.0 / 16.0); // 16:9 aspect ratio

        let image = egui::Image::new(egui::load::SizedTexture::new(
            texture_id,
            [available_width, preview_height],
        ));
        ui.add(image);
    } else {
        // No preview available placeholder
        let available_width = ui.available_width();
        let preview_height = available_width * (9.0 / 16.0);

        egui::Frame::new()
            .fill(Color32::from_rgb(30, 30, 38))
            .show(ui, |ui| {
                ui.set_min_size(Vec2::new(available_width, preview_height));
                ui.centered_and_justified(|ui| {
                    ui.label(
                        RichText::new("Preview loading...")
                            .color(Color32::from_rgb(100, 100, 110)),
                    );
                });
            });
    }

    ui.add_space(4.0);

    // Sanitize values
    sanitize_f32(&mut camera_data.fov, 10.0, 120.0, 60.0);

    // Field of View
    changed |= inline_property(ui, 0, "Field of View", |ui| {
        ui.add(egui::DragValue::new(&mut camera_data.fov).speed(1.0).range(10.0..=120.0).suffix("°")).changed()
    });

    changed
}
