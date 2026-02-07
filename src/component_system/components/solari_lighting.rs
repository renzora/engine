//! Solari lighting component definition

use bevy::prelude::*;
use bevy_egui::egui;

use crate::component_system::{ComponentCategory, ComponentRegistry};
use crate::register_component;
use crate::shared::{DlssQualityMode, SolariLightingData};
use crate::ui::{inline_property, property_row};

use egui_phosphor::regular::SPARKLE;

// ============================================================================
// Custom Inspector
// ============================================================================

fn inspect_solari_lighting(
    ui: &mut egui::Ui,
    world: &mut World,
    entity: Entity,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) -> bool {
    let Some(mut data) = world.get_mut::<SolariLightingData>(entity) else {
        return false;
    };
    let mut changed = false;
    let mut row = 0;

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
                    egui::RichText::new("Solari feature is not enabled")
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

    // DLSS toggle
    {
        property_row(ui, row, |ui| {
            ui.horizontal(|ui| {
                ui.label("DLSS Ray Reconstruction");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.checkbox(&mut data.dlss_enabled, "").changed() {
                        changed = true;
                    }
                });
            });
        });
        row += 1;

        // DLSS Quality (only show if DLSS is enabled)
        if data.dlss_enabled {
            property_row(ui, row, |ui| {
                ui.horizontal(|ui| {
                    ui.label("DLSS Quality");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        egui::ComboBox::from_id_salt("dlss_quality")
                            .selected_text(match data.dlss_quality {
                                DlssQualityMode::Auto => "Auto",
                                DlssQualityMode::Dlaa => "DLAA",
                                DlssQualityMode::Quality => "Quality",
                                DlssQualityMode::Balanced => "Balanced",
                                DlssQualityMode::Performance => "Performance",
                                DlssQualityMode::UltraPerformance => "Ultra Performance",
                            })
                            .show_ui(ui, |ui| {
                                if ui
                                    .selectable_value(
                                        &mut data.dlss_quality,
                                        DlssQualityMode::Auto,
                                        "Auto",
                                    )
                                    .changed()
                                {
                                    changed = true;
                                }
                                if ui
                                    .selectable_value(
                                        &mut data.dlss_quality,
                                        DlssQualityMode::Dlaa,
                                        "DLAA",
                                    )
                                    .changed()
                                {
                                    changed = true;
                                }
                                if ui
                                    .selectable_value(
                                        &mut data.dlss_quality,
                                        DlssQualityMode::Quality,
                                        "Quality",
                                    )
                                    .changed()
                                {
                                    changed = true;
                                }
                                if ui
                                    .selectable_value(
                                        &mut data.dlss_quality,
                                        DlssQualityMode::Balanced,
                                        "Balanced",
                                    )
                                    .changed()
                                {
                                    changed = true;
                                }
                                if ui
                                    .selectable_value(
                                        &mut data.dlss_quality,
                                        DlssQualityMode::Performance,
                                        "Performance",
                                    )
                                    .changed()
                                {
                                    changed = true;
                                }
                                if ui
                                    .selectable_value(
                                        &mut data.dlss_quality,
                                        DlssQualityMode::UltraPerformance,
                                        "Ultra Performance",
                                    )
                                    .changed()
                                {
                                    changed = true;
                                }
                            });
                    });
                });
            });
        }
    }

    // Info text
    ui.add_space(8.0);
    ui.label(
        egui::RichText::new(
            "Solari provides real-time raytraced global illumination. \
             DLSS Ray Reconstruction improves quality on NVIDIA RTX GPUs.",
        )
        .weak()
        .small(),
    );

    changed
}

// ============================================================================
// Registration
// ============================================================================

pub fn register(registry: &mut ComponentRegistry) {
    registry.register_owned(register_component!(SolariLightingData {
        type_id: "solari_lighting",
        display_name: "Solari Lighting",
        category: ComponentCategory::Lighting,
        icon: SPARKLE,
        priority: 3,
        custom_inspector: inspect_solari_lighting,
    }));
}
