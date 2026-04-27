#![allow(deprecated)] // egui API rename pending; will migrate at next bevy_egui bump.

//! Node Graph Particle Editor Panel — Niagara-style visual node editor.

use std::cell::RefCell;
use std::path::PathBuf;

use bevy::prelude::*;
use bevy_egui::egui::{self, RichText};
use egui_phosphor::regular::{
    FLOW_ARROW, PLUS, CROSSHAIR, MAGNIFYING_GLASS_PLUS, MAGNIFYING_GLASS_MINUS,
    SPARKLE,
};

use renzora_editor::{EditorCommands, EditorPanel, PanelLocation};
use renzora_theme::{Theme, ThemeManager};

use renzora_hanabi::{
    ParticleEditorState, load_effect_from_file,
    node_graph::{ParticleNodeGraph, ParticleNodeType},
};

use crate::graph_editor::{self, GraphEditorState};

thread_local! {
    static GRAPH_EDITOR_STATE: RefCell<GraphEditorState> = RefCell::new(GraphEditorState::default());
}

fn preset_icon(name: &str) -> &'static str {
    match name {
        "fire" => egui_phosphor::regular::FLAME,
        "firework" | "firework_rocket" => egui_phosphor::regular::STAR_FOUR,
        "rain" => egui_phosphor::regular::CLOUD_RAIN,
        "snow" => egui_phosphor::regular::SNOWFLAKE,
        _ => SPARKLE,
    }
}

