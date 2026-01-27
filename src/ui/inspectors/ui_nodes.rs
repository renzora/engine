//! Inspector widgets for UI nodes

use bevy_egui::egui;

use crate::shared::{UIButtonData, UIImageData, UILabelData, UIPanelData};
use crate::ui::inline_property;

/// Render the UIPanel inspector
pub fn render_ui_panel_inspector(ui: &mut egui::Ui, panel_data: &mut UIPanelData) -> bool {
    let mut changed = false;
    let mut row = 0;

    // Size
    inline_property(ui, row, "Size", |ui| {
        if ui.add(egui::DragValue::new(&mut panel_data.width).speed(1.0).range(10.0..=2000.0).prefix("W ")).changed() {
            changed = true;
        }
        if ui.add(egui::DragValue::new(&mut panel_data.height).speed(1.0).range(10.0..=2000.0).prefix("H ")).changed() {
            changed = true;
        }
    });
    row += 1;

    // Background color
    changed |= inline_property(ui, row, "Background", |ui| {
        let mut color = [
            panel_data.background_color.x,
            panel_data.background_color.y,
            panel_data.background_color.z,
            panel_data.background_color.w,
        ];
        let resp = ui.color_edit_button_rgba_unmultiplied(&mut color).changed();
        if resp {
            panel_data.background_color.x = color[0];
            panel_data.background_color.y = color[1];
            panel_data.background_color.z = color[2];
            panel_data.background_color.w = color[3];
        }
        resp
    });
    row += 1;

    // Border radius
    changed |= inline_property(ui, row, "Border Radius", |ui| {
        ui.add(egui::DragValue::new(&mut panel_data.border_radius).speed(0.5).range(0.0..=50.0).suffix(" px")).changed()
    });
    row += 1;

    // Padding
    changed |= inline_property(ui, row, "Padding", |ui| {
        ui.add(egui::DragValue::new(&mut panel_data.padding).speed(0.5).range(0.0..=100.0).suffix(" px")).changed()
    });

    changed
}

/// Render the UILabel inspector
pub fn render_ui_label_inspector(ui: &mut egui::Ui, label_data: &mut UILabelData) -> bool {
    let mut changed = false;
    let mut row = 0;

    // Text content
    changed |= inline_property(ui, row, "Text", |ui| {
        ui.add(egui::TextEdit::singleline(&mut label_data.text).desired_width(120.0)).changed()
    });
    row += 1;

    // Font size
    changed |= inline_property(ui, row, "Font Size", |ui| {
        ui.add(egui::DragValue::new(&mut label_data.font_size).speed(0.5).range(8.0..=72.0).suffix(" px")).changed()
    });
    row += 1;

    // Text color
    changed |= inline_property(ui, row, "Color", |ui| {
        let mut color = [
            label_data.color.x,
            label_data.color.y,
            label_data.color.z,
            label_data.color.w,
        ];
        let resp = ui.color_edit_button_rgba_unmultiplied(&mut color).changed();
        if resp {
            label_data.color.x = color[0];
            label_data.color.y = color[1];
            label_data.color.z = color[2];
            label_data.color.w = color[3];
        }
        resp
    });

    changed
}

