//! Inspector widgets for light components

use bevy::prelude::*;
use bevy_egui::egui;

use crate::ui::property_row;

/// Render the point light inspector
pub fn render_point_light_inspector(ui: &mut egui::Ui, light: &mut PointLight) -> bool {
    let mut changed = false;

    // Color
    property_row(ui, 0, |ui| {
        ui.horizontal(|ui| {
            ui.label("Color");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
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
        });
    });

    // Intensity
    property_row(ui, 1, |ui| {
        ui.horizontal(|ui| {
            ui.label("Intensity");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let mut intensity = light.intensity;
                if ui.add(egui::DragValue::new(&mut intensity).speed(10.0).range(0.0..=f32::MAX)).changed() {
                    light.intensity = intensity;
                    changed = true;
                }
            });
        });
    });

    // Range
    property_row(ui, 2, |ui| {
        ui.horizontal(|ui| {
            ui.label("Range");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let mut range = light.range;
                if ui.add(egui::DragValue::new(&mut range).speed(0.1).range(0.0..=f32::MAX)).changed() {
                    light.range = range;
                    changed = true;
                }
            });
        });
    });

    // Shadows
    property_row(ui, 3, |ui| {
        ui.horizontal(|ui| {
            ui.label("Shadows");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.checkbox(&mut light.shadows_enabled, "").changed() {
                    changed = true;
                }
            });
        });
    });

    changed
}

/// Render the directional light inspector
pub fn render_directional_light_inspector(ui: &mut egui::Ui, light: &mut DirectionalLight) -> bool {
    let mut changed = false;

    // Color
    property_row(ui, 0, |ui| {
        ui.horizontal(|ui| {
            ui.label("Color");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
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
        });
    });

    // Illuminance
    property_row(ui, 1, |ui| {
        ui.horizontal(|ui| {
            ui.label("Illuminance");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let mut illuminance = light.illuminance;
                if ui.add(egui::DragValue::new(&mut illuminance).speed(100.0).range(0.0..=f32::MAX)).changed() {
                    light.illuminance = illuminance;
                    changed = true;
                }
            });
        });
    });

    // Shadows
    property_row(ui, 2, |ui| {
        ui.horizontal(|ui| {
            ui.label("Shadows");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.checkbox(&mut light.shadows_enabled, "").changed() {
                    changed = true;
                }
            });
        });
    });

    changed
}

/// Render the spot light inspector
pub fn render_spot_light_inspector(ui: &mut egui::Ui, light: &mut SpotLight) -> bool {
    let mut changed = false;

    // Color
    property_row(ui, 0, |ui| {
        ui.horizontal(|ui| {
            ui.label("Color");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
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
        });
    });

    // Intensity
    property_row(ui, 1, |ui| {
        ui.horizontal(|ui| {
            ui.label("Intensity");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let mut intensity = light.intensity;
                if ui.add(egui::DragValue::new(&mut intensity).speed(10.0).range(0.0..=f32::MAX)).changed() {
                    light.intensity = intensity;
                    changed = true;
                }
            });
        });
    });

    // Range
    property_row(ui, 2, |ui| {
        ui.horizontal(|ui| {
            ui.label("Range");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let mut range = light.range;
                if ui.add(egui::DragValue::new(&mut range).speed(0.1).range(0.0..=f32::MAX)).changed() {
                    light.range = range;
                    changed = true;
                }
            });
        });
    });

    // Inner Angle
    property_row(ui, 3, |ui| {
        ui.horizontal(|ui| {
            ui.label("Inner Angle");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let mut inner_deg = light.inner_angle.to_degrees();
                if ui.add(egui::DragValue::new(&mut inner_deg).speed(1.0).range(0.0..=90.0).suffix("°")).changed() {
                    light.inner_angle = inner_deg.to_radians();
                    changed = true;
                }
            });
        });
    });

    // Outer Angle
    property_row(ui, 4, |ui| {
        ui.horizontal(|ui| {
            ui.label("Outer Angle");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let mut outer_deg = light.outer_angle.to_degrees();
                if ui.add(egui::DragValue::new(&mut outer_deg).speed(1.0).range(0.0..=90.0).suffix("°")).changed() {
                    light.outer_angle = outer_deg.to_radians();
                    changed = true;
                }
            });
        });
    });

    // Shadows
    property_row(ui, 5, |ui| {
        ui.horizontal(|ui| {
            ui.label("Shadows");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.checkbox(&mut light.shadows_enabled, "").changed() {
                    changed = true;
                }
            });
        });
    });

    changed
}
