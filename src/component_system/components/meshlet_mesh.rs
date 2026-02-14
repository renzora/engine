//! Meshlet mesh (virtual geometry) component definition

use bevy::prelude::*;
use bevy_egui::egui;

use crate::component_system::{ComponentCategory, ComponentRegistry};
use crate::register_component;
use crate::component_system::MeshletMeshData;
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

    if !cfg!(feature = "solari") {
        let warning_color = egui::Color32::from_rgb(200, 160, 60);
        let bg_color = egui::Color32::from_rgb(50, 42, 15);
        let frame = egui::Frame::new()
            .inner_margin(8.0)
            .corner_radius(4.0)
            .fill(bg_color)
            .stroke(egui::Stroke::new(1.0, warning_color));
        frame.show(ui, |ui| {
            ui.vertical_centered(|ui| {
                ui.label(
                    egui::RichText::new("Meshlet feature is not enabled")
                        .color(warning_color)
                        .small(),
                );
                ui.label(
                    egui::RichText::new("Rebuild with --features solari")
                        .color(warning_color)
                        .small(),
                );
            });
        });
        return false;
    }

    // Get current meshlet data
    let Some(mut meshlet_data) = world.get_mut::<MeshletMeshData>(entity) else {
        return false;
    };

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
