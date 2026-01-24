//! Inspector widget for camera nodes

use bevy_egui::egui::{self, Color32, RichText, TextureId, Vec2};

use crate::node_system::CameraNodeData;
use crate::shared::CameraRigData;
use crate::ui::property_row;

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

    ui.add_space(8.0);

    // Rig Settings header
    ui.label(RichText::new("Rig Settings").strong());
    ui.add_space(4.0);

    // Distance
    property_row(ui, 0, |ui| {
        ui.horizontal(|ui| {
            ui.label("Distance");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add(egui::DragValue::new(&mut rig_data.distance).speed(0.1).range(0.5..=50.0).suffix(" m"))
                    .changed()
                {
                    changed = true;
                }
            });
        });
    });

    // Height
    property_row(ui, 1, |ui| {
        ui.horizontal(|ui| {
            ui.label("Height");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add(egui::DragValue::new(&mut rig_data.height).speed(0.1).range(-10.0..=20.0).suffix(" m"))
                    .changed()
                {
                    changed = true;
                }
            });
        });
    });

    // Horizontal Offset (for over-the-shoulder cameras)
    property_row(ui, 0, |ui| {
        ui.horizontal(|ui| {
            ui.label("Horizontal Offset");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add(egui::DragValue::new(&mut rig_data.horizontal_offset).speed(0.1).range(-10.0..=10.0).suffix(" m"))
                    .changed()
                {
                    changed = true;
                }
            });
        });
    });

    ui.add_space(8.0);

    // Camera Settings header
    ui.label(RichText::new("Camera Settings").strong());
    ui.add_space(4.0);

    // Field of View
    property_row(ui, 1, |ui| {
        ui.horizontal(|ui| {
            ui.label("Field of View");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add(egui::DragValue::new(&mut rig_data.fov).speed(1.0).range(10.0..=120.0).suffix("°"))
                    .changed()
                {
                    changed = true;
                }
            });
        });
    });

    ui.add_space(8.0);

    // Smoothing Settings header
    ui.label(RichText::new("Smoothing").strong());
    ui.add_space(4.0);

    // Follow Smoothing
    property_row(ui, 0, |ui| {
        ui.horizontal(|ui| {
            ui.label("Follow Speed");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add(egui::DragValue::new(&mut rig_data.follow_smoothing).speed(0.1).range(0.1..=50.0))
                    .changed()
                {
                    changed = true;
                }
            });
        });
    });

    // Look Smoothing
    property_row(ui, 1, |ui| {
        ui.horizontal(|ui| {
            ui.label("Look Speed");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add(egui::DragValue::new(&mut rig_data.look_smoothing).speed(0.1).range(0.1..=50.0))
                    .changed()
                {
                    changed = true;
                }
            });
        });
    });

    ui.add_space(8.0);

    // Default Camera checkbox
    property_row(ui, 0, |ui| {
        if ui.checkbox(&mut rig_data.is_default_camera, "Default Camera").changed() {
            changed = true;
        }
    });

    // Info
    property_row(ui, 1, |ui| {
        ui.label(
            RichText::new("Third-person camera. Follows parent entity at runtime.")
                .color(Color32::from_rgb(100, 100, 110))
                .small()
                .italics(),
        );
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
