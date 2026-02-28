//! UI Image component definition

use bevy::prelude::*;
use bevy_egui::egui;

use crate::component_system::{ComponentCategory, ComponentRegistry};
use crate::register_component;
use crate::component_system::UIImageData;
use crate::ui::property_row;

use egui_phosphor::regular::IMAGE;

// ============================================================================
// Custom Inspector
// ============================================================================

fn inspect_ui_image(
    ui: &mut egui::Ui,
    world: &mut World,
    entity: Entity,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) -> bool {
    let Some(mut data) = world.get_mut::<UIImageData>(entity) else {
        return false;
    };
    let mut changed = false;

    // Texture Path
    property_row(ui, 0, |ui| {
        ui.horizontal(|ui| {
            ui.label(&crate::locale::t("comp.ui_image.texture"));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.text_edit_singleline(&mut data.texture_path).changed() {
                    changed = true;
                }
            });
        });
    });

    // Width
    property_row(ui, 1, |ui| {
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
    property_row(ui, 2, |ui| {
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

    // Tint
    property_row(ui, 3, |ui| {
        ui.horizontal(|ui| {
            ui.label(&crate::locale::t("comp.ui_image.tint"));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let mut color = egui::Color32::from_rgba_unmultiplied(
                    (data.tint.x * 255.0) as u8,
                    (data.tint.y * 255.0) as u8,
                    (data.tint.z * 255.0) as u8,
                    (data.tint.w * 255.0) as u8,
                );
                if ui.color_edit_button_srgba(&mut color).changed() {
                    data.tint = Vec4::new(
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

    changed
}

// ============================================================================
// Registration
// ============================================================================

pub fn register(registry: &mut ComponentRegistry) {
    registry.register_owned(register_component!(UIImageData {
        type_id: "ui_image",
        display_name: "UI Image",
        category: ComponentCategory::UI,
        icon: IMAGE,
        priority: 3,
        conflicts_with: ["ui_panel", "ui_label", "ui_button"],
        custom_inspector: inspect_ui_image,
    }));
}