/// Render the UIButton inspector
pub fn render_ui_button_inspector(ui: &mut egui::Ui, button_data: &mut UIButtonData) -> bool {
    let mut changed = false;
    let mut row = 0;

    // Text content
    changed |= inline_property(ui, row, "Text", |ui| {
        ui.add(egui::TextEdit::singleline(&mut button_data.text).desired_width(120.0)).changed()
    });
    row += 1;

    // Size
    inline_property(ui, row, "Size", |ui| {
        if ui.add(egui::DragValue::new(&mut button_data.width).speed(1.0).range(40.0..=500.0).prefix("W ")).changed() {
            changed = true;
        }
        if ui.add(egui::DragValue::new(&mut button_data.height).speed(1.0).range(20.0..=200.0).prefix("H ")).changed() {
            changed = true;
        }
    });
    row += 1;

    // Font size
    changed |= inline_property(ui, row, "Font Size", |ui| {
        ui.add(egui::DragValue::new(&mut button_data.font_size).speed(0.5).range(8.0..=48.0).suffix(" px")).changed()
    });
    row += 1;

    // Normal color
    changed |= inline_property(ui, row, "Normal", |ui| {
        let mut color = [
            button_data.normal_color.x,
            button_data.normal_color.y,
            button_data.normal_color.z,
            button_data.normal_color.w,
        ];
        let resp = ui.color_edit_button_rgba_unmultiplied(&mut color).changed();
        if resp {
            button_data.normal_color.x = color[0];
            button_data.normal_color.y = color[1];
            button_data.normal_color.z = color[2];
            button_data.normal_color.w = color[3];
        }
        resp
    });
    row += 1;

    // Hover color
    changed |= inline_property(ui, row, "Hover", |ui| {
        let mut color = [
            button_data.hover_color.x,
            button_data.hover_color.y,
            button_data.hover_color.z,
            button_data.hover_color.w,
        ];
        let resp = ui.color_edit_button_rgba_unmultiplied(&mut color).changed();
        if resp {
            button_data.hover_color.x = color[0];
            button_data.hover_color.y = color[1];
            button_data.hover_color.z = color[2];
            button_data.hover_color.w = color[3];
        }
        resp
    });
    row += 1;

    // Pressed color
    changed |= inline_property(ui, row, "Pressed", |ui| {
        let mut color = [
            button_data.pressed_color.x,
            button_data.pressed_color.y,
            button_data.pressed_color.z,
            button_data.pressed_color.w,
        ];
        let resp = ui.color_edit_button_rgba_unmultiplied(&mut color).changed();
        if resp {
            button_data.pressed_color.x = color[0];
            button_data.pressed_color.y = color[1];
            button_data.pressed_color.z = color[2];
            button_data.pressed_color.w = color[3];
        }
        resp
    });
    row += 1;

    // Text color
    changed |= inline_property(ui, row, "Text Color", |ui| {
        let mut color = [
            button_data.text_color.x,
            button_data.text_color.y,
            button_data.text_color.z,
            button_data.text_color.w,
        ];
        let resp = ui.color_edit_button_rgba_unmultiplied(&mut color).changed();
        if resp {
            button_data.text_color.x = color[0];
            button_data.text_color.y = color[1];
            button_data.text_color.z = color[2];
            button_data.text_color.w = color[3];
        }
        resp
    });

    changed
}

/// Render the UIImage inspector
pub fn render_ui_image_inspector(ui: &mut egui::Ui, image_data: &mut UIImageData) -> bool {
    let mut changed = false;
    let mut row = 0;

    // Texture path
    changed |= inline_property(ui, row, "Texture", |ui| {
        ui.add(egui::TextEdit::singleline(&mut image_data.texture_path).desired_width(120.0)).changed()
    });
    row += 1;

    // Size
    inline_property(ui, row, "Size", |ui| {
        if ui.add(egui::DragValue::new(&mut image_data.width).speed(1.0).range(1.0..=2000.0).prefix("W ")).changed() {
            changed = true;
        }
        if ui.add(egui::DragValue::new(&mut image_data.height).speed(1.0).range(1.0..=2000.0).prefix("H ")).changed() {
            changed = true;
        }
    });
    row += 1;

    // Tint color
    changed |= inline_property(ui, row, "Tint", |ui| {
        let mut color = [
            image_data.tint.x,
            image_data.tint.y,
            image_data.tint.z,
            image_data.tint.w,
        ];
        let resp = ui.color_edit_button_rgba_unmultiplied(&mut color).changed();
        if resp {
            image_data.tint.x = color[0];
            image_data.tint.y = color[1];
            image_data.tint.z = color[2];
            image_data.tint.w = color[3];
        }
        resp
    });

    changed
}
