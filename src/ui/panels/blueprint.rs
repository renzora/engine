//! Blueprint visual scripting panel

use bevy_egui::egui::{self, Color32, Pos2, Rect, RichText, Sense};
use std::collections::HashMap;

use crate::blueprint::{
    BlueprintCanvasState, BlueprintEditorState, NodeId, PinId,
    canvas::{draw_grid, draw_node, draw_connections, draw_pending_connection, draw_box_selection},
    interactions::{process_canvas_interactions, render_add_node_popup},
    nodes::NodeRegistry,
    serialization::{BlueprintFile, list_blueprints},
    generate_rhai_code,
};
use crate::project::CurrentProject;

/// Render the blueprint editor panel
pub fn render_blueprint_panel(
    ui: &mut egui::Ui,
    ctx: &egui::Context,
    editor_state: &mut BlueprintEditorState,
    canvas_state: &mut BlueprintCanvasState,
    node_registry: &NodeRegistry,
    current_project: Option<&CurrentProject>,
) {
    // Initialize canvas state if needed
    if canvas_state.zoom == 0.0 {
        canvas_state.zoom = 1.0;
    }

    ui.vertical(|ui| {
        // Toolbar
        render_blueprint_toolbar(ui, editor_state, canvas_state, node_registry, current_project);

        ui.separator();

        // Tab bar for open blueprints
        render_blueprint_tabs(ui, editor_state);

        // Main canvas area
        let available_rect = ui.available_rect_before_wrap();

        if editor_state.active_blueprint.is_some() {
            render_blueprint_canvas(ui, ctx, editor_state, canvas_state, node_registry, available_rect);
        } else {
            render_empty_state(ui, editor_state, current_project);
        }
    });
}

/// Render the blueprint toolbar
fn render_blueprint_toolbar(
    ui: &mut egui::Ui,
    editor_state: &mut BlueprintEditorState,
    canvas_state: &mut BlueprintCanvasState,
    _node_registry: &NodeRegistry,
    current_project: Option<&CurrentProject>,
) {
    ui.horizontal(|ui| {
        // New blueprint button
        if ui.button("\u{2795} New").clicked() {
            let name = format!("blueprint_{}", editor_state.open_blueprints.len() + 1);
            editor_state.create_new_blueprint(&name);
        }

        // Open blueprint button
        if ui.button("\u{1F4C2} Open").clicked() {
            if let Some(project) = current_project {
                let blueprints_dir = project.path.join("blueprints");
                let blueprints = list_blueprints(&blueprints_dir);

                // TODO: Show file picker popup
                // For now, load first available if any
                if let Some(path) = blueprints.first() {
                    if let Ok(file) = BlueprintFile::load(path) {
                        let path_str = path.to_string_lossy().to_string();
                        editor_state.open_blueprints.insert(path_str.clone(), file.graph);
                        editor_state.active_blueprint = Some(path_str);
                    }
                }
            }
        }

        // Save button
        if ui.add_enabled(editor_state.active_blueprint.is_some(), egui::Button::new("\u{1F4BE} Save")).clicked() {
            save_current_blueprint(editor_state, current_project);
        }

        ui.separator();

        // Compile button
        if ui.add_enabled(editor_state.active_blueprint.is_some(), egui::Button::new("\u{25B6} Compile")).clicked() {
            if let Some(graph) = editor_state.active_graph() {
                let result = generate_rhai_code(graph);
                if result.errors.is_empty() {
                    info!("Blueprint compiled successfully:\n{}", result.code);
                } else {
                    for err in &result.errors {
                        error!("Blueprint error: {}", err);
                    }
                }
                for warn in &result.warnings {
                    warn!("Blueprint warning: {}", warn);
                }
            }
        }

        ui.separator();

        // Zoom controls
        ui.label("Zoom:");
        if ui.button("-").clicked() {
            canvas_state.zoom = (canvas_state.zoom * 0.8).max(0.25);
        }
        ui.label(format!("{:.0}%", canvas_state.zoom * 100.0));
        if ui.button("+").clicked() {
            canvas_state.zoom = (canvas_state.zoom * 1.25).min(4.0);
        }

        // Reset view button
        if ui.button("\u{1F3E0}").on_hover_text("Reset view").clicked() {
            canvas_state.offset = [0.0, 0.0];
            canvas_state.zoom = 1.0;
        }
    });
}

