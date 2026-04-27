//! Palette panel — persistent vertical dock with "Add Entity" and "Add
//! Component" sections, grouped by category. Replaces hunting through the
//! context menu for frequent spawns.

use std::sync::{Mutex, RwLock};

use bevy::prelude::*;
use bevy_egui::egui::{self, Color32, RichText};
use egui_phosphor::regular;
use renzora_editor::{
    EditorPanel, EditorSelection, InspectorRegistry, PanelLocation, SpawnRegistry,
};
use renzora_theme::ThemeManager;

#[derive(Clone, Copy, PartialEq, Eq)]
enum PaletteTab {
    Entities,
    Components,
}

struct PaletteState {
    tab: PaletteTab,
    query: String,
}

impl Default for PaletteState {
    fn default() -> Self {
        Self {
            tab: PaletteTab::Entities,
            query: String::new(),
        }
    }
}

enum PendingPaletteAction {
    SpawnEntity(&'static str),
    AddComponent(&'static str),
}

pub struct PalettePanel {
    state: RwLock<PaletteState>,
}

impl PalettePanel {
    pub fn new() -> Self {
        Self {
            state: RwLock::new(PaletteState::default()),
        }
    }
}

impl EditorPanel for PalettePanel {
    fn id(&self) -> &str { "palette" }
    fn title(&self) -> &str { "Palette" }
    fn icon(&self) -> Option<&str> { Some(regular::SQUARES_FOUR) }
    fn default_location(&self) -> PanelLocation { PanelLocation::Left }
    fn min_size(&self) -> [f32; 2] { [200.0, 200.0] }

    fn ui(&self, ui: &mut egui::Ui, world: &World) {
        let theme = world.get_resource::<ThemeManager>().map(|m| m.active_theme.clone());
        let (text_primary, text_muted, hover, header_bg) = theme
            .as_ref()
            .map(|t| (
                t.text.primary.to_color32(),
                t.text.muted.to_color32(),
                t.widgets.hovered_bg.to_color32(),
                t.surfaces.panel.to_color32(),
            ))
            .unwrap_or((
                Color32::WHITE,
                Color32::from_gray(140),
                Color32::from_rgb(55, 55, 65),
                Color32::from_rgb(40, 40, 45),
            ));

        let Ok(mut state) = self.state.write() else { return };

        // Tab bar
        ui.horizontal(|ui| {
            let entities_label = if state.tab == PaletteTab::Entities {
                RichText::new(format!("{} Entities", regular::CUBE)).color(text_primary).strong()
            } else {
                RichText::new(format!("{} Entities", regular::CUBE)).color(text_muted)
            };
            if ui.selectable_label(state.tab == PaletteTab::Entities, entities_label).clicked() {
                state.tab = PaletteTab::Entities;
                state.query.clear();
            }

            let components_label = if state.tab == PaletteTab::Components {
                RichText::new(format!("{} Components", regular::PUZZLE_PIECE)).color(text_primary).strong()
            } else {
                RichText::new(format!("{} Components", regular::PUZZLE_PIECE)).color(text_muted)
            };
            if ui.selectable_label(state.tab == PaletteTab::Components, components_label).clicked() {
                state.tab = PaletteTab::Components;
                state.query.clear();
            }
        });

        ui.separator();

        // Search
        let edit = egui::TextEdit::singleline(&mut state.query)
            .hint_text("Search...")
            .text_color(text_primary)
            .font(egui::FontId::proportional(13.0))
            .desired_width(f32::INFINITY);
        ui.add(edit);
        ui.add_space(4.0);

        let q = state.query.to_lowercase();

        match state.tab {
            PaletteTab::Entities => {
                self.render_entities(ui, world, &q, text_primary, text_muted, hover, header_bg);
            }
            PaletteTab::Components => {
                self.render_components(ui, world, &q, text_primary, text_muted, hover, header_bg);
            }
        }
    }
}

impl PalettePanel {
    fn render_entities(
        &self,
        ui: &mut egui::Ui,
        world: &World,
        q: &str,
        text_primary: Color32,
        text_muted: Color32,
        hover: Color32,
        header_bg: Color32,
    ) {
        let Some(registry) = world.get_resource::<SpawnRegistry>() else {
            ui.label(RichText::new("No SpawnRegistry").color(text_muted));
            return;
        };

        let mut groups: Vec<(&str, Vec<&renzora_editor::EntityPreset>)> = Vec::new();
        for preset in registry.iter() {
            let matches = q.is_empty()
                || preset.display_name.to_lowercase().contains(q)
                || preset.category.to_lowercase().contains(q);
            if !matches { continue; }

            if let Some(bucket) = groups.iter_mut().find(|(c, _)| *c == preset.category) {
                bucket.1.push(preset);
            } else {
                groups.push((preset.category, vec![preset]));
            }
        }

        if groups.is_empty() {
            ui.label(RichText::new("No matches").color(text_muted));
            return;
        }

        egui::ScrollArea::vertical().auto_shrink([false, true]).show(ui, |ui| {
            for (category, entries) in &groups {
                // Category header
                let header_rect = ui.horizontal(|ui| {
                    ui.label(RichText::new(*category).color(text_muted).size(11.0).strong());
                }).response.rect;
                ui.painter().rect_filled(
                    header_rect.expand2(egui::vec2(4.0, 1.0)),
                    egui::CornerRadius::same(2),
                    header_bg.linear_multiply(0.3),
                );

                for preset in entries {
                    let w = ui.available_width();
                    let (rect, resp) = ui.allocate_exact_size(
                        egui::Vec2::new(w, 22.0),
                        egui::Sense::click(),
                    );
                    if resp.hovered() {
                        ui.painter().rect_filled(rect, egui::CornerRadius::same(3), hover);
                    }
                    ui.painter().text(
                        rect.left_center() + egui::Vec2::new(8.0, 0.0),
                        egui::Align2::LEFT_CENTER,
                        format!("{}  {}", preset.icon, preset.display_name),
                        egui::FontId::proportional(13.0),
                        text_primary,
                    );
                    if resp.clicked() {
                        if let Some(q) = world.get_resource::<PaletteActionQueue>() {
                            q.0.lock().unwrap().push(
                                PendingPaletteAction::SpawnEntity(preset.id),
                            );
                        }
                    }
                }
                ui.add_space(2.0);
            }
        });
    }

    fn render_components(
        &self,
        ui: &mut egui::Ui,
        world: &World,
        q: &str,
        text_primary: Color32,
        text_muted: Color32,
        hover: Color32,
        header_bg: Color32,
    ) {
        let Some(registry) = world.get_resource::<InspectorRegistry>() else {
            ui.label(RichText::new("No InspectorRegistry").color(text_muted));
            return;
        };

        let selection = world.get_resource::<EditorSelection>();
        let targets = selection.map(|s| s.get_all()).unwrap_or_default();

        let mut groups: Vec<(&str, Vec<&renzora_editor::InspectorEntry>)> = Vec::new();
        for entry in registry.iter() {
            if entry.add_fn.is_none() { continue; }
            // Hide if ALL selected entities already have it
            if !targets.is_empty() && targets.iter().all(|e| (entry.has_fn)(world, *e)) {
                continue;
            }
            let matches = q.is_empty()
                || entry.display_name.to_lowercase().contains(q)
                || entry.category.to_lowercase().contains(q);
            if !matches { continue; }

            if let Some(bucket) = groups.iter_mut().find(|(c, _)| *c == entry.category) {
                bucket.1.push(entry);
            } else {
                groups.push((entry.category, vec![entry]));
            }
        }

        if groups.is_empty() {
            if targets.is_empty() {
                ui.label(RichText::new("Select an entity to add components").color(text_muted));
            } else {
                ui.label(RichText::new("No matches").color(text_muted));
            }
            return;
        }

        let count_label = if targets.len() > 1 {
            format!("Adding to {} entities", targets.len())
        } else {
            String::new()
        };
        if !count_label.is_empty() {
            ui.label(RichText::new(count_label).color(text_muted).size(11.0));
            ui.add_space(2.0);
        }

        egui::ScrollArea::vertical().auto_shrink([false, true]).show(ui, |ui| {
            for (category, entries) in &groups {
                let header_rect = ui.horizontal(|ui| {
                    ui.label(RichText::new(*category).color(text_muted).size(11.0).strong());
                }).response.rect;
                ui.painter().rect_filled(
                    header_rect.expand2(egui::vec2(4.0, 1.0)),
                    egui::CornerRadius::same(2),
                    header_bg.linear_multiply(0.3),
                );

                for entry in entries {
                    let w = ui.available_width();
                    let (rect, resp) = ui.allocate_exact_size(
                        egui::Vec2::new(w, 22.0),
                        egui::Sense::click(),
                    );
                    if resp.hovered() {
                        ui.painter().rect_filled(rect, egui::CornerRadius::same(3), hover);
                    }
                    ui.painter().text(
                        rect.left_center() + egui::Vec2::new(8.0, 0.0),
                        egui::Align2::LEFT_CENTER,
                        format!("{}  {}", entry.icon, entry.display_name),
                        egui::FontId::proportional(13.0),
                        text_primary,
                    );
                    if resp.clicked() {
                        if let Some(q) = world.get_resource::<PaletteActionQueue>() {
                            q.0.lock().unwrap().push(
                                PendingPaletteAction::AddComponent(entry.type_id),
                            );
                        }
                    }
                }
                ui.add_space(2.0);
            }
        });
    }
}

/// Drain pending palette actions each frame.
pub fn drain_palette_actions(world: &mut World) {
    let actions: Vec<PendingPaletteAction> = {
        let Some(queue) = world.get_resource::<PaletteActionQueue>() else { return };
        let mut lock = queue.0.lock().unwrap();
        std::mem::take(&mut *lock)
    };

    for action in actions {
        match action {
            PendingPaletteAction::SpawnEntity(id) => {
                super::spawn_preset_at(world, id, bevy::prelude::Vec3::ZERO);
            }
            PendingPaletteAction::AddComponent(type_id) => {
                super::add_component_to_selection(world, type_id);
            }
        }
    }
}

/// Shared action queue resource — the panel pushes, the system drains.
#[derive(Resource, Default)]
pub struct PaletteActionQueue(pub Mutex<Vec<PendingPaletteAction>>);