fn capitalize_preset(s: &str) -> String {
    s.split('_')
        .map(|w| {
            let mut c = w.chars();
            match c.next() {
                None => String::new(),
                Some(f) => f.to_uppercase().to_string() + c.as_str(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

pub struct ParticleGraphPanel;

impl EditorPanel for ParticleGraphPanel {
    fn id(&self) -> &str {
        "particle_graph"
    }

    fn title(&self) -> &str {
        "Particle Graph"
    }

    fn icon(&self) -> Option<&str> {
        Some(FLOW_ARROW)
    }

    fn ui(&self, ui: &mut egui::Ui, world: &World) {
        let theme = match world.get_resource::<ThemeManager>() {
            Some(tm) => tm.active_theme.clone(),
            None => return,
        };
        let cmds = match world.get_resource::<EditorCommands>() {
            Some(c) => c,
            None => return,
        };
        let Some(editor_state) = world.get_resource::<ParticleEditorState>() else {
            return;
        };

        // If no effect is loaded, show a message
        if editor_state.current_effect.is_none() {
            let text_muted = theme.text.muted.to_color32();
            ui.vertical_centered(|ui| {
                ui.add_space(ui.available_height() * 0.35);
                ui.label(RichText::new("Particle Graph Editor").size(20.0).color(text_muted));
                ui.add_space(16.0);
                ui.label(RichText::new("Load an effect in the Particle Editor panel first").size(12.0).color(text_muted));
            });
            return;
        }

        let effect_name = editor_state.current_effect.as_ref()
            .map(|e| e.name.clone())
            .unwrap_or_else(|| "New Effect".to_string());

        // Initialize graph if needed (generate from current effect)
        let mut node_graph = editor_state.node_graph.clone()
            .unwrap_or_else(|| {
                if let Some(ref effect) = editor_state.current_effect {
                    ParticleNodeGraph::from_effect(effect)
                } else {
                    ParticleNodeGraph::new(&effect_name)
                }
            });

        // ── Toolbar ──
        let toolbar_modified = GRAPH_EDITOR_STATE.with(|cell| {
            let mut graph_state = cell.borrow_mut();
            render_toolbar(ui, &mut node_graph, &mut graph_state, cmds, &theme)
        });

        ui.separator();

        // ── Graph canvas ──
        let (graph_modified, selected) = GRAPH_EDITOR_STATE.with(|cell| {
            let mut graph_state = cell.borrow_mut();
            let m = graph_editor::render_graph_editor(ui, &mut node_graph, &mut graph_state, &theme);
            let sel = if graph_state.widget_state.selected.len() == 1 {
                Some(graph_state.widget_state.selected[0])
            } else {
                None
            };
            (m, sel)
        });

        // Always sync selected node to editor state
        let prev_selected = editor_state.selected_node;
        if selected != prev_selected {
            let sel = selected;
            cmds.push(move |world: &mut World| {
                world.resource_mut::<ParticleEditorState>().selected_node = sel;
            });
        }

        if graph_modified || toolbar_modified {
            cmds.push(move |world: &mut World| {
                let mut state = world.resource_mut::<ParticleEditorState>();
                state.node_graph = Some(node_graph);
                state.is_modified = true;
            });
        }
    }

    fn closable(&self) -> bool {
        true
    }

    fn default_location(&self) -> PanelLocation {
        PanelLocation::Center
    }
}

/// Render the toolbar above the graph canvas. Returns true if graph was modified.
fn render_toolbar(
    ui: &mut egui::Ui,
    graph: &mut ParticleNodeGraph,
    state: &mut GraphEditorState,
    cmds: &EditorCommands,
    theme: &Theme,
) -> bool {
    let mut modified = false;
    let text_color = theme.text.primary.to_color32();
    let text_muted = theme.text.muted.to_color32();
    let accent = theme.semantic.accent.to_color32();

    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        ui.style_mut().spacing.button_padding = egui::vec2(6.0, 2.0);

        // ── Presets dropdown ──
        let presets_btn = ui.add(egui::Button::new(
            RichText::new(format!("{} Presets", SPARKLE)).size(12.0).color(text_color),
        ));
        let presets_id = ui.make_persistent_id("presets_popup");
        if presets_btn.clicked() {
            ui.memory_mut(|m| m.toggle_popup(presets_id));
        }
        egui::popup_below_widget(ui, presets_id, &presets_btn, egui::PopupCloseBehavior::CloseOnClickOutside, |ui| {
            ui.set_min_width(160.0);
            let presets_dir = PathBuf::from("assets/particles");
            if presets_dir.exists() {
                let mut preset_files: Vec<PathBuf> = std::fs::read_dir(&presets_dir)
                    .into_iter()
                    .flatten()
                    .filter_map(|e| e.ok())
                    .map(|e| e.path())
                    .filter(|p| p.extension().is_some_and(|ext| ext == "particle"))
                    .collect();
                preset_files.sort();

                for preset_path in &preset_files {
                    let stem = preset_path.file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("unknown");
                    let icon = preset_icon(stem);
                    let display = capitalize_preset(stem);

                    if ui.button(format!("{} {}", icon, display)).clicked() {
                        let path = preset_path.clone();
                        cmds.push(move |world: &mut World| {
                            if let Some(effect) = load_effect_from_file(&path) {
                                let mut state = world.resource_mut::<ParticleEditorState>();
                                state.current_effect = Some(effect.clone());
                                state.current_file_path = Some(path.to_string_lossy().to_string());
                                state.is_modified = false;
                                state.node_graph = Some(ParticleNodeGraph::from_effect(&effect));
                            }
                        });
                        // Force resync
                        state.needs_sync = true;
                        ui.memory_mut(|m| m.toggle_popup(presets_id));
                    }
                }
            } else {
                ui.label(RichText::new("No presets found").size(11.0).color(text_muted));
            }
        });

        ui.separator();

        // ── Add Node dropdown ──
        let add_btn = ui.add(egui::Button::new(
            RichText::new(format!("{} Add Node", PLUS)).size(12.0).color(accent),
        ));
        let add_id = ui.make_persistent_id("add_node_popup");
        if add_btn.clicked() {
            ui.memory_mut(|m| m.toggle_popup(add_id));
        }
        egui::popup_below_widget(ui, add_id, &add_btn, egui::PopupCloseBehavior::CloseOnClickOutside, |ui| {
            ui.set_min_width(160.0);
            for &category in ParticleNodeType::categories() {
                let icon = graph_editor::category_icon(category);
                ui.menu_button(format!("{} {}", icon, category), |ui| {
                    let nodes = ParticleNodeType::nodes_in_category(category);
                    for node_type in &nodes {
                        if ui.button(format!("{} {}", icon, node_type.display_name())).clicked() {
                            // Place near center of current view
                            let canvas_pos = if let Some(cr) = state.canvas_rect {
                                let center = cr.center();
                                graph_editor::screen_to_canvas_pub(
                                    center,
                                    state.widget_state.offset,
                                    state.widget_state.zoom,
                                    cr,
                                )
                            } else {
                                [0.0, 0.0]
                            };
                            let new_id = graph.add_node(*node_type, canvas_pos);
                            let emitter_pin = match node_type.category() {
                                "Spawn" => Some("spawn"),
                                "Init" => Some("init"),
                                "Update" => Some("update"),
                                "Render" => Some("render"),
                                _ => None,
                            };
                            if let Some(pin) = emitter_pin {
                                if let Some(emitter) = graph.nodes.iter().find(|n| n.node_type == ParticleNodeType::Emitter) {
                                    let eid = emitter.id;
                                    graph.connect(new_id, "module", eid, pin);
                                }
                            }
                            state.needs_sync = true;
                            modified = true;
                            ui.memory_mut(|m| m.toggle_popup(add_id));
                        }
                    }
                });
            }
        });

        ui.separator();

        // ── Center Graph ──
        if ui.add(egui::Button::new(
            RichText::new(format!("{}", CROSSHAIR)).size(12.0).color(text_muted),
        )).on_hover_text("Center Graph").clicked() {
            // Reset offset to center all nodes
            if !graph.nodes.is_empty() {
                let mut min_x = f32::MAX;
                let mut max_x = f32::MIN;
                let mut min_y = f32::MAX;
                let mut max_y = f32::MIN;
                for node in &graph.nodes {
                    min_x = min_x.min(node.position[0]);
                    max_x = max_x.max(node.position[0]);
                    min_y = min_y.min(node.position[1]);
                    max_y = max_y.max(node.position[1]);
                }
                let center_x = (min_x + max_x) / 2.0;
                let center_y = (min_y + max_y) / 2.0;
                state.widget_state.offset = [-center_x, -center_y];
            } else {
                state.widget_state.offset = [0.0, 0.0];
            }
        }

        // ── Zoom controls ──
        if ui.add(egui::Button::new(
            RichText::new(format!("{}", MAGNIFYING_GLASS_MINUS)).size(12.0).color(text_muted),
        )).on_hover_text("Zoom Out").clicked() {
            state.widget_state.zoom = (state.widget_state.zoom * 0.8).max(0.25);
        }

        let zoom_pct = (state.widget_state.zoom * 100.0) as u32;
        ui.label(RichText::new(format!("{}%", zoom_pct)).size(11.0).color(text_muted));

        if ui.add(egui::Button::new(
            RichText::new(format!("{}", MAGNIFYING_GLASS_PLUS)).size(12.0).color(text_muted),
        )).on_hover_text("Zoom In").clicked() {
            state.widget_state.zoom = (state.widget_state.zoom * 1.25).min(4.0);
        }
    });

    modified
}
