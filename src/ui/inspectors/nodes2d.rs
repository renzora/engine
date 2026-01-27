//! Inspector widgets for 2D nodes

use bevy_egui::egui;

use crate::shared::{Camera2DData, Sprite2DData};
use crate::ui::inline_property;

/// Render the Sprite2D inspector
pub fn render_sprite2d_inspector(ui: &mut egui::Ui, sprite_data: &mut Sprite2DData) -> bool {
    let mut changed = false;
    let mut row = 0;

    // Texture path
    changed |= inline_property(ui, row, "Texture", |ui| {
        ui.add(egui::TextEdit::singleline(&mut sprite_data.texture_path).desired_width(120.0)).changed()
    });
    row += 1;

    // Color tint
    changed |= inline_property(ui, row, "Color", |ui| {
        let mut color = [
            sprite_data.color.x,
            sprite_data.color.y,
            sprite_data.color.z,
            sprite_data.color.w,
        ];
        let resp = ui.color_edit_button_rgba_unmultiplied(&mut color).changed();
        if resp {
            sprite_data.color.x = color[0];
            sprite_data.color.y = color[1];
            sprite_data.color.z = color[2];
            sprite_data.color.w = color[3];
        }
        resp
    });
    row += 1;

    // Flip controls
    inline_property(ui, row, "Flip", |ui| {
        if ui.checkbox(&mut sprite_data.flip_x, "X").changed() {
            changed = true;
        }
        if ui.checkbox(&mut sprite_data.flip_y, "Y").changed() {
            changed = true;
        }
    });
    row += 1;

    // Anchor
    inline_property(ui, row, "Anchor", |ui| {
        if ui.add(egui::DragValue::new(&mut sprite_data.anchor.x).speed(0.01).range(0.0..=1.0).prefix("X ")).changed() {
            changed = true;
        }
        if ui.add(egui::DragValue::new(&mut sprite_data.anchor.y).speed(0.01).range(0.0..=1.0).prefix("Y ")).changed() {
            changed = true;
        }
    });

    changed
}

/// Render the Camera2D inspector
pub fn render_camera2d_inspector(ui: &mut egui::Ui, camera_data: &mut Camera2DData) -> bool {
    let mut changed = false;

    // Zoom
    changed |= inline_property(ui, 0, "Zoom", |ui| {
        ui.add(egui::DragValue::new(&mut camera_data.zoom).speed(0.01).range(0.1..=10.0).suffix("x")).changed()
    });

    // Default camera checkbox
    changed |= inline_property(ui, 1, "Default Camera", |ui| {
        ui.checkbox(&mut camera_data.is_default_camera, "").changed()
    });

    changed
}
