//! Meshlet mesh (virtual geometry) component definition

use bevy::prelude::*;
use bevy_egui::egui;

use crate::component_system::{ComponentCategory, ComponentRegistry};
use crate::register_component;
use crate::shared::MeshletMeshData;
use crate::ui::property_row;

use egui_phosphor::regular::POLYGON;

// ============================================================================
// Custom Inspector
// ============================================================================

fn inspect_meshlet_mesh(
    ui: &mut egui::Ui,
    world: &mut World,
    entity: Entity,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) -> bool {
    let mut changed = false;
    let solari_available = cfg!(feature = "solari");

    // Get current meshlet data
    let Some(mut meshlet_data) = world.get_mut::<MeshletMeshData>(entity) else {
        return false;
    };

    // Enabled toggle (disabled if solari feature not compiled)
    property_row(ui, 0, |ui| {
        ui.horizontal(|ui| {
            ui.label("Enabled");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.add_enabled(solari_available, egui::Checkbox::new(&mut meshlet_data.enabled, "")).changed() {
                    changed = true;
                }
            });
        });
    });

    if !solari_available {
        ui.add_space(4.0);
        ui.label(
            egui::RichText::new(
                "Meshlet feature is not enabled. Rebuild with --features solari \
                 (requires Vulkan SDK + DLSS SDK).",
            )
            .color(egui::Color32::from_rgb(200, 160, 60))
            .small(),
        );
        return changed;
    }

    // Asset path
    property_row(ui, 1, |ui| {
        ui.horizontal(|ui| {
            ui.label("Asset Path");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let response = ui.add(
                    egui::TextEdit::singleline(&mut meshlet_data.meshlet_path)
                        .hint_text("path/to/mesh.meshlet")
                        .desired_width(120.0)
                );
                if response.changed() {
                    changed = true;
                }
            });
        });
    });

    // LOD bias slider
    property_row(ui, 2, |ui| {
        ui.horizontal(|ui| {
            ui.label("LOD Bias");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let response = ui.add(
                    egui::Slider::new(&mut meshlet_data.lod_bias, -1.0..=1.0)
                        .show_value(true)
                );
                if response.changed() {
                    changed = true;
                }
            });
        });
    });

    // Status indicator
    let status_color = if meshlet_data.meshlet_path.is_empty() {
        egui::Color32::from_rgb(180, 140, 60) // Warning
    } else if meshlet_data.enabled {
        egui::Color32::from_rgb(80, 180, 100) // Success
    } else {
        egui::Color32::from_rgb(140, 140, 150) // Disabled
    };

    let status_text = if meshlet_data.meshlet_path.is_empty() {
        "No asset assigned"
    } else if meshlet_data.enabled {
        "Meshlet rendering enabled"
    } else {
        "Meshlet rendering disabled"
    };

    property_row(ui, 3, |ui| {
        ui.horizontal(|ui| {
            ui.label("Status");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(egui::RichText::new(status_text).color(status_color));
            });
        });
    });

    // Info text
    ui.add_space(4.0);
    ui.label(
        egui::RichText::new("Virtual geometry with GPU-driven LOD")
            .small()
            .color(egui::Color32::from_rgb(120, 120, 130)),
    );

    changed
}

// ============================================================================
// Registration
// ============================================================================

pub fn register(registry: &mut ComponentRegistry) {
    registry.register_owned(register_component!(MeshletMeshData {
        type_id: "meshlet_mesh",
        display_name: "Meshlet Mesh",
        category: ComponentCategory::Rendering,
        icon: POLYGON,
        priority: 3,
        conflicts_with: ["mesh_renderer"],
        custom_inspector: inspect_meshlet_mesh,
    }));
}
