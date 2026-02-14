//! UI Button component definition

use bevy::prelude::*;
use bevy_egui::egui;

use crate::component_system::{ComponentCategory, ComponentRegistry};
use crate::register_component;
use crate::component_system::UIButtonData;
use crate::ui::property_row;

use egui_phosphor::regular::CURSOR_CLICK;

// ============================================================================
// Custom Inspector
// ============================================================================

fn inspect_ui_button(
    ui: &mut egui::Ui,
    world: &mut World,
    entity: Entity,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) -> bool {
    let Some(mut data) = world.get_mut::<UIButtonData>(entity) else {
        return false;
    };
    let mut changed = false;

    // Text
    property_row(ui, 0, |ui| {
        ui.horizontal(|ui| {
            ui.label("Text");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.text_edit_singleline(&mut data.text).changed() {
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

    // Font Size
    property_row(ui, 3, |ui| {
        ui.horizontal(|ui| {
            ui.label("Font Size");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add(
                        egui::DragValue::new(&mut data.font_size)
                            .speed(0.5)
                            .range(8.0..=72.0),
                    )
                    .changed()
                {
                    changed = true;
                }
            });
        });
    });

    // Text Color
    property_row(ui, 4, |ui| {
        ui.horizontal(|ui| {
            ui.label("Text Color");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let mut color = egui::Color32::from_rgba_unmultiplied(
                    (data.text_color.x * 255.0) as u8,
                    (data.text_color.y * 255.0) as u8,
                    (data.text_color.z * 255.0) as u8,
                    (data.text_color.w * 255.0) as u8,
                );
                if ui.color_edit_button_srgba(&mut color).changed() {
                    data.text_color = Vec4::new(
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
    registry.register_owned(register_component!(UIButtonData {
        type_id: "ui_button",
        display_name: "UI Button",
        category: ComponentCategory::UI,
        icon: CURSOR_CLICK,
        priority: 2,
        conflicts_with: ["ui_panel", "ui_label", "ui_image"],
        custom_inspector: inspect_ui_button,
    }));
}
