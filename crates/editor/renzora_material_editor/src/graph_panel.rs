//! Material Graph Panel — selection-driven node graph editor.
//!
//! Selecting a mesh entity in the viewport loads its material into this panel.
//! Edits auto-save to the project's `assets/materials/` directory.

use std::cell::RefCell;

use bevy::prelude::*;
use bevy_egui::egui::{self, RichText};
use egui_phosphor::regular::{
    PLUS, CROSSHAIR,
    MAGNIFYING_GLASS_PLUS, MAGNIFYING_GLASS_MINUS,
    FLOW_ARROW, CUBE,
};

use renzora_editor::{EditorCommands, EditorPanel, EditorSelection, PanelLocation};
use renzora_theme::{Theme, ThemeManager};
use renzora_material::graph::{MaterialGraph, PinValue};
use renzora_material::codegen;
use renzora_material::material_ref::MaterialRef;
use renzora_material::resolver::MaterialResolved;
use renzora_ui::asset_drag::AssetDragPayload;
use renzora_core::CurrentProject;

use crate::{MaterialEditorState, MaterialEditMode};
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

        // ── Selection sync ──────────────────────────────────────────────
        let selection = world.get_resource::<EditorSelection>();
        let selected_entity = selection.and_then(|s| s.get());
        let mat_ref_path = selected_entity.and_then(|e| world.get::<MaterialRef>(e).map(|m| m.0.clone()));

        // Detect entity change OR MaterialRef change on the same entity
        let current_edit_path = match &editor_state.edit_mode {
            MaterialEditMode::Existing { path, .. } => Some(path.clone()),
            _ => None,
        };
        let entity_changed = selected_entity != editor_state.editing_entity;
        let path_changed = !entity_changed && mat_ref_path != current_edit_path;

        if entity_changed || path_changed {
            let new_entity = selected_entity;
            let has_mesh = new_entity.map_or(false, |e| world.get::<Mesh3d>(e).is_some());
            let entity_name = new_entity.and_then(|e| world.get::<Name>(e).map(|n| n.as_str().to_string()));

            cmds.push(move |world: &mut World| {
                sync_to_entity(world, new_entity, has_mesh, mat_ref_path, entity_name);
            });
            GRAPH_EDITOR_STATE.with(|cell| {
                cell.borrow_mut().needs_sync = true;
            });
        }

        // ── Inactive state — show empty message ─────────────────────────
        if matches!(editor_state.edit_mode, MaterialEditMode::Inactive) {
            let text_muted = theme.text.muted.to_color32();
            renzora_editor::empty_state(
                ui,
                egui_phosphor::regular::CUBE,
                "No mesh selected",
                "Select a mesh entity to edit its material",
                &theme,
            );
            // Still show pending status if entity just changed
            let _ = text_muted;
            return;
        }

        // Get thumbnail map for texture node previews
        let thumbnail_map = world
            .get_resource::<crate::thumbnails::NodeThumbnails>()
            .map(|t| t.to_map())
            .unwrap_or_default();

        let mut graph = editor_state.graph.clone();

        // ── Toolbar ──
        let toolbar_modified = GRAPH_EDITOR_STATE.with(|cell| {
            let mut gs = cell.borrow_mut();
            render_toolbar(ui, &mut graph, &mut gs, editor_state, cmds, world, &theme)
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

        // ── Texture drag-and-drop onto canvas ──
        let texture_drop_modified = GRAPH_EDITOR_STATE.with(|cell| {
            let gs = cell.borrow();
            handle_texture_drop(ui, &mut graph, &gs, world)
        });
        if texture_drop_modified {
            GRAPH_EDITOR_STATE.with(|cell| {
                cell.borrow_mut().needs_sync = true;
            });
        }

        // Sync node selection
        if selected != editor_state.selected_node {
            let sel = selected;
            cmds.push(move |world: &mut World| {
                world.resource_mut::<MaterialEditorState>().selected_node = sel;
            });
        }

        // ── On any graph change: recompile + schedule auto-save ──
        if graph_modified || toolbar_modified || texture_drop_modified {
            let result = codegen::compile(&graph);
            let wgsl = result.fragment_shader.clone();
            let errors = result.errors.clone();

            cmds.push(move |world: &mut World| {
                // Check Pending INSIDE the closure (after sync_to_entity has run)
                let pending_entity = {
                    let state = world.resource::<MaterialEditorState>();
                    if let MaterialEditMode::Pending { entity } = state.edit_mode {
                        Some(entity)
                    } else {
                        None
                    }
                };

                if let Some(entity) = pending_entity {
                    let graph_name = {
                        let state = world.resource::<MaterialEditorState>();
                        state.graph.name.clone()
                    };
                    let asset_path = format!("materials/{}.material", graph_name);

                    // Save the initial file
                    if let Some(project) = world.get_resource::<CurrentProject>() {
                        let dir = project.path.join("materials");
                        let _ = std::fs::create_dir_all(&dir);
                        let file = dir.join(format!("{}.material", graph_name));
                        if let Ok(json) = serde_json::to_string_pretty(&graph) {
                            let _ = std::fs::write(&file, &json);
                        }
                    }

                    // Set MaterialRef on the entity
                    world.entity_mut(entity).remove::<MaterialResolved>();
                    world.entity_mut(entity).insert(MaterialRef(asset_path.clone()));

                    let mut state = world.resource_mut::<MaterialEditorState>();
                    state.edit_mode = MaterialEditMode::Existing { path: asset_path, entity };
                }

                let mut state = world.resource_mut::<MaterialEditorState>();
                state.graph = graph;
                state.compiled_wgsl = Some(wgsl);
                state.compile_errors = errors;
                state.is_dirty = true;
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

/// Deferred: load or create a material graph for the newly selected entity.
fn sync_to_entity(
    world: &mut World,
    new_entity: Option<Entity>,
    has_mesh: bool,
    mat_ref_path: Option<String>,
    entity_name: Option<String>,
) {
    let mut state = world.resource_mut::<MaterialEditorState>();
    state.editing_entity = new_entity;
    state.selected_node = None;
    state.is_dirty = false;

    let Some(entity) = new_entity else {
        state.edit_mode = MaterialEditMode::Inactive;
        state.graph = MaterialGraph::new("New Material", renzora_material::graph::MaterialDomain::Surface);
        state.compiled_wgsl = None;
        state.compile_errors.clear();
        return;
    };

    if !has_mesh {
        state.edit_mode = MaterialEditMode::Inactive;
        state.graph = MaterialGraph::new("New Material", renzora_material::graph::MaterialDomain::Surface);
        state.compiled_wgsl = None;
        state.compile_errors.clear();
        return;
    }

    if let Some(path) = mat_ref_path {
        // Entity has a MaterialRef — load the .material file
        let fs_path = if let Some(project) = world.get_resource::<CurrentProject>() {
            project.resolve_path(&path).to_string_lossy().to_string()
        } else {
            path.clone()
        };

        // Need to re-borrow state after accessing project
        let mut state = world.resource_mut::<MaterialEditorState>();

        if let Ok(json) = std::fs::read_to_string(&fs_path) {
            if let Ok(graph) = serde_json::from_str::<MaterialGraph>(&json) {
                let result = codegen::compile(&graph);
                state.compiled_wgsl = Some(result.fragment_shader);
                state.compile_errors = result.errors;
                state.graph = graph;
                state.edit_mode = MaterialEditMode::Existing { path, entity };
                return;
            }
        }

        // File failed to load — treat as pending with the path's name
        warn!("[material_editor] Failed to load '{}', starting fresh", path);
        let name = std::path::Path::new(&path)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("material")
            .to_string();
        let graph = MaterialGraph::new(&name, renzora_material::graph::MaterialDomain::Surface);
        let result = codegen::compile(&graph);
        state.compiled_wgsl = Some(result.fragment_shader);
        state.compile_errors = result.errors;
        state.graph = graph;
        state.edit_mode = MaterialEditMode::Pending { entity };
    } else {
        // No MaterialRef — show empty graph, will save on first edit
        let name = entity_name.unwrap_or_else(|| format!("material_{}", entity.index()));
        let graph = MaterialGraph::new(&name, renzora_material::graph::MaterialDomain::Surface);
        let result = codegen::compile(&graph);

        let mut state = world.resource_mut::<MaterialEditorState>();
        state.compiled_wgsl = Some(result.fragment_shader);
        state.compile_errors = result.errors;
        state.graph = graph;
        state.edit_mode = MaterialEditMode::Pending { entity };
    }
}

/// Render toolbar (no New/Open/Save — selection-driven).
fn render_toolbar(
    ui: &mut egui::Ui,
    graph: &mut MaterialGraph,
    state: &mut GraphEditorState,
    editor_state: &MaterialEditorState,
    cmds: &renzora_editor::EditorCommands,
    world: &World,
    theme: &Theme,
) -> bool {
    let mut modified = false;
    let text_color = theme.text.primary.to_color32();
    let text_muted = theme.text.muted.to_color32();
    let accent = theme.semantic.accent.to_color32();

    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        ui.style_mut().spacing.button_padding = egui::vec2(6.0, 2.0);

        // ── Material name ──
        let name_icon = match &editor_state.edit_mode {
            MaterialEditMode::Pending { .. } => egui_phosphor::regular::PENCIL_SIMPLE,
            MaterialEditMode::Existing { .. } => egui_phosphor::regular::FILE,
            MaterialEditMode::Inactive => egui_phosphor::regular::CUBE,
        };
        ui.label(RichText::new(format!("{name_icon} {}", graph.name)).size(12.0).color(text_color));

        // ── Apply / Apply All ──
        if !matches!(editor_state.edit_mode, MaterialEditMode::Inactive) {
            let apply_label = if editor_state.is_dirty {
                format!("{} Apply*", egui_phosphor::regular::CHECK)
            } else {
                format!("{} Apply", egui_phosphor::regular::CHECK)
            };
            let apply_color = if editor_state.is_dirty { accent } else { text_muted };
            if ui.add(egui::Button::new(RichText::new(apply_label).size(12.0).color(apply_color)))
                .on_hover_text("Save material and apply to mesh")
                .clicked()
            {
                cmds.push(crate::apply_material);
            }

            // Apply All — applies to every selected mesh entity
            let selection = world.get_resource::<EditorSelection>();
            let selected_count = selection.map(|s| s.get_all().len()).unwrap_or(0);
            if selected_count > 1 {
                if ui.add(egui::Button::new(
                    RichText::new(format!("{} Apply All ({})", egui_phosphor::regular::CHECKS, selected_count))
                        .size(12.0).color(accent),
                )).on_hover_text("Apply this material to all selected meshes").clicked() {
                    let entities = selection.unwrap().get_all();
                    cmds.push(move |world: &mut World| {
                        // First save the material
                        crate::apply_material(world);

                        // Get the material path
                        let path = {
                            let state = world.resource::<MaterialEditorState>();
                            match &state.edit_mode {
                                MaterialEditMode::Existing { path, .. } => Some(path.clone()),
                                _ => None,
                            }
                        };

                        if let Some(path) = path {
                            for entity in &entities {
                                // Only apply to entities with Mesh3d
                                if world.get::<Mesh3d>(*entity).is_none() {
                                    continue;
                                }
                                world.entity_mut(*entity).remove::<renzora_material::resolver::MaterialResolved>();
                                world.entity_mut(*entity).insert(MaterialRef(path.clone()));
                            }
                        }
                    });
                }
            }
        }

        // ── Domain indicator ──
        ui.separator();
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

            if matches!(editor_state.edit_mode, MaterialEditMode::Pending { .. }) {
                ui.label(RichText::new("New").size(10.0).color(text_muted));
            }
            if editor_state.is_dirty {
                ui.label(RichText::new("Unsaved").size(10.0).color(text_muted));
            }
        });
    });

    modified
}

const IMAGE_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "ktx2", "tga", "bmp", "dds", "exr", "hdr", "webp"];

