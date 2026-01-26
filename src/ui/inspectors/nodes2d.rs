//! Inspector widgets for 2D nodes

use bevy_egui::egui::{self, Color32, RichText};

use crate::shared::{Camera2DData, Sprite2DData};
use crate::ui::property_row;

/// Render the Sprite2D inspector
pub fn render_sprite2d_inspector(ui: &mut egui::Ui, sprite_data: &mut Sprite2DData) -> bool {
    let mut changed = false;

    // Texture path
    property_row(ui, 0, |ui| {
        ui.horizontal(|ui| {
            ui.label("Texture");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let mut path = sprite_data.texture_path.clone();
                if ui.add(egui::TextEdit::singleline(&mut path).desired_width(150.0)).changed() {
                    sprite_data.texture_path = path;
                    changed = true;
                }
            });
        });
    });

    // Color tint
    property_row(ui, 1, |ui| {
        ui.horizontal(|ui| {
            ui.label("Color");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let mut color = [
                    sprite_data.color.x,
                    sprite_data.color.y,
                    sprite_data.color.z,
                    sprite_data.color.w,
                ];
                if ui.color_edit_button_rgba_unmultiplied(&mut color).changed() {
                    sprite_data.color.x = color[0];
                    sprite_data.color.y = color[1];
                    sprite_data.color.z = color[2];
                    sprite_data.color.w = color[3];
                    changed = true;
                }
            });
        });
    });

    // Flip controls
    property_row(ui, 0, |ui| {
        ui.horizontal(|ui| {
            if ui.checkbox(&mut sprite_data.flip_x, "Flip X").changed() {
                changed = true;
            }
            if ui.checkbox(&mut sprite_data.flip_y, "Flip Y").changed() {
                changed = true;
            }
        });
    });

    // Anchor
    property_row(ui, 1, |ui| {
        ui.horizontal(|ui| {
            ui.label("Anchor");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(4.0);
                if ui.add(egui::DragValue::new(&mut sprite_data.anchor.y).speed(0.01).range(0.0..=1.0).prefix("Y: ")).changed() {
                    changed = true;
                }
                if ui.add(egui::DragValue::new(&mut sprite_data.anchor.x).speed(0.01).range(0.0..=1.0).prefix("X: ")).changed() {
                    changed = true;
                }
            });
        });
    });

    // Info
    property_row(ui, 0, |ui| {
        ui.label(
            RichText::new("2D sprite. Set texture path relative to assets folder.")
                .color(Color32::from_rgb(100, 100, 110))
                .small()
                .italics(),
        );
    });

    changed
}

/// Render the Camera2D inspector
pub fn render_camera2d_inspector(ui: &mut egui::Ui, camera_data: &mut Camera2DData) -> bool {
    let mut changed = false;

    // Zoom
    property_row(ui, 0, |ui| {
        ui.horizontal(|ui| {
            ui.label("Zoom");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add(egui::DragValue::new(&mut camera_data.zoom).speed(0.01).range(0.1..=10.0).suffix("x"))
                    .changed()
                {
                    changed = true;
                }
            });
        });
    });

    // Default camera checkbox
    property_row(ui, 1, |ui| {
        if ui.checkbox(&mut camera_data.is_default_camera, "Default Camera").changed() {
            changed = true;
        }
    });

    // Info
    property_row(ui, 0, |ui| {
        ui.label(
            RichText::new("2D orthographic camera for 2D scenes.")
                .color(Color32::from_rgb(100, 100, 110))
                .small()
                .italics(),
        );
    });

    changed
}
