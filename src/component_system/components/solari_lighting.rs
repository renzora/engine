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

    // Enabled toggle
    changed |= inline_property(ui, row, "Enabled", |ui| {
        ui.checkbox(&mut data.enabled, "").changed()
    });
    row += 1;

    if !data.enabled {
        return changed;
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