fn is_image_extension(ext: &str) -> bool {
    IMAGE_EXTENSIONS.iter().any(|&allowed| ext.eq_ignore_ascii_case(allowed))
}

/// Check for texture asset drag-and-drop onto the graph canvas.
fn handle_texture_drop(
    ui: &mut egui::Ui,
    graph: &mut MaterialGraph,
    state: &GraphEditorState,
    world: &World,
) -> bool {
    let Some(canvas_rect) = state.canvas_rect else {
        return false;
    };

    // --- Asset browser drag ---
    let payload = world.get_resource::<AssetDragPayload>();
    let has_asset_drag = payload
        .filter(|p| p.is_detached && p.matches_extensions(IMAGE_EXTENSIONS))
        .is_some();

    // --- OS file drag ---
    let os_hovering = ui.ctx().input(|i| {
        i.raw.hovered_files.iter().any(|f| {
            f.path.as_ref().and_then(|p| p.extension()).and_then(|e| e.to_str())
                .map_or(false, is_image_extension)
        })
    });
    let os_dropped: Option<std::path::PathBuf> = ui.ctx().input(|i| {
        i.raw.dropped_files.iter()
            .find(|f| {
                f.path.as_ref().and_then(|p| p.extension()).and_then(|e| e.to_str())
                    .map_or(false, is_image_extension)
            })
            .and_then(|f| f.path.clone())
    });

    let pointer = ui.ctx().pointer_hover_pos();
    let pointer_in_canvas = pointer.map_or(false, |p| canvas_rect.contains(p));

    // Draw visual feedback overlay — only when pointer is over the canvas
    if (has_asset_drag || os_hovering) && pointer_in_canvas {
        let accent = if let Some(p) = payload.filter(|_| has_asset_drag) {
            p.color
        } else {
            egui::Color32::from_rgb(80, 140, 255)
        };

        let painter = ui.painter();
        painter.rect_filled(
            canvas_rect,
            0.0,
            egui::Color32::from_rgba_unmultiplied(accent.r(), accent.g(), accent.b(), 15),
        );
        painter.rect_stroke(
            canvas_rect,
            0.0,
            egui::Stroke::new(2.0, accent),
            egui::StrokeKind::Inside,
        );

        let label_pos = if let Some(pos) = pointer {
            pos + egui::Vec2::new(0.0, -20.0)
        } else {
            canvas_rect.center()
        };
        painter.text(
            label_pos,
            egui::Align2::CENTER_BOTTOM,
            format!("{} Drop to create texture node", egui_phosphor::regular::IMAGE),
            egui::FontId::proportional(12.0),
            accent,
        );
    }

    // --- Handle OS file drop (only if pointer is over the canvas) ---
    if let Some(os_path) = os_dropped.filter(|_| pointer_in_canvas) {
        let texture_path = if let Some(project) = world.get_resource::<CurrentProject>() {
            let rel = project.make_asset_relative(&os_path);
            if std::path::Path::new(&rel).is_absolute() {
                let textures_dir = project.path.join("textures");
                let _ = std::fs::create_dir_all(&textures_dir);
                if let Some(filename) = os_path.file_name() {
                    let dest = textures_dir.join(filename);
                    match std::fs::copy(&os_path, &dest) {
                        Ok(_) => project.make_asset_relative(&dest),
                        Err(e) => {
                            warn!("Failed to import texture: {}", e);
                            return false;
                        }
                    }
                } else {
                    return false;
                }
            } else {
                rel
            }
        } else {
            os_path.to_string_lossy().to_string()
        };

        let drop_pos = pointer.unwrap_or(canvas_rect.center());
        let canvas_pos = graph_editor::screen_to_canvas(
            drop_pos,
            state.widget_state.offset,
            state.widget_state.zoom,
            canvas_rect,
        );

        return spawn_texture_nodes(graph, canvas_pos, texture_path);
    }

    // --- Handle asset browser drop ---
    if !has_asset_drag || !pointer_in_canvas {
        return false;
    }
    if ui.ctx().input(|i| i.pointer.any_down()) {
        return false;
    }

    let payload = payload.unwrap();
    let texture_path = if let Some(project) = world.get_resource::<CurrentProject>() {
        project.make_asset_relative(&payload.path)
    } else {
        payload.path.to_string_lossy().to_string()
    };

    let drop_pos = pointer.unwrap();
    let canvas_pos = graph_editor::screen_to_canvas(
        drop_pos,
        state.widget_state.offset,
        state.widget_state.zoom,
        canvas_rect,
    );

    spawn_texture_nodes(graph, canvas_pos, texture_path)
}

/// Create a Sample Texture node with the texture path pre-set.
fn spawn_texture_nodes(graph: &mut MaterialGraph, canvas_pos: [f32; 2], texture_path: String) -> bool {
    let tex_id = graph.add_node("texture/sample", canvas_pos);
    if let Some(tex_node) = graph.get_node_mut(tex_id) {
        tex_node.input_values.insert("texture".to_string(), PinValue::TexturePath(texture_path));
    }
    true
}
