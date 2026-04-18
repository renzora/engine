#![allow(dead_code)] // WIP file — many helpers staged for future panel layouts.

//! Foliage painting panel — type palette, brush tools, settings.

use std::sync::RwLock;

use bevy::prelude::*;
use bevy_egui::egui::{self, RichText};
use egui_phosphor::regular::{TREE, PAINT_BRUSH, ERASER, PLUS, TRASH};

use renzora_theme::ThemeManager;
use renzora_editor_framework::EditorPanel;
use renzora_terrain::foliage::{FoliageConfig, FoliageDensityMap, FoliageType};
use renzora_terrain::data::TerrainChunkData;
use renzora_editor_framework::EditorCommands;

use renzora_terrain::foliage::{FoliageBrushType, FoliagePaintSettings};
use crate::systems::FoliageToolState;

struct PanelState {
    section_types: bool,
    section_brush: bool,
    section_properties: bool,
}

impl Default for PanelState {
    fn default() -> Self {
        Self {
            section_types: true,
            section_brush: true,
            section_properties: true,
        }
    }
}

pub struct FoliagePanel {
    state: RwLock<PanelState>,
}

impl FoliagePanel {
    pub fn new() -> Self {
        Self {
            state: RwLock::new(PanelState::default()),
        }
    }
}

impl EditorPanel for FoliagePanel {
    fn id(&self) -> &str {
        "foliage_painting"
    }

    fn title(&self) -> &str {
        "Foliage"
    }

    fn icon(&self) -> Option<&str> {
        Some(TREE)
    }

    fn default_location(&self) -> renzora_editor_framework::PanelLocation {
        renzora_editor_framework::PanelLocation::Left
    }

