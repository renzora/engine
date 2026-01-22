use bevy::prelude::*;
use bevy_egui::egui;

/// Render the point light inspector
pub fn render_point_light_inspector(ui: &mut egui::Ui, light: &mut PointLight) -> bool {
    let mut changed = false;

    ui.add_space(4.0);

    // Color
    ui.horizontal(|ui| {
        ui.label("Color");
        let color_srgba = light.color.to_srgba();
        let mut color = egui::Color32::from_rgb(
            (color_srgba.red * 255.0) as u8,
            (color_srgba.green * 255.0) as u8,
            (color_srgba.blue * 255.0) as u8,
        );
        if ui.color_edit_button_srgba(&mut color).changed() {
            light.color = Color::srgb(
                color.r() as f32 / 255.0,
                color.g() as f32 / 255.0,
                color.b() as f32 / 255.0,
            );
            changed = true;
        }
    });

    ui.add_space(4.0);

    // Intensity
    ui.horizontal(|ui| {
        ui.label("Intensity");
        let mut intensity = light.intensity;
        if ui.add(egui::DragValue::new(&mut intensity).speed(10.0).range(0.0..=f32::MAX)).changed() {
            light.intensity = intensity;
            changed = true;
        }
    });

    ui.add_space(4.0);

    // Range
    ui.horizontal(|ui| {
        ui.label("Range");
        let mut range = light.range;
        if ui.add(egui::DragValue::new(&mut range).speed(0.1).range(0.0..=f32::MAX)).changed() {
            light.range = range;
            changed = true;
        }
    });

    ui.add_space(4.0);

    // Shadows
    ui.horizontal(|ui| {
        ui.label("Shadows");
        if ui.checkbox(&mut light.shadows_enabled, "").changed() {
            changed = true;
        }
    });

    ui.add_space(4.0);

    changed
}

/// Render the directional light inspector
pub fn render_directional_light_inspector(ui: &mut egui::Ui, light: &mut DirectionalLight) -> bool {
    let mut changed = false;

    ui.add_space(4.0);

    // Color
    ui.horizontal(|ui| {
        ui.label("Color");
        let color_srgba = light.color.to_srgba();
        let mut color = egui::Color32::from_rgb(
            (color_srgba.red * 255.0) as u8,
            (color_srgba.green * 255.0) as u8,
            (color_srgba.blue * 255.0) as u8,
        );
        if ui.color_edit_button_srgba(&mut color).changed() {
            light.color = Color::srgb(
                color.r() as f32 / 255.0,
                color.g() as f32 / 255.0,
                color.b() as f32 / 255.0,
            );
            changed = true;
        }
    });

    ui.add_space(4.0);

    // Illuminance
    ui.horizontal(|ui| {
        ui.label("Illuminance");
        let mut illuminance = light.illuminance;
        if ui.add(egui::DragValue::new(&mut illuminance).speed(100.0).range(0.0..=f32::MAX)).changed() {
            light.illuminance = illuminance;
            changed = true;
        }
    });

    ui.add_space(4.0);

    // Shadows
    ui.horizontal(|ui| {
        ui.label("Shadows");
        if ui.checkbox(&mut light.shadows_enabled, "").changed() {
            changed = true;
        }
    });

    ui.add_space(4.0);

    changed
}

/// Render the spot light inspector
pub fn render_spot_light_inspector(ui: &mut egui::Ui, light: &mut SpotLight) -> bool {
    let mut changed = false;

    ui.add_space(4.0);

    // Color
    ui.horizontal(|ui| {
        ui.label("Color");
        let color_srgba = light.color.to_srgba();
        let mut color = egui::Color32::from_rgb(
            (color_srgba.red * 255.0) as u8,
            (color_srgba.green * 255.0) as u8,
            (color_srgba.blue * 255.0) as u8,
        );
        if ui.color_edit_button_srgba(&mut color).changed() {
            light.color = Color::srgb(
                color.r() as f32 / 255.0,
                color.g() as f32 / 255.0,
                color.b() as f32 / 255.0,
            );
            changed = true;
        }
    });

    ui.add_space(4.0);

    // Intensity
    ui.horizontal(|ui| {
        ui.label("Intensity");
        let mut intensity = light.intensity;
        if ui.add(egui::DragValue::new(&mut intensity).speed(10.0).range(0.0..=f32::MAX)).changed() {
            light.intensity = intensity;
            changed = true;
        }
    });

    ui.add_space(4.0);

    // Range
    ui.horizontal(|ui| {
        ui.label("Range");
        let mut range = light.range;
        if ui.add(egui::DragValue::new(&mut range).speed(0.1).range(0.0..=f32::MAX)).changed() {
            light.range = range;
            changed = true;
        }
    });

    ui.add_space(4.0);

    // Inner Angle (in degrees for easier editing)
    ui.horizontal(|ui| {
        ui.label("Inner Angle");
        let mut inner_deg = light.inner_angle.to_degrees();
        if ui.add(egui::Slider::new(&mut inner_deg, 0.0..=90.0).suffix("°")).changed() {
            light.inner_angle = inner_deg.to_radians();
            changed = true;
        }
    });

    ui.add_space(4.0);

    // Outer Angle (in degrees)
    ui.horizontal(|ui| {
        ui.label("Outer Angle");
        let mut outer_deg = light.outer_angle.to_degrees();
        if ui.add(egui::Slider::new(&mut outer_deg, 0.0..=90.0).suffix("°")).changed() {
            light.outer_angle = outer_deg.to_radians();
            changed = true;
        }
    });

    ui.add_space(4.0);

    // Shadows
    ui.horizontal(|ui| {
        ui.label("Shadows");
        if ui.checkbox(&mut light.shadows_enabled, "").changed() {
            changed = true;
        }
    });

    ui.add_space(4.0);

    changed
}