/// Render tabs for open blueprints
fn render_blueprint_tabs(ui: &mut egui::Ui, editor_state: &mut BlueprintEditorState) {
    if editor_state.open_blueprints.is_empty() {
        return;
    }

    ui.horizontal(|ui| {
        let paths: Vec<_> = editor_state.open_blueprints.keys().cloned().collect();

        for path in paths {
            let name = path
                .rsplit('/')
                .next()
                .unwrap_or(&path)
                .trim_end_matches(".blueprint");

            let is_active = editor_state.active_blueprint.as_ref() == Some(&path);

            let tab_response = ui.selectable_label(is_active, name);
            if tab_response.clicked() {
                editor_state.active_blueprint = Some(path.clone());
            }

            // Right-click to close
            if tab_response.secondary_clicked() {
                editor_state.open_blueprints.remove(&path);
                if editor_state.active_blueprint.as_ref() == Some(&path) {
                    editor_state.active_blueprint = editor_state.open_blueprints.keys().next().cloned();
                }
            }
        }
    });
}

/// Render the main blueprint canvas
fn render_blueprint_canvas(
    ui: &mut egui::Ui,
    ctx: &egui::Context,
    editor_state: &mut BlueprintEditorState,
    canvas_state: &mut BlueprintCanvasState,
    node_registry: &NodeRegistry,
    canvas_rect: Rect,
) {
    // Allocate the canvas area
    let (response, painter) = ui.allocate_painter(canvas_rect.size(), Sense::click_and_drag());
    let canvas_rect = response.rect;

    // Dark background
    painter.rect_filled(canvas_rect, 0.0, Color32::from_rgb(25, 25, 30));

    // Draw grid
    draw_grid(&painter, canvas_rect, canvas_state);

    // Draw drag preview if dragging a new node from library
    if let Some(type_id) = &editor_state.dragging_new_node {
        if let Some(mouse_pos) = ctx.input(|i| i.pointer.hover_pos()) {
            if canvas_rect.contains(mouse_pos) {
                // Draw a ghost preview of the node
                let node_def = node_registry.get(type_id);
                let display_name = node_def.map(|d| d.display_name).unwrap_or(type_id);
                let color = node_def.map(|d| Color32::from_rgb(d.color[0], d.color[1], d.color[2]))
                    .unwrap_or(Color32::from_rgb(100, 100, 100));

                let preview_rect = Rect::from_min_size(
                    mouse_pos - egui::vec2(60.0, 15.0),
                    egui::vec2(120.0, 30.0),
                );

                // Draw semi-transparent preview
                painter.rect_filled(preview_rect, 4.0, Color32::from_rgba_unmultiplied(40, 40, 45, 180));
                painter.rect_filled(
                    Rect::from_min_size(preview_rect.min, egui::vec2(preview_rect.width(), 20.0)),
                    egui::CornerRadius { nw: 4, ne: 4, sw: 0, se: 0 },
                    Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 180),
                );
                painter.text(
                    preview_rect.center_top() + egui::vec2(0.0, 10.0),
                    egui::Align2::CENTER_CENTER,
                    display_name,
                    egui::FontId::proportional(11.0),
                    Color32::from_rgba_unmultiplied(255, 255, 255, 200),
                );

                // Show drop hint
                painter.text(
                    preview_rect.center_bottom() + egui::vec2(0.0, 15.0),
                    egui::Align2::CENTER_CENTER,
                    "Release to place",
                    egui::FontId::proportional(10.0),
                    Color32::from_rgba_unmultiplied(150, 200, 150, 200),
                );
            }
        }
    }

    // Clone graph for rendering (avoid borrow issues)
    let graph = editor_state.active_graph().cloned();
    let Some(graph) = graph else { return };

    // Track node rects and pin positions
    let mut node_rects: HashMap<NodeId, Rect> = HashMap::new();
    editor_state.pin_positions.clear();

    // Draw nodes
    for node in &graph.nodes {
        let node_def = node_registry.get(&node.node_type);
        let is_selected = editor_state.is_node_selected(node.id);

        let (node_rect, pin_pos) = draw_node(
            ui,
            &painter,
            node,
            node_def,
            canvas_state,
            canvas_rect,
            is_selected,
            editor_state,
        );

        node_rects.insert(node.id, node_rect);

        // Store pin positions globally (with direction to distinguish same-named pins)
        for ((pin_name, direction), pos) in pin_pos {
            editor_state.pin_positions.insert(
                PinId { node_id: node.id, pin_name, direction },
                pos
            );
        }
    }

    // Draw connections
    draw_connections(&painter, &graph, &editor_state.pin_positions, canvas_state.zoom);

    // Draw pending connection (if creating one)
    if let Some(from_pin) = &editor_state.creating_connection {
        if let Some(&from_pos) = editor_state.pin_positions.get(from_pin) {
            let mouse_pos = ctx.input(|i| i.pointer.hover_pos()).unwrap_or(from_pos);
            draw_pending_connection(&painter, from_pos, mouse_pos, canvas_state.zoom);
        }
    }

    // Draw box selection
    if let (Some(start), Some(end)) = (editor_state.box_select_start, editor_state.box_select_end) {
        draw_box_selection(&painter, start, end);
    }

    // Process interactions
    let interaction_result = process_canvas_interactions(
        ui,
        &response,
        canvas_state,
        editor_state,
        &graph,
        &node_rects,
        canvas_rect,
    );

    // Apply node dragging - inline to avoid borrow issues
    if let Some(dragging_id) = editor_state.dragging_node {
        let mouse_pos = ctx.input(|i| i.pointer.hover_pos()).unwrap_or(Pos2::ZERO);

        // Calculate new canvas position
        let target_screen = Pos2::new(
            mouse_pos.x - editor_state.drag_offset[0],
            mouse_pos.y - editor_state.drag_offset[1],
        );
        let target_canvas = canvas_state.screen_to_canvas(target_screen, canvas_rect);

        // Get current position and calculate delta
        let delta = if let Some(graph) = editor_state.active_graph() {
            if let Some(dragging_node) = graph.get_node(dragging_id) {
                Some((
                    target_canvas.x - dragging_node.position[0],
                    target_canvas.y - dragging_node.position[1],
                ))
            } else {
                None
            }
        } else {
            None
        };

        // Apply delta to all selected nodes
        if let Some((delta_x, delta_y)) = delta {
            let selected = editor_state.selected_nodes.clone();
            if let Some(graph) = editor_state.active_graph_mut() {
                for node_id in selected {
                    if let Some(node) = graph.get_node_mut(node_id) {
                        node.position[0] += delta_x;
                        node.position[1] += delta_y;
                    }
                }
            }
        }
    }

    // Handle connection completion
    if let Some((from, to)) = interaction_result.connection_completed {
        if let Some(graph) = editor_state.active_graph_mut() {
            graph.add_connection(from, to);
        }
    }

    // Handle drop from Node Library (drag-and-drop)
    let mouse_pos = ctx.input(|i| i.pointer.hover_pos());
    let mouse_released = ctx.input(|i| i.pointer.any_released());

    if let Some(type_id) = editor_state.dragging_new_node.take() {
        // Check if mouse is over the canvas and was released
        if let Some(pos) = mouse_pos {
            if canvas_rect.contains(pos) && mouse_released {
                // Convert screen position to canvas position
                let canvas_pos = canvas_state.screen_to_canvas(pos, canvas_rect);

                if let Some(graph) = editor_state.active_graph_mut() {
                    let node_id = graph.next_node_id();
                    if let Some(mut node) = node_registry.create_node(&type_id, node_id) {
                        node.position = [canvas_pos.x, canvas_pos.y];
                        graph.add_node(node);
                    }
                }
            } else if !mouse_released {
                // Still dragging, put it back
                editor_state.dragging_new_node = Some(type_id);
            }
            // If released outside canvas, just drop it (don't put back)
        }
    }

    // Handle node deletion
    if !interaction_result.nodes_to_delete.is_empty() {
        if let Some(graph) = editor_state.active_graph_mut() {
            for node_id in &interaction_result.nodes_to_delete {
                graph.remove_node(*node_id);
            }
        }
        editor_state.clear_selection();
    }

    // Handle wire cutting (Alt+click or right-click on connected pin)
    if !interaction_result.connections_to_remove.is_empty() {
        if let Some(graph) = editor_state.active_graph_mut() {
            for pin_id in &interaction_result.connections_to_remove {
                graph.remove_connections_for_pin(pin_id);
            }
        }
    }

    // Handle add node from popup
    if let Some((type_id, position, connecting_from)) = render_add_node_popup(ui, editor_state, node_registry, canvas_state, canvas_rect) {
        if let Some(graph) = editor_state.active_graph_mut() {
            let node_id = graph.next_node_id();
            if let Some(mut node) = node_registry.create_node(&type_id, node_id) {
                node.position = position;
                graph.add_node(node);

                // Auto-connect if we were creating a connection
                if let Some(from_pin) = connecting_from {
                    // Collect pin info first to avoid borrow issues
                    let source_is_output = graph.get_node(from_pin.node_id)
                        .and_then(|n| n.get_output_pin(&from_pin.pin_name))
                        .is_some();

                    // Get target pins from new node
                    let target_pins: Vec<_> = if let Some(new_node) = graph.get_node(node_id) {
                        if source_is_output {
                            new_node.input_pins().map(|p| p.name.clone()).collect()
                        } else {
                            new_node.output_pins().map(|p| p.name.clone()).collect()
                        }
                    } else {
                        vec![]
                    };

                    // Now make the connection
                    for pin_name in target_pins {
                        if source_is_output {
                            let to_pin = PinId::new(node_id, &pin_name);
                            if graph.add_connection(from_pin.clone(), to_pin) {
                                break;
                            }
                        } else {
                            let new_from = PinId::new(node_id, &pin_name);
                            if graph.add_connection(new_from, from_pin.clone()) {
                                break;
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Render empty state when no blueprint is open
fn render_empty_state(
    ui: &mut egui::Ui,
    editor_state: &mut BlueprintEditorState,
    current_project: Option<&CurrentProject>,
) {
    ui.vertical_centered(|ui| {
        ui.add_space(60.0);

        ui.label(RichText::new("\u{1F4C4}").size(48.0).color(Color32::from_gray(80)));
        ui.add_space(16.0);

        ui.label(RichText::new("No Blueprint Open").size(18.0).color(Color32::from_gray(140)));
        ui.add_space(8.0);

        ui.label(RichText::new("Create a new blueprint or open an existing one").size(12.0).weak());
        ui.add_space(24.0);

        if ui.button("Create New Blueprint").clicked() {
            let name = format!("blueprint_{}", editor_state.open_blueprints.len() + 1);
            editor_state.create_new_blueprint(&name);
        }

        ui.add_space(8.0);

        // List existing blueprints
        if let Some(project) = current_project {
            let blueprints_dir = project.path.join("blueprints");
            let blueprints = list_blueprints(&blueprints_dir);

            if !blueprints.is_empty() {
                ui.add_space(16.0);
                ui.label(RichText::new("Recent Blueprints:").size(12.0).color(Color32::from_gray(120)));
                ui.add_space(4.0);

                for path in blueprints.iter().take(5) {
                    let name = path.file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("unknown");

                    if ui.button(name).clicked() {
                        if let Ok(file) = BlueprintFile::load(path) {
                            let path_str = path.to_string_lossy().to_string();
                            editor_state.open_blueprints.insert(path_str.clone(), file.graph);
                            editor_state.active_blueprint = Some(path_str);
                        }
                    }
                }
            }
        }
    });
}

/// Save the currently active blueprint
fn save_current_blueprint(
    editor_state: &BlueprintEditorState,
    current_project: Option<&CurrentProject>,
) {
    let Some(project) = current_project else {
        error!("No project open, cannot save blueprint");
        return;
    };

    let Some(path) = &editor_state.active_blueprint else {
        return;
    };

    let Some(graph) = editor_state.active_graph() else {
        return;
    };

    let file = BlueprintFile::new(graph.clone());
    let full_path = project.path.join(path);

    match file.save(&full_path) {
        Ok(_) => info!("Blueprint saved to {:?}", full_path),
        Err(e) => error!("Failed to save blueprint: {}", e),
    }
}

use bevy::log::{info, error, warn};
