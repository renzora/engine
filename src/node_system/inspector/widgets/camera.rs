//! Inspector widget for camera nodes

use bevy_egui::egui::{self, Color32, RichText, TextureId, Vec2};

use crate::node_system::CameraNodeData;
use crate::ui::property_row;

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

    // Field of View
    property_row(ui, 0, |ui| {
        ui.horizontal(|ui| {
            ui.label("Field of View");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add(egui::DragValue::new(&mut camera_data.fov).speed(1.0).range(10.0..=120.0).suffix("°"))
                    .changed()
                {
                    changed = true;
                }
            });
        });
    });

    // Info
    property_row(ui, 1, |ui| {
        ui.label(
            RichText::new("Runtime camera. Position controlled by Transform.")
                .color(Color32::from_rgb(100, 100, 110))
                .small()
                .italics(),
        );
    });

    changed
}
