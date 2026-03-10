//! Material Graph Panel — the main editor panel with toolbar + node graph canvas.

use std::cell::RefCell;

use bevy::prelude::*;
use bevy_egui::egui::{self, RichText};
use egui_phosphor::regular::{
    FLOPPY_DISK, FOLDER_OPEN, PLUS, CROSSHAIR,
    MAGNIFYING_GLASS_PLUS, MAGNIFYING_GLASS_MINUS,
    FLOW_ARROW, FILE_PLUS, CUBE,
};

use renzora_editor::{EditorCommands, EditorPanel, PanelLocation};
use renzora_theme::{Theme, ThemeManager};
use renzora_material::graph::{MaterialDomain, MaterialGraph};
use renzora_material::codegen;

use crate::MaterialEditorState;
use crate::graph_editor::{self, GraphEditorState};

thread_local! {
    static GRAPH_EDITOR_STATE: RefCell<GraphEditorState> = RefCell::new(GraphEditorState::default());
}

pub struct MaterialGraphPanel;

impl EditorPanel for MaterialGraphPanel {
    fn id(&self) -> &str {
        "material_graph"
    }

    fn title(&self) -> &str {
        "Material Graph"
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
        let Some(editor_state) = world.get_resource::<MaterialEditorState>() else {
            return;
        };

        // Get thumbnail map for texture node previews
        let thumbnail_map = world
            .get_resource::<crate::thumbnails::NodeThumbnails>()
            .map(|t| t.to_map())
            .unwrap_or_default();

        let mut graph = editor_state.graph.clone();

        // ── Toolbar ──
        let toolbar_modified = GRAPH_EDITOR_STATE.with(|cell| {
            let mut gs = cell.borrow_mut();
            render_toolbar(ui, &mut graph, &mut gs, cmds, editor_state, &theme)
        });

        ui.separator();

        // ── Graph canvas ──
        let (graph_modified, selected) = GRAPH_EDITOR_STATE.with(|cell| {
            let mut gs = cell.borrow_mut();
            let m = graph_editor::render_graph_editor(ui, &mut graph, &mut gs, &theme, &thumbnail_map);
            let sel = if gs.widget_state.selected.len() == 1 {
                Some(gs.widget_state.selected[0])
            } else {
                None
            };
            (m, sel)
        });

        // Sync selection
        if selected != editor_state.selected_node {
            let sel = selected;
            cmds.push(move |world: &mut World| {
                world.resource_mut::<MaterialEditorState>().selected_node = sel;
            });
        }

        if graph_modified || toolbar_modified {
            // Recompile on change
            let result = codegen::compile(&graph);
            let wgsl = result.fragment_shader.clone();
            let errors = result.errors.clone();

            cmds.push(move |world: &mut World| {
                let mut state = world.resource_mut::<MaterialEditorState>();
                state.graph = graph;
                state.is_modified = true;
                state.compiled_wgsl = Some(wgsl);
                state.compile_errors = errors;
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

/// Render toolbar. Returns true if graph was modified.
fn render_toolbar(
    ui: &mut egui::Ui,
    graph: &mut MaterialGraph,
    state: &mut GraphEditorState,
    cmds: &EditorCommands,
    editor_state: &MaterialEditorState,
    theme: &Theme,
) -> bool {
    let mut modified = false;
    let text_color = theme.text.primary.to_color32();
    let text_muted = theme.text.muted.to_color32();
    let accent = theme.semantic.accent.to_color32();

    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        ui.style_mut().spacing.button_padding = egui::vec2(6.0, 2.0);

        // ── New ──
        let new_btn = ui.add(egui::Button::new(
            RichText::new(format!("{FILE_PLUS} New")).size(12.0).color(text_color),
        ));
        let new_id = ui.make_persistent_id("mat_new_popup");
        if new_btn.clicked() {
            ui.memory_mut(|m| m.toggle_popup(new_id));
        }
        egui::popup_below_widget(ui, new_id, &new_btn, egui::PopupCloseBehavior::CloseOnClickOutside, |ui| {
            ui.set_min_width(140.0);
            let domains = [
                (MaterialDomain::Surface, "Surface", CUBE),
                (MaterialDomain::TerrainLayer, "Terrain Layer", egui_phosphor::regular::MOUNTAINS),
                (MaterialDomain::Vegetation, "Vegetation", egui_phosphor::regular::TREE),
                (MaterialDomain::Unlit, "Unlit", egui_phosphor::regular::LIGHTBULB),
            ];
            for (domain, label, icon) in domains {
                if ui.button(format!("{icon} {label}")).clicked() {
                    let new_graph = MaterialGraph::new("New Material", domain);
                    cmds.push(move |world: &mut World| {
                        let mut s = world.resource_mut::<MaterialEditorState>();
                        s.graph = new_graph;
                        s.file_path = None;
                        s.is_modified = false;
                        s.selected_node = None;
                        s.compiled_wgsl = None;
                        s.compile_errors = Vec::new();
                    });
                    state.needs_sync = true;
                    ui.memory_mut(|m| m.toggle_popup(new_id));
                }
            }
        });

        // ── Open ──
        if ui.add(egui::Button::new(
            RichText::new(format!("{FOLDER_OPEN} Open")).size(12.0).color(text_color),
        )).clicked() {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("Material", &["material"])
                .pick_file()
            {
                let path_str = path.to_string_lossy().to_string();
                if let Ok(contents) = std::fs::read_to_string(&path) {
                    if let Ok(loaded_graph) = serde_json::from_str::<MaterialGraph>(&contents) {
                        cmds.push(move |world: &mut World| {
                            let mut s = world.resource_mut::<MaterialEditorState>();
                            s.graph = loaded_graph;
                            s.file_path = Some(path_str);
                            s.is_modified = false;
                            s.selected_node = None;
                        });
                        state.needs_sync = true;
                    }
                }
            }
        }

        // ── Save ──
        let save_label = if editor_state.is_modified {
            format!("{FLOPPY_DISK} Save*")
        } else {
            format!("{FLOPPY_DISK} Save")
        };
        if ui.add(egui::Button::new(
            RichText::new(save_label).size(12.0).color(text_color),
        )).clicked() {
            let save_path = if let Some(ref p) = editor_state.file_path {
                Some(std::path::PathBuf::from(p))
            } else {
                rfd::FileDialog::new()
                    .add_filter("Material", &["material"])
                    .set_file_name(&format!("{}.material", graph.name))
                    .save_file()
            };
            if let Some(path) = save_path {
                if let Ok(json) = serde_json::to_string_pretty(graph) {
                    let _ = std::fs::write(&path, &json);
                    let path_str = path.to_string_lossy().to_string();
                    cmds.push(move |world: &mut World| {
                        let mut s = world.resource_mut::<MaterialEditorState>();
                        s.file_path = Some(path_str);
                        s.is_modified = false;
                    });
                }
            }
        }

        ui.separator();

        // ── Domain indicator ──
        let domain_label = graph.domain.display_name();
        ui.label(RichText::new(format!("{CUBE} {domain_label}")).size(11.0).color(text_muted));

        ui.separator();

        // ── Add Node ──
        let add_btn = ui.add(egui::Button::new(
            RichText::new(format!("{PLUS} Add Node")).size(12.0).color(accent),
        ));
        let add_id = ui.make_persistent_id("mat_add_node_popup");
        if add_btn.clicked() {
            ui.memory_mut(|m| m.toggle_popup(add_id));
        }
        egui::popup_below_widget(ui, add_id, &add_btn, egui::PopupCloseBehavior::CloseOnClickOutside, |ui| {
            ui.set_min_width(160.0);
            for &category in &renzora_material::nodes::categories() {
                if category == "Output" {
                    continue;
                }
                let icon = graph_editor::category_icon(category);
                ui.menu_button(format!("{icon} {category}"), |ui| {
                    let defs = renzora_material::nodes::nodes_in_category(category);
                    for def in &defs {
                        if ui.button(format!("{icon} {}", def.display_name)).clicked() {
                            let canvas_pos = if let Some(cr) = state.canvas_rect {
                                graph_editor::screen_to_canvas(
                                    cr.center(),
                                    state.widget_state.offset,
                                    state.widget_state.zoom,
                                    cr,
                                )
                            } else {
                                [0.0, 0.0]
                            };
                            graph.add_node(def.node_type, canvas_pos);
                            state.needs_sync = true;
                            modified = true;
                            ui.memory_mut(|m| m.toggle_popup(add_id));
                        }
                    }
                });
            }
        });

        ui.separator();

        // ── Center ──
        if ui.add(egui::Button::new(
            RichText::new(format!("{CROSSHAIR}")).size(12.0).color(text_muted),
        )).on_hover_text("Center Graph").clicked() {
            if !graph.nodes.is_empty() {
                let (mut min_x, mut max_x) = (f32::MAX, f32::MIN);
                let (mut min_y, mut max_y) = (f32::MAX, f32::MIN);
                for node in &graph.nodes {
                    min_x = min_x.min(node.position[0]);
                    max_x = max_x.max(node.position[0]);
                    min_y = min_y.min(node.position[1]);
                    max_y = max_y.max(node.position[1]);
                }
                state.widget_state.offset = [-(min_x + max_x) / 2.0, -(min_y + max_y) / 2.0];
            } else {
                state.widget_state.offset = [0.0, 0.0];
            }
        }

        // ── Zoom ──
        if ui.add(egui::Button::new(
            RichText::new(format!("{MAGNIFYING_GLASS_MINUS}")).size(12.0).color(text_muted),
        )).on_hover_text("Zoom Out").clicked() {
            state.widget_state.zoom = (state.widget_state.zoom * 0.8).max(0.25);
        }

        let zoom_pct = (state.widget_state.zoom * 100.0) as u32;
        ui.label(RichText::new(format!("{zoom_pct}%")).size(11.0).color(text_muted));

        if ui.add(egui::Button::new(
            RichText::new(format!("{MAGNIFYING_GLASS_PLUS}")).size(12.0).color(text_muted),
        )).on_hover_text("Zoom In").clicked() {
            state.widget_state.zoom = (state.widget_state.zoom * 1.25).min(4.0);
        }

        // ── Compile status ──
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if editor_state.compile_errors.is_empty() {
                if editor_state.compiled_wgsl.is_some() {
                    ui.label(RichText::new(format!("{} Compiled", egui_phosphor::regular::CHECK_CIRCLE))
                        .size(11.0).color(egui::Color32::from_rgb(80, 200, 120)));
                }
            } else {
                let err_count = editor_state.compile_errors.len();
                let tip = editor_state.compile_errors.join("\n");
                ui.label(RichText::new(format!("{} {} error(s)", egui_phosphor::regular::WARNING_CIRCLE, err_count))
                    .size(11.0).color(egui::Color32::from_rgb(255, 100, 80)))
                    .on_hover_text(tip);
            }
        });
    });

    modified
}
