//! Inspector widgets for UI nodes

use bevy_egui::egui::{self, Color32, RichText};

use crate::shared::{UIButtonData, UIImageData, UILabelData, UIPanelData};
use crate::ui::property_row;

/// Render the UIPanel inspector
pub fn render_ui_panel_inspector(ui: &mut egui::Ui, panel_data: &mut UIPanelData) -> bool {
    let mut changed = false;

    // Size
    property_row(ui, 0, |ui| {
        ui.horizontal(|ui| {
            ui.label("Size");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.add(egui::DragValue::new(&mut panel_data.height).speed(1.0).range(10.0..=2000.0).prefix("H: ")).changed() {
                    changed = true;
                }
                if ui.add(egui::DragValue::new(&mut panel_data.width).speed(1.0).range(10.0..=2000.0).prefix("W: ")).changed() {
                    changed = true;
                }
            });
        });
    });

    // Background color
    property_row(ui, 1, |ui| {
        ui.horizontal(|ui| {
            ui.label("Background");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let mut color = [
                    panel_data.background_color.x,
                    panel_data.background_color.y,
                    panel_data.background_color.z,
                    panel_data.background_color.w,
                ];
                if ui.color_edit_button_rgba_unmultiplied(&mut color).changed() {
                    panel_data.background_color.x = color[0];
                    panel_data.background_color.y = color[1];
                    panel_data.background_color.z = color[2];
                    panel_data.background_color.w = color[3];
                    changed = true;
                }
            });
        });
    });

    // Border radius
    property_row(ui, 0, |ui| {
        ui.horizontal(|ui| {
            ui.label("Border Radius");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.add(egui::DragValue::new(&mut panel_data.border_radius).speed(0.5).range(0.0..=50.0).suffix(" px")).changed() {
                    changed = true;
                }
            });
        });
    });

    // Padding
    property_row(ui, 1, |ui| {
        ui.horizontal(|ui| {
            ui.label("Padding");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.add(egui::DragValue::new(&mut panel_data.padding).speed(0.5).range(0.0..=100.0).suffix(" px")).changed() {
                    changed = true;
                }
            });
        });
    });

    changed
}

/// Render the UILabel inspector
pub fn render_ui_label_inspector(ui: &mut egui::Ui, label_data: &mut UILabelData) -> bool {
    let mut changed = false;

    // Text content
    property_row(ui, 0, |ui| {
        ui.horizontal(|ui| {
            ui.label("Text");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.add(egui::TextEdit::singleline(&mut label_data.text).desired_width(150.0)).changed() {
                    changed = true;
                }
            });
        });
    });

    // Font size
    property_row(ui, 1, |ui| {
        ui.horizontal(|ui| {
            ui.label("Font Size");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.add(egui::DragValue::new(&mut label_data.font_size).speed(0.5).range(8.0..=72.0).suffix(" px")).changed() {
                    changed = true;
                }
            });
        });
    });

    // Text color
    property_row(ui, 0, |ui| {
        ui.horizontal(|ui| {
            ui.label("Color");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let mut color = [
                    label_data.color.x,
                    label_data.color.y,
                    label_data.color.z,
                    label_data.color.w,
                ];
                if ui.color_edit_button_rgba_unmultiplied(&mut color).changed() {
                    label_data.color.x = color[0];
                    label_data.color.y = color[1];
                    label_data.color.z = color[2];
                    label_data.color.w = color[3];
                    changed = true;
                }
            });
        });
    });

    changed
}