    fn ui(&self, ui: &mut egui::Ui, world: &World) {
        let theme = match world.get_resource::<ThemeManager>() {
            Some(tm) => tm.active_theme.clone(),
            None => return,
        };
        let text_primary = theme.text.primary.to_color32();
        let text_secondary = theme.text.secondary.to_color32();

        let mut state = self.state.write().unwrap();

        // Get mutable access to resources via EditorCommands
        let Some(config) = world.get_resource::<FoliageConfig>() else { return };
        let settings = world.get_resource::<FoliagePaintSettings>().cloned().unwrap_or_default();
        let tool_state = world.get_resource::<FoliageToolState>().copied().unwrap_or_default();

        let mut config_clone = config.clone();
        let mut settings_mut = settings.clone();
        let mut tool_mut = tool_state;

        // ── Enable toggle ──────────────────────────────────────────────
        ui.horizontal(|ui| {
            ui.checkbox(&mut tool_mut.active, "");
            ui.label(
                RichText::new(format!("{} Foliage Painting", TREE))
                    .color(text_primary)
                    .size(14.0),
            );
        });
        ui.add_space(4.0);

        if !tool_mut.active {
            ui.label(
                RichText::new("Enable to paint foliage on terrain.")
                    .color(text_secondary)
                    .size(11.0),
            );

            // Write back
            if tool_mut != tool_state {
                let tool_val = tool_mut;
                world.get_resource::<EditorCommands>().map(|cmds| {
                    cmds.push(move |w: &mut World| {
                        w.insert_resource(tool_val);
                    });
                });
            }
            return;
        }

        // ── Foliage Types ──────────────────────────────────────────────
        if collapsible(ui, "Foliage Types", &mut state.section_types, text_primary) {
            ui.indent("foliage_types", |ui| {
                for (i, ft) in config_clone.types.iter().enumerate() {
                    let selected = i == settings_mut.active_type;
                    let label = format!("{}. {}", i + 1, ft.name);
                    if ui
                        .selectable_label(selected, RichText::new(&label).color(text_primary))
                        .clicked()
                    {
                        settings_mut.active_type = i;
                    }
                }

                ui.horizontal(|ui| {
                    if ui.button(RichText::new(format!("{} Add", PLUS)).color(text_primary)).clicked() {
                        config_clone.types.push(FoliageType::default());
                    }
                    if config_clone.types.len() > 1 {
                        if ui.button(RichText::new(format!("{} Remove", TRASH)).color(text_primary)).clicked() {
                            let idx = settings_mut.active_type.min(config_clone.types.len().saturating_sub(1));
                            config_clone.types.remove(idx);
                            if settings_mut.active_type >= config_clone.types.len() {
                                settings_mut.active_type = config_clone.types.len().saturating_sub(1);
                            }
                        }
                    }
                });
            });
            ui.add_space(4.0);
        }

        // ── Brush Tools ────────────────────────────────────────────────
        if collapsible(ui, "Brush", &mut state.section_brush, text_primary) {
            ui.indent("foliage_brush", |ui| {
                ui.horizontal(|ui| {
                    if ui
                        .selectable_label(
                            settings_mut.brush_type == FoliageBrushType::Paint,
                            RichText::new(PAINT_BRUSH).size(18.0),
                        )
                        .clicked()
                    {
                        settings_mut.brush_type = FoliageBrushType::Paint;
                    }
                    if ui
                        .selectable_label(
                            settings_mut.brush_type == FoliageBrushType::Erase,
                            RichText::new(ERASER).size(18.0),
                        )
                        .clicked()
                    {
                        settings_mut.brush_type = FoliageBrushType::Erase;
                    }
                });

                ui.add_space(4.0);
                ui.label(RichText::new("Size").color(text_secondary).size(11.0));
                ui.add(egui::Slider::new(&mut settings_mut.brush_radius, 0.01..=0.5));
                ui.label(RichText::new("Strength").color(text_secondary).size(11.0));
                ui.add(egui::Slider::new(&mut settings_mut.brush_strength, 0.01..=1.0));
                ui.label(RichText::new("Falloff").color(text_secondary).size(11.0));
                ui.add(egui::Slider::new(&mut settings_mut.brush_falloff, 0.0..=1.0));
            });
            ui.add_space(4.0);
        }

        // ── Selected Type Properties ───────────────────────────────────
        if collapsible(ui, "Properties", &mut state.section_properties, text_primary) {
            if let Some(ft) = config_clone.types.get_mut(settings_mut.active_type) {
                ui.indent("foliage_props", |ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Name").color(text_secondary).size(11.0));
                        ui.text_edit_singleline(&mut ft.name);
                    });

                    ui.label(RichText::new("Density").color(text_secondary).size(11.0));
                    ui.add(egui::Slider::new(&mut ft.density, 1.0..=50.0));

                    ui.label(RichText::new("Height Range").color(text_secondary).size(11.0));
                    ui.horizontal(|ui| {
                        ui.add(egui::DragValue::new(&mut ft.height_range.x).speed(0.01).range(0.01..=2.0).prefix("min "));
                        ui.add(egui::DragValue::new(&mut ft.height_range.y).speed(0.01).range(0.01..=2.0).prefix("max "));
                    });

                    ui.label(RichText::new("Wind Strength").color(text_secondary).size(11.0));
                    ui.add(egui::Slider::new(&mut ft.wind_strength, 0.0..=2.0));

                    ui.checkbox(&mut ft.enabled, "Enabled");
                });
            }
            ui.add_space(4.0);
        }

        // ── Write back changes ─────────────────────────────────────────
        let config_changed = config_clone.types != config.types;
        let settings_changed = settings_mut.active_type != settings.active_type
            || settings_mut.brush_type != settings.brush_type
            || settings_mut.brush_radius != settings.brush_radius
            || settings_mut.brush_strength != settings.brush_strength
            || settings_mut.brush_falloff != settings.brush_falloff;

        if let Some(cmds) = world.get_resource::<EditorCommands>() {
            if tool_mut != tool_state {
                let val = tool_mut;
                cmds.push(move |w: &mut World| { w.insert_resource(val); });
            }
            if settings_changed {
                let val = settings_mut;
                cmds.push(move |w: &mut World| { w.insert_resource(val); });
            }
            if config_changed {
                let val = config_clone;
                cmds.push(move |w: &mut World| { w.insert_resource(val); });
            }
            // Auto-add FoliageDensityMap to terrain chunks that don't have one
            if tool_mut.active {
                cmds.push(|w: &mut World| {
                    let entities: Vec<Entity> = w
                        .query_filtered::<Entity, (With<TerrainChunkData>, Without<FoliageDensityMap>)>()
                        .iter(w)
                        .collect();
                    for entity in entities {
                        w.entity_mut(entity).insert(FoliageDensityMap::new(64));
                    }
                });
            }
        }
    }
}

fn collapsible(
    ui: &mut egui::Ui,
    label: &str,
    open: &mut bool,
    color: egui::Color32,
) -> bool {
    let resp = ui.horizontal(|ui| {
        let arrow = if *open { egui_phosphor::regular::CARET_DOWN } else { egui_phosphor::regular::CARET_RIGHT };
        let btn = ui.button(RichText::new(arrow).color(color).size(12.0));
        ui.label(RichText::new(label).color(color).size(12.0).strong());
        btn.clicked()
    });
    if resp.inner {
        *open = !*open;
    }
    *open
}
