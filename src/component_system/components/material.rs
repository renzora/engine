//! Material (blueprint) component definition

use bevy::prelude::*;
use bevy_egui::egui;

use crate::component_system::{ComponentCategory, ComponentRegistry};
use crate::register_component;
use crate::shared::MaterialData;
use crate::input::MaterialApplied;
use crate::ui::property_row;

use egui_phosphor::regular::PALETTE;

// ============================================================================
// Custom Remove
// ============================================================================

fn remove_material(commands: &mut Commands, entity: Entity) {
    commands
        .entity(entity)
        .remove::<MaterialData>()
        .remove::<MaterialApplied>();
}

// ============================================================================
// Custom Inspector
// ============================================================================

fn inspect_material(
    ui: &mut egui::Ui,
    world: &mut World,
    entity: Entity,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) -> bool {
    let mut changed = false;

    // Get current material path
    let current_path = world
        .get::<MaterialData>(entity)
        .and_then(|d| d.material_path.clone());

    let display_path = current_path
        .as_ref()
        .map(|p| {
            // Show just the filename for brevity
            std::path::Path::new(p)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(p)
        })
        .unwrap_or("None");

    property_row(ui, 0, |ui| {
        ui.horizontal(|ui| {
            ui.label("Blueprint");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                // Show the current material path (read-only for now, drag-drop to change)
                ui.label(display_path);
            });
        });
    });

    // Show applied status
    let is_applied = world.get::<MaterialApplied>(entity).is_some();
    property_row(ui, 1, |ui| {
        ui.horizontal(|ui| {
            ui.label("Status");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if current_path.is_some() {
                    if is_applied {
                        ui.colored_label(egui::Color32::from_rgb(100, 200, 100), "Applied");
                    } else {
                        ui.colored_label(egui::Color32::from_rgb(200, 200, 100), "Pending...");
                    }
                } else {
                    ui.colored_label(egui::Color32::from_rgb(150, 150, 150), "No material");
                }
            });
        });
    });

    // Clear button
    if current_path.is_some() {
        property_row(ui, 2, |ui| {
            ui.horizontal(|ui| {
                if ui.button("Clear Material").clicked() {
                    if let Some(mut data) = world.get_mut::<MaterialData>(entity) {
                        data.material_path = None;
                    }
                    // Remove the applied marker so the system knows to update
                    // (This needs commands, but we're in world access - mark changed and handle elsewhere)
                    changed = true;
                }
            });
        });
    }

    // Hint text
    ui.add_space(4.0);
    ui.label(
        egui::RichText::new("Drag a .material_bp file here to apply")
            .small()
            .color(egui::Color32::from_rgb(120, 120, 130)),
    );

    changed
}

// ============================================================================
// Registration
// ============================================================================

pub fn register(registry: &mut ComponentRegistry) {
    registry.register_owned(register_component!(MaterialData {
        type_id: "material",
        display_name: "Material",
        category: ComponentCategory::Rendering,
        icon: PALETTE,
        priority: 2,
        custom_inspector: inspect_material,
        custom_remove: remove_material,
    }));
}
