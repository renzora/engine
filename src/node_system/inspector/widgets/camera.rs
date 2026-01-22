//! Inspector widget for camera nodes

use bevy_egui::egui::{self, Color32, CornerRadius, Margin, RichText, TextureId, Vec2};

use crate::node_system::CameraNodeData;

/// Render the camera inspector with preview
pub fn render_camera_inspector(
    ui: &mut egui::Ui,
    camera_data: &mut CameraNodeData,
    preview_texture_id: Option<TextureId>,
) -> bool {
    let mut changed = false;

    // Camera Preview section
    ui.add_space(8.0);
    egui::Frame::NONE
        .fill(Color32::from_rgb(40, 40, 48))
        .corner_radius(CornerRadius::same(3))
        .inner_margin(Margin::symmetric(8, 4))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.label(
                RichText::new("Camera Preview")
                    .color(Color32::from_rgb(180, 180, 190))
                    .strong(),
            );
        });
    ui.add_space(4.0);

    // Display the preview texture
    if let Some(texture_id) = preview_texture_id {
        let available_width = ui.available_width() - 16.0;
        let preview_height = available_width * (9.0 / 16.0); // 16:9 aspect ratio

        // Frame around preview
        egui::Frame::NONE
            .fill(Color32::from_rgb(20, 20, 26))
            .corner_radius(CornerRadius::same(4))
            .inner_margin(Margin::same(2))
            .show(ui, |ui| {
                let image = egui::Image::new(egui::load::SizedTexture::new(
                    texture_id,
                    [available_width, preview_height],
                ));
                ui.add(image);
            });
    } else {
        // No preview available placeholder
        let available_width = ui.available_width() - 16.0;
        let preview_height = available_width * (9.0 / 16.0);

        egui::Frame::NONE
            .fill(Color32::from_rgb(30, 30, 38))
            .corner_radius(CornerRadius::same(4))
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

    ui.add_space(8.0);

    // Camera Settings section
    egui::Frame::NONE
        .fill(Color32::from_rgb(40, 40, 48))
        .corner_radius(CornerRadius::same(3))
        .inner_margin(Margin::symmetric(8, 4))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.label(
                RichText::new("Settings")
                    .color(Color32::from_rgb(180, 180, 190))
                    .strong(),
            );
        });
    ui.add_space(4.0);

    // Field of View slider
    ui.horizontal(|ui| {
        ui.label("Field of View");
        if ui
            .add(egui::Slider::new(&mut camera_data.fov, 10.0..=120.0).suffix("°"))
            .changed()
        {
            changed = true;
        }
    });

    // Info about the camera
    ui.add_space(8.0);
    ui.label(
        RichText::new("This camera will be used at runtime.\nPosition and rotation are controlled by Transform.")
            .color(Color32::from_rgb(100, 100, 110))
            .small()
            .italics(),
    );

    ui.add_space(4.0);

    changed
}