/// Render the UIButton inspector
pub fn render_ui_button_inspector(ui: &mut egui::Ui, button_data: &mut UIButtonData) -> bool {
    let mut changed = false;

    // Text content
    property_row(ui, 0, |ui| {
        ui.horizontal(|ui| {
            ui.label("Text");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.add(egui::TextEdit::singleline(&mut button_data.text).desired_width(150.0)).changed() {
                    changed = true;
                }
            });
        });
    });

    // Size
    property_row(ui, 1, |ui| {
        ui.horizontal(|ui| {
            ui.label("Size");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.add(egui::DragValue::new(&mut button_data.height).speed(1.0).range(20.0..=200.0).prefix("H: ")).changed() {
                    changed = true;
                }
                if ui.add(egui::DragValue::new(&mut button_data.width).speed(1.0).range(40.0..=500.0).prefix("W: ")).changed() {
                    changed = true;
                }
            });
        });
    });

    // Font size
    property_row(ui, 0, |ui| {
        ui.horizontal(|ui| {
            ui.label("Font Size");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.add(egui::DragValue::new(&mut button_data.font_size).speed(0.5).range(8.0..=48.0).suffix(" px")).changed() {
                    changed = true;
                }
            });
        });
    });

    ui.add_space(4.0);
    ui.label(RichText::new("Colors").strong());

    // Normal color
    property_row(ui, 1, |ui| {
        ui.horizontal(|ui| {
            ui.label("Normal");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let mut color = [
                    button_data.normal_color.x,
                    button_data.normal_color.y,
                    button_data.normal_color.z,
                    button_data.normal_color.w,
                ];
                if ui.color_edit_button_rgba_unmultiplied(&mut color).changed() {
                    button_data.normal_color.x = color[0];
                    button_data.normal_color.y = color[1];
                    button_data.normal_color.z = color[2];
                    button_data.normal_color.w = color[3];
                    changed = true;
                }
            });
        });
    });

    // Hover color
    property_row(ui, 0, |ui| {
        ui.horizontal(|ui| {
            ui.label("Hover");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let mut color = [
                    button_data.hover_color.x,
                    button_data.hover_color.y,
                    button_data.hover_color.z,
                    button_data.hover_color.w,
                ];
                if ui.color_edit_button_rgba_unmultiplied(&mut color).changed() {
                    button_data.hover_color.x = color[0];
                    button_data.hover_color.y = color[1];
                    button_data.hover_color.z = color[2];
                    button_data.hover_color.w = color[3];
                    changed = true;
                }
            });
        });
    });

    // Pressed color
    property_row(ui, 1, |ui| {
        ui.horizontal(|ui| {
            ui.label("Pressed");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let mut color = [
                    button_data.pressed_color.x,
                    button_data.pressed_color.y,
                    button_data.pressed_color.z,
                    button_data.pressed_color.w,
                ];
                if ui.color_edit_button_rgba_unmultiplied(&mut color).changed() {
                    button_data.pressed_color.x = color[0];
                    button_data.pressed_color.y = color[1];
                    button_data.pressed_color.z = color[2];
                    button_data.pressed_color.w = color[3];
                    changed = true;
                }
            });
        });
    });

    // Text color
    property_row(ui, 0, |ui| {
        ui.horizontal(|ui| {
            ui.label("Text Color");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let mut color = [
                    button_data.text_color.x,
                    button_data.text_color.y,
                    button_data.text_color.z,
                    button_data.text_color.w,
                ];
                if ui.color_edit_button_rgba_unmultiplied(&mut color).changed() {
                    button_data.text_color.x = color[0];
                    button_data.text_color.y = color[1];
                    button_data.text_color.z = color[2];
                    button_data.text_color.w = color[3];
                    changed = true;
                }
            });
        });
    });

    changed
}

/// Render the UIImage inspector
pub fn render_ui_image_inspector(ui: &mut egui::Ui, image_data: &mut UIImageData) -> bool {
    let mut changed = false;

    // Texture path
    property_row(ui, 0, |ui| {
        ui.horizontal(|ui| {
            ui.label("Texture");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.add(egui::TextEdit::singleline(&mut image_data.texture_path).desired_width(150.0)).changed() {
                    changed = true;
                }
            });
        });
    });

    // Size
    property_row(ui, 1, |ui| {
        ui.horizontal(|ui| {
            ui.label("Size");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.add(egui::DragValue::new(&mut image_data.height).speed(1.0).range(1.0..=2000.0).prefix("H: ")).changed() {
                    changed = true;
                }
                if ui.add(egui::DragValue::new(&mut image_data.width).speed(1.0).range(1.0..=2000.0).prefix("W: ")).changed() {
                    changed = true;
                }
            });
        });
    });

    // Tint color
    property_row(ui, 0, |ui| {
        ui.horizontal(|ui| {
            ui.label("Tint");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let mut color = [
                    image_data.tint.x,
                    image_data.tint.y,
                    image_data.tint.z,
                    image_data.tint.w,
                ];
                if ui.color_edit_button_rgba_unmultiplied(&mut color).changed() {
                    image_data.tint.x = color[0];
                    image_data.tint.y = color[1];
                    image_data.tint.z = color[2];
                    image_data.tint.w = color[3];
                    changed = true;
                }
            });
        });
    });

    // Info
    property_row(ui, 1, |ui| {
        ui.label(
            RichText::new("UI image. Set texture path relative to assets folder.")
                .color(Color32::from_rgb(100, 100, 110))
                .small()
                .italics(),
        );
    });

    changed
}
