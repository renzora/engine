//! Inspector widgets for light components

use bevy::prelude::*;
use bevy_egui::egui;

use crate::ui::inline_property;
use super::utils::sanitize_f32;

// Large but finite maximum values for light parameters
const MAX_INTENSITY: f32 = 10_000_000.0;
const MAX_RANGE: f32 = 10_000.0;

/// Render the point light inspector
pub fn render_point_light_inspector(ui: &mut egui::Ui, light: &mut PointLight) -> bool {
    let mut changed = false;

    // Sanitize values
    sanitize_f32(&mut light.intensity, 0.0, MAX_INTENSITY, 800.0);
    sanitize_f32(&mut light.range, 0.0, MAX_RANGE, 20.0);

    // Color
    changed |= inline_property(ui, 0, "Color", |ui| {
        let color_srgba = light.color.to_srgba();
        let mut color = egui::Color32::from_rgb(
            (color_srgba.red * 255.0) as u8,
            (color_srgba.green * 255.0) as u8,
            (color_srgba.blue * 255.0) as u8,
        );
        let resp = ui.color_edit_button_srgba(&mut color).changed();
        if resp {
            light.color = Color::srgb(
                color.r() as f32 / 255.0,
                color.g() as f32 / 255.0,
                color.b() as f32 / 255.0,
            );
        }
        resp
    });

    // Intensity
    changed |= inline_property(ui, 1, "Intensity", |ui| {
        ui.add(egui::DragValue::new(&mut light.intensity).speed(10.0).range(0.0..=MAX_INTENSITY)).changed()
    });

    // Range
    changed |= inline_property(ui, 2, "Range", |ui| {
        ui.add(egui::DragValue::new(&mut light.range).speed(0.1).range(0.0..=MAX_RANGE)).changed()
    });

    // Shadows
    changed |= inline_property(ui, 3, "Shadows", |ui| {
        ui.checkbox(&mut light.shadows_enabled, "").changed()
    });

    changed
}

/// Render the directional light inspector
pub fn render_directional_light_inspector(ui: &mut egui::Ui, light: &mut DirectionalLight) -> bool {
    let mut changed = false;

    // Sanitize values
    sanitize_f32(&mut light.illuminance, 0.0, MAX_INTENSITY, 100000.0);

    // Color
    changed |= inline_property(ui, 0, "Color", |ui| {
        let color_srgba = light.color.to_srgba();
        let mut color = egui::Color32::from_rgb(
            (color_srgba.red * 255.0) as u8,
            (color_srgba.green * 255.0) as u8,
            (color_srgba.blue * 255.0) as u8,
        );
        let resp = ui.color_edit_button_srgba(&mut color).changed();
        if resp {
            light.color = Color::srgb(
                color.r() as f32 / 255.0,
                color.g() as f32 / 255.0,
                color.b() as f32 / 255.0,
            );
        }
        resp
    });

    // Illuminance
    changed |= inline_property(ui, 1, "Illuminance", |ui| {
        ui.add(egui::DragValue::new(&mut light.illuminance).speed(100.0).range(0.0..=MAX_INTENSITY)).changed()
    });

    // Shadows
    changed |= inline_property(ui, 2, "Shadows", |ui| {
        ui.checkbox(&mut light.shadows_enabled, "").changed()
    });

    changed
}

/// Render the spot light inspector
pub fn render_spot_light_inspector(ui: &mut egui::Ui, light: &mut SpotLight) -> bool {
    let mut changed = false;

    // Sanitize values
    sanitize_f32(&mut light.intensity, 0.0, MAX_INTENSITY, 800.0);
    sanitize_f32(&mut light.range, 0.0, MAX_RANGE, 20.0);
    sanitize_f32(&mut light.inner_angle, 0.0, std::f32::consts::FRAC_PI_2, 0.0);
    sanitize_f32(&mut light.outer_angle, 0.0, std::f32::consts::FRAC_PI_2, std::f32::consts::FRAC_PI_4);

    // Color
    changed |= inline_property(ui, 0, "Color", |ui| {
        let color_srgba = light.color.to_srgba();
        let mut color = egui::Color32::from_rgb(
            (color_srgba.red * 255.0) as u8,
            (color_srgba.green * 255.0) as u8,
            (color_srgba.blue * 255.0) as u8,
        );
        let resp = ui.color_edit_button_srgba(&mut color).changed();
        if resp {
            light.color = Color::srgb(
                color.r() as f32 / 255.0,
                color.g() as f32 / 255.0,
                color.b() as f32 / 255.0,
            );
        }
        resp
    });

    // Intensity
    changed |= inline_property(ui, 1, "Intensity", |ui| {
        ui.add(egui::DragValue::new(&mut light.intensity).speed(10.0).range(0.0..=MAX_INTENSITY)).changed()
    });

    // Range
    changed |= inline_property(ui, 2, "Range", |ui| {
        ui.add(egui::DragValue::new(&mut light.range).speed(0.1).range(0.0..=MAX_RANGE)).changed()
    });

    // Inner Angle
    inline_property(ui, 3, "Inner Angle", |ui| {
        let mut inner_deg = light.inner_angle.to_degrees();
        sanitize_f32(&mut inner_deg, 0.0, 90.0, 0.0);
        if ui.add(egui::DragValue::new(&mut inner_deg).speed(1.0).range(0.0..=90.0).suffix("°")).changed() {
            light.inner_angle = inner_deg.to_radians();
            changed = true;
        }
    });

    // Outer Angle
    inline_property(ui, 4, "Outer Angle", |ui| {
        let mut outer_deg = light.outer_angle.to_degrees();
        sanitize_f32(&mut outer_deg, 0.0, 90.0, 45.0);
        if ui.add(egui::DragValue::new(&mut outer_deg).speed(1.0).range(0.0..=90.0).suffix("°")).changed() {
            light.outer_angle = outer_deg.to_radians();
            changed = true;
        }
    });

    // Shadows
    changed |= inline_property(ui, 5, "Shadows", |ui| {
        ui.checkbox(&mut light.shadows_enabled, "").changed()
    });

    changed
}
