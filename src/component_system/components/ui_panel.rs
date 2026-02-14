//! UI Panel component definition

use bevy::prelude::*;
use bevy_egui::egui;

use crate::component_system::{ComponentCategory, ComponentRegistry};
use crate::register_component;
use crate::component_system::UIPanelData;
use crate::ui::property_row;

use egui_phosphor::regular::SQUARES_FOUR;

// ============================================================================
// Custom Inspector
// ============================================================================

fn inspect_ui_panel(
    ui: &mut egui::Ui,
    world: &mut World,
    entity: Entity,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) -> bool {
    let Some(mut data) = world.get_mut::<UIPanelData>(entity) else {
        return false;
    };
    let mut changed = false;

    // Width
    property_row(ui, 0, |ui| {
        ui.horizontal(|ui| {
            ui.label("Width");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add(egui::DragValue::new(&mut data.width).speed(1.0))
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
                    .add(egui::DragValue::new(&mut data.height).speed(1.0))
                    .changed()
                {
                    changed = true;
                }
            });
        });
    });

    // Background Color
    property_row(ui, 2, |ui| {
        ui.horizontal(|ui| {
            ui.label("Background");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let mut color = egui::Color32::from_rgba_unmultiplied(
                    (data.background_color.x * 255.0) as u8,
                    (data.background_color.y * 255.0) as u8,
                    (data.background_color.z * 255.0) as u8,
                    (data.background_color.w * 255.0) as u8,
                );
                if ui.color_edit_button_srgba(&mut color).changed() {
                    data.background_color = Vec4::new(
                        color.r() as f32 / 255.0,
                        color.g() as f32 / 255.0,
                        color.b() as f32 / 255.0,
                        color.a() as f32 / 255.0,
                    );
                    changed = true;
                }
            });
        });
    });

    // Border Radius
    property_row(ui, 3, |ui| {
        ui.horizontal(|ui| {
            ui.label("Border Radius");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add(
                        egui::DragValue::new(&mut data.border_radius)
                            .speed(0.5)
                            .range(0.0..=50.0),
                    )
                    .changed()
                {
                    changed = true;
                }
            });
        });
    });

    // Padding
    property_row(ui, 4, |ui| {
        ui.horizontal(|ui| {
            ui.label("Padding");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add(
                        egui::DragValue::new(&mut data.padding)
                            .speed(0.5)
                            .range(0.0..=50.0),
                    )
                    .changed()
                {
                    changed = true;
                }
            });
        });
    });

    changed
}

// ============================================================================
// Registration
// ============================================================================

pub fn register(registry: &mut ComponentRegistry) {
    registry.register_owned(register_component!(UIPanelData {
        type_id: "ui_panel",
        display_name: "UI Panel",
        category: ComponentCategory::UI,
        icon: SQUARES_FOUR,
        priority: 0,
        conflicts_with: ["ui_label", "ui_button", "ui_image"],
        custom_inspector: inspect_ui_panel,
    }));
}
