//! Blueprint visual scripting panel

use bevy_egui::egui::{self, Color32, Pos2, Rect, RichText, Sense};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use crate::blueprint::{
    BlueprintCanvasState, BlueprintEditorState, BlueprintType, BlueprintGraph, NodeId, PinId, PinValue,
    canvas::{draw_grid, draw_node, draw_connections, draw_pending_connection, draw_box_selection, NodeValueChange, is_texture_node},
    interactions::{process_canvas_interactions, render_add_node_popup},
    nodes::NodeRegistry,
    serialization::{BlueprintFile, list_blueprints},
    generate_rhai_code,
    compile_material_blueprint,
};
use crate::core::{AssetBrowserState, ThumbnailCache};
use crate::project::CurrentProject;

/// Render the blueprint editor panel
pub fn render_blueprint_panel(
    ui: &mut egui::Ui,
    ctx: &egui::Context,
    editor_state: &mut BlueprintEditorState,
    canvas_state: &mut BlueprintCanvasState,
    node_registry: &NodeRegistry,
    current_project: Option<&CurrentProject>,
    assets: &mut AssetBrowserState,
    thumbnail_cache: &mut ThumbnailCache,
) {
    // Initialize canvas state if needed
    if canvas_state.zoom == 0.0 {
        canvas_state.zoom = 1.0;
    }

    ui.vertical(|ui| {
        // Toolbar (only show if a blueprint is open)
        if editor_state.active_blueprint.is_some() {
            render_blueprint_toolbar(ui, editor_state, canvas_state, node_registry, current_project);
            ui.separator();
        }

        // Main canvas area
        let available_rect = ui.available_rect_before_wrap();

        if editor_state.active_blueprint.is_some() {
            render_blueprint_canvas(ui, ctx, editor_state, canvas_state, node_registry, available_rect, assets, current_project, thumbnail_cache);
        } else {
            // Simple message when no blueprint is open
            ui.vertical_centered(|ui| {
                ui.add_space(60.0);
                ui.label(egui::RichText::new("No Blueprint Open").size(14.0).color(egui::Color32::from_gray(120)));
                ui.add_space(8.0);
                ui.label(egui::RichText::new("Right-click in Assets panel to create a material").size(11.0).color(egui::Color32::from_gray(100)));
            });
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
        // Save button
        if ui.button("\u{1F4BE} Save").clicked() {
            save_current_blueprint(editor_state, current_project);
        }

        ui.separator();

        // Show current blueprint type indicator
        if let Some(graph) = editor_state.active_graph() {
            let (type_icon, type_name, type_color) = match graph.graph_type {
                BlueprintType::Behavior => ("\u{1F4DC}", "Behavior", Color32::from_rgb(100, 180, 100)),
                BlueprintType::Material => ("\u{1F3A8}", "Material", Color32::from_rgb(180, 130, 200)),
            };
            ui.label(RichText::new(format!("{} {}", type_icon, type_name)).color(type_color));
            ui.separator();
        }

        // Compile button
        let compile_text = match editor_state.active_graph().map(|g| g.graph_type) {
            Some(BlueprintType::Material) => "\u{25B6} Generate Shader",
            _ => "\u{25B6} Compile",
        };
        if ui.add_enabled(editor_state.active_blueprint.is_some(), egui::Button::new(compile_text)).clicked() {
            if let Some(graph) = editor_state.active_graph() {
                match graph.graph_type {
                    BlueprintType::Material => {
                        // Generate WGSL shader code
                        let result = compile_material_blueprint(graph);
                        if result.is_ok() {
                            console_log(LogLevel::Success, "Blueprint", format!("Material '{}' compiled successfully!", result.name));
                            console_log(LogLevel::Info, "Blueprint", format!("Generated WGSL shader:\n{}", result.shader_code));

                            // Save shader to project if we have one
                            if let Some(project) = current_project {
                                let shaders_dir = project.path.join("shaders");
                                if let Err(e) = crate::blueprint::save_compiled_material(&result, &shaders_dir) {
                                    console_log(LogLevel::Error, "Blueprint", format!("Failed to save shader: {}", e));
                                }
                            }
                        } else {
                            for err in &result.errors {
                                console_log(LogLevel::Error, "Blueprint", format!("Material compilation error: {}", err));
                            }
                        }
                        for warning in &result.warnings {
                            console_log(LogLevel::Warning, "Blueprint", format!("Material compilation warning: {}", warning));
                        }
                    }
                    BlueprintType::Behavior => {
                        let result = generate_rhai_code(graph);
                        if result.errors.is_empty() {
                            console_log(LogLevel::Success, "Blueprint", format!("Blueprint compiled successfully:\n{}", result.code));
                        } else {
                            for err in &result.errors {
                                console_log(LogLevel::Error, "Blueprint", format!("Blueprint error: {}", err));
                            }
                        }
                        for warning in &result.warnings {
                            console_log(LogLevel::Warning, "Blueprint", format!("Blueprint warning: {}", warning));
                        }
                    }
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

/// Render tabs for open blueprints (deprecated - tabs now in document tabs bar)
#[allow(dead_code)]
fn render_blueprint_tabs(ui: &mut egui::Ui, editor_state: &mut BlueprintEditorState) {
    if editor_state.open_blueprints.is_empty() {
        return;
    }

    ui.horizontal(|ui| {
        let paths: Vec<_> = editor_state.open_blueprints.keys().cloned().collect();

        for path in paths {
            let graph = editor_state.open_blueprints.get(&path);
            let graph_type = graph.map(|g| g.graph_type).unwrap_or(BlueprintType::Behavior);

            let name = path
                .rsplit('/')
                .next()
                .unwrap_or(&path)
                .trim_end_matches(".blueprint")
                .trim_end_matches(".material_bp")
                .trim_end_matches(".render_bp");

            // Add type icon to tab name
            let tab_name = match graph_type {
                BlueprintType::Behavior => format!("\u{1F4DC} {}", name),
                BlueprintType::Material => format!("\u{1F3A8} {}", name),
            };

            let is_active = editor_state.active_blueprint.as_ref() == Some(&path);

            let tab_response = ui.selectable_label(is_active, tab_name);
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
    assets: &mut AssetBrowserState,
    current_project: Option<&CurrentProject>,
    thumbnail_cache: &mut ThumbnailCache,
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

    // Build a map of connected inputs for each node
    let mut connected_inputs_map: HashMap<NodeId, HashSet<String>> = HashMap::new();
    for conn in &graph.connections {
        connected_inputs_map
            .entry(conn.to.node_id)
            .or_default()
            .insert(conn.to.pin_name.clone());
    }

    // Track node rects and pin positions
    let mut node_rects: HashMap<NodeId, Rect> = HashMap::new();
    let mut all_value_changes: Vec<NodeValueChange> = Vec::new();
    editor_state.pin_positions.clear();

    // Draw nodes
    for node in &graph.nodes {
        let node_def = node_registry.get(&node.node_type);
        let is_selected = editor_state.is_node_selected(node.id);
        let connected_inputs = connected_inputs_map.get(&node.id);
        let empty_set = HashSet::new();
        let connected = connected_inputs.unwrap_or(&empty_set);

        let (node_rect, pin_pos, value_changes) = draw_node(
            ui,
            &painter,
            node,
            node_def,
            canvas_state,
            canvas_rect,
            is_selected,
            connected,
            editor_state,
        );

        node_rects.insert(node.id, node_rect);
        all_value_changes.extend(value_changes);

        // Store pin positions globally (with direction to distinguish same-named pins)
        for ((pin_name, direction), pos) in pin_pos {
            editor_state.pin_positions.insert(
                PinId { node_id: node.id, pin_name, direction },
                pos
            );
        }

        // Draw texture preview for texture nodes
        if is_texture_node(&node.node_type) {
            if let Some(PinValue::Texture2D(texture_path)) = node.get_input_value("path") {
                if !texture_path.is_empty() {
                    // Resolve full path
                    let full_path = if let Some(project) = current_project {
                        project.path.join(&texture_path)
                    } else {
                        PathBuf::from(&texture_path)
                    };

                    // Calculate preview rect inside the node (larger size)
                    let preview_size = 100.0 * canvas_state.zoom;
                    let preview_margin = 10.0 * canvas_state.zoom;
                    let preview_rect = Rect::from_min_size(
                        Pos2::new(
                            node_rect.center().x - preview_size / 2.0,
                            node_rect.max.y - preview_size - preview_margin,
                        ),
                        egui::vec2(preview_size, preview_size),
                    );

                    // Get or request texture
                    if let Some(texture_id) = thumbnail_cache.get_texture_id(&full_path) {
                        // Draw checkerboard background for transparency
                        draw_checkerboard(&painter, preview_rect, canvas_state.zoom);

                        // Draw texture preview
                        painter.image(
                            texture_id,
                            preview_rect,
                            Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                            Color32::WHITE,
                        );

                        // Draw border
                        painter.rect_stroke(
                            preview_rect,
                            2.0 * canvas_state.zoom,
                            egui::Stroke::new(1.0, Color32::from_gray(60)),
                            egui::StrokeKind::Inside,
                        );
                    } else if !thumbnail_cache.is_loading(&full_path) && !thumbnail_cache.has_failed(&full_path) {
                        // Request loading
                        thumbnail_cache.request_load(full_path.clone());

                        // Draw placeholder
                        painter.rect_filled(preview_rect, 2.0 * canvas_state.zoom, Color32::from_gray(40));
                        painter.text(
                            preview_rect.center(),
                            egui::Align2::CENTER_CENTER,
                            "\u{1F5BC}",
                            egui::FontId::proportional(28.0 * canvas_state.zoom),
                            Color32::from_gray(80),
                        );
                    } else if thumbnail_cache.is_loading(&full_path) {
                        // Show loading indicator
                        painter.rect_filled(preview_rect, 2.0 * canvas_state.zoom, Color32::from_gray(40));
                        painter.text(
                            preview_rect.center(),
                            egui::Align2::CENTER_CENTER,
                            "...",
                            egui::FontId::proportional(18.0 * canvas_state.zoom),
                            Color32::from_gray(100),
                        );
                    } else {
                        // Failed to load
                        painter.rect_filled(preview_rect, 2.0 * canvas_state.zoom, Color32::from_rgba_unmultiplied(60, 40, 40, 255));
                        painter.text(
                            preview_rect.center(),
                            egui::Align2::CENTER_CENTER,
                            "!",
                            egui::FontId::proportional(28.0 * canvas_state.zoom),
                            Color32::from_rgb(200, 100, 100),
                        );
                    }
                } else {
                    // No path set - show drop hint
                    let preview_size = 100.0 * canvas_state.zoom;
                    let preview_margin = 10.0 * canvas_state.zoom;
                    let preview_rect = Rect::from_min_size(
                        Pos2::new(
                            node_rect.center().x - preview_size / 2.0,
                            node_rect.max.y - preview_size - preview_margin,
                        ),
                        egui::vec2(preview_size, preview_size),
                    );

                    painter.rect_filled(preview_rect, 2.0 * canvas_state.zoom, Color32::from_gray(35));
                    painter.rect_stroke(
                        preview_rect,
                        2.0 * canvas_state.zoom,
                        egui::Stroke::new(1.0, Color32::from_gray(50)),
                        egui::StrokeKind::Inside,
                    );
                    painter.text(
                        preview_rect.center() - egui::vec2(0.0, 8.0 * canvas_state.zoom),
                        egui::Align2::CENTER_CENTER,
                        "\u{1F5BC}",
                        egui::FontId::proportional(24.0 * canvas_state.zoom),
                        Color32::from_gray(60),
                    );
                    painter.text(
                        preview_rect.center() + egui::vec2(0.0, 14.0 * canvas_state.zoom),
                        egui::Align2::CENTER_CENTER,
                        "Drop texture",
                        egui::FontId::proportional(10.0 * canvas_state.zoom),
                        Color32::from_gray(70),
                    );
                }
            } else {
                // No path value at all - show drop hint
                let preview_size = 100.0 * canvas_state.zoom;
                let preview_margin = 10.0 * canvas_state.zoom;
                let preview_rect = Rect::from_min_size(
                    Pos2::new(
                        node_rect.center().x - preview_size / 2.0,
                        node_rect.max.y - preview_size - preview_margin,
                    ),
                    egui::vec2(preview_size, preview_size),
                );

                painter.rect_filled(preview_rect, 2.0 * canvas_state.zoom, Color32::from_gray(35));
                painter.rect_stroke(
                    preview_rect,
                    2.0 * canvas_state.zoom,
                    egui::Stroke::new(1.0, Color32::from_gray(50)),
                    egui::StrokeKind::Inside,
                );
                painter.text(
                    preview_rect.center() - egui::vec2(0.0, 8.0 * canvas_state.zoom),
                    egui::Align2::CENTER_CENTER,
                    "\u{1F5BC}",
                    egui::FontId::proportional(24.0 * canvas_state.zoom),
                    Color32::from_gray(60),
                );
                painter.text(
                    preview_rect.center() + egui::vec2(0.0, 14.0 * canvas_state.zoom),
                    egui::Align2::CENTER_CENTER,
                    "Drop texture",
                    egui::FontId::proportional(10.0 * canvas_state.zoom),
                    Color32::from_gray(70),
                );
            }
        }
    }

    // Apply value changes from inline editors
    if !all_value_changes.is_empty() {
        if let Some(graph) = editor_state.active_graph_mut() {
            for change in all_value_changes {
                if let Some(node) = graph.get_node_mut(change.node_id) {
                    node.set_input_value(change.pin_name, change.new_value);
                }
            }
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
            console_log(LogLevel::Info, "Blueprint", format!("Connection: {:?}.{} -> {:?}.{}",
                from.node_id, from.pin_name, to.node_id, to.pin_name));
            graph.add_connection(from.clone(), to.clone());

            // Log connection count and verify it was added
            console_log(LogLevel::Info, "Blueprint", format!("Total connections after add: {}", graph.connections.len()));
            for conn in &graph.connections {
                console_log(LogLevel::Info, "Blueprint", format!("  - {:?}.{} -> {:?}.{}",
                    conn.from.node_id, conn.from.pin_name, conn.to.node_id, conn.to.pin_name));
            }
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

    // Handle drop from Assets panel (textures create Texture nodes, blueprints get loaded)
    if let Some(asset_path) = assets.dragging_asset.take() {
        if let Some(pos) = mouse_pos {
            if canvas_rect.contains(pos) && mouse_released {
                // Check if it's a blueprint file
                if is_blueprint_file(&asset_path) {
                    // Load the blueprint
                    if let Ok(file) = BlueprintFile::load(&asset_path) {
                        let path_str = asset_path.to_string_lossy().to_string();
                        editor_state.open_blueprints.insert(path_str.clone(), file.graph);
                        editor_state.active_blueprint = Some(path_str);
                    }
                }
                // Check if it's an image file
                else if is_image_file(&asset_path) {
                    // Only allow texture drops in material blueprints
                    let is_material = editor_state.active_graph().map(|g| g.is_material()).unwrap_or(false);

                    if is_material {
                        let canvas_pos = canvas_state.screen_to_canvas(pos, canvas_rect);

                        // Get relative path from project
                        let relative_path = if let Some(project) = current_project {
                            asset_path.strip_prefix(&project.path)
                                .map(|p| p.to_string_lossy().to_string())
                                .unwrap_or_else(|_| asset_path.to_string_lossy().to_string())
                        } else {
                            asset_path.to_string_lossy().to_string()
                        };

                        // Detect texture type from filename
                        let node_type = detect_texture_node_type(&asset_path);

                        if let Some(graph) = editor_state.active_graph_mut() {
                            let node_id = graph.next_node_id();
                            if let Some(mut node) = node_registry.create_node(node_type, node_id) {
                                node.position = [canvas_pos.x, canvas_pos.y];
                                // Set the texture path as input value (hidden, not a pin)
                                node.set_input_value("path".to_string(), PinValue::Texture2D(relative_path.clone()));
                                console_log(LogLevel::Success, "Blueprint", format!("Created {} node with path: {}", node_type, relative_path));
                                graph.add_node(node);
                            }
                        }
                    }
                }
            } else if !mouse_released {
                // Still dragging, put it back
                assets.dragging_asset = Some(asset_path);
            }
        }
    }

    // Draw drag preview for textures and blueprints
    if let Some(asset_path) = &assets.dragging_asset {
        if let Some(pos) = mouse_pos {
            if canvas_rect.contains(pos) {
                let file_name = asset_path.file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("file");

                if is_blueprint_file(asset_path) {
                    // Blueprint file preview
                    let preview_rect = Rect::from_min_size(
                        pos - egui::vec2(60.0, 15.0),
                        egui::vec2(160.0, 40.0),
                    );

                    painter.rect_filled(preview_rect, 4.0, Color32::from_rgba_unmultiplied(40, 45, 55, 200));
                    painter.text(
                        preview_rect.center_top() + egui::vec2(0.0, 12.0),
                        egui::Align2::CENTER_CENTER,
                        format!("\u{1F4DC} {}", truncate_filename(file_name, 18)),
                        egui::FontId::proportional(11.0),
                        Color32::WHITE,
                    );
                    painter.text(
                        preview_rect.center_bottom() - egui::vec2(0.0, 8.0),
                        egui::Align2::CENTER_CENTER,
                        "Drop to open blueprint",
                        egui::FontId::proportional(10.0),
                        Color32::from_rgb(150, 180, 220),
                    );
                } else if is_image_file(asset_path) {
                    // Texture file preview
                    let is_material = editor_state.active_graph().map(|g| g.is_material()).unwrap_or(false);
                    let detected_type = detect_texture_node_type(asset_path);
                    let type_name = texture_type_display_name(detected_type);

                    let (hint_text, hint_color) = if is_material {
                        (format!("Drop to create {} node", type_name), Color32::from_rgb(150, 200, 150))
                    } else {
                        ("Textures only in Material blueprints".to_string(), Color32::from_rgb(200, 150, 150))
                    };

                    let preview_rect = Rect::from_min_size(
                        pos - egui::vec2(70.0, 15.0),
                        egui::vec2(160.0, 40.0),
                    );

                    painter.rect_filled(preview_rect, 4.0, Color32::from_rgba_unmultiplied(40, 40, 45, 200));
                    painter.text(
                        preview_rect.center_top() + egui::vec2(0.0, 12.0),
                        egui::Align2::CENTER_CENTER,
                        format!("\u{1F5BC} {}", truncate_filename(file_name, 18)),
                        egui::FontId::proportional(11.0),
                        Color32::WHITE,
                    );
                    painter.text(
                        preview_rect.center_bottom() - egui::vec2(0.0, 8.0),
                        egui::Align2::CENTER_CENTER,
                        hint_text,
                        egui::FontId::proportional(10.0),
                        hint_color,
                    );
                }
            }
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

        // Two buttons for different blueprint types
        ui.horizontal(|ui| {
            ui.add_space(ui.available_width() / 2.0 - 140.0);

            if ui.button(RichText::new("\u{1F4DC} New Behavior Blueprint").size(13.0)).clicked() {
                let name = format!("blueprint_{}", editor_state.open_blueprints.len() + 1);
                let graph = BlueprintGraph::new(&name);
                let path = format!("blueprints/{}.blueprint", name);
                editor_state.open_blueprints.insert(path.clone(), graph);
                editor_state.active_blueprint = Some(path);
            }

            ui.add_space(8.0);

            if ui.button(RichText::new("\u{1F3A8} New Material Blueprint").size(13.0)).clicked() {
                let name = format!("material_{}", editor_state.open_blueprints.len() + 1);
                let graph = BlueprintGraph::new_material(&name);
                let path = format!("blueprints/{}.material_bp", name);
                editor_state.open_blueprints.insert(path.clone(), graph);
                editor_state.active_blueprint = Some(path);
            }
        });

        ui.add_space(16.0);

        // Description of each type
        ui.label(RichText::new("Behavior: Entity logic (compiles to Rhai script)").size(11.0).weak());
        ui.label(RichText::new("Material: Custom shaders (compiles to WGSL)").size(11.0).weak());

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

                    // Show icon based on extension
                    let is_material = path.extension()
                        .map(|e| e == "material_bp")
                        .unwrap_or(false);
                    let icon = if is_material { "\u{1F3A8}" } else { "\u{1F4DC}" };

                    if ui.button(format!("{} {}", icon, name)).clicked() {
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
        console_log(LogLevel::Error, "Blueprint", "No project open, cannot save blueprint".to_string());
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
        Ok(_) => console_log(LogLevel::Success, "Blueprint", format!("Blueprint saved to {:?}", full_path)),
        Err(e) => console_log(LogLevel::Error, "Blueprint", format!("Failed to save blueprint: {}", e)),
    }
}

use crate::core::resources::console::{console_log, LogLevel};

/// Check if a path is an image file
fn is_image_file(path: &PathBuf) -> bool {
    let ext = path.extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    matches!(ext.as_str(), "png" | "jpg" | "jpeg" | "bmp" | "tga" | "webp" | "hdr" | "exr")
}

/// Truncate a filename for display
fn truncate_filename(name: &str, max_len: usize) -> String {
    if name.len() <= max_len {
        name.to_string()
    } else {
        format!("{}...", &name[..max_len.saturating_sub(3)])
    }
}

/// Draw a checkerboard pattern for transparent image backgrounds
fn draw_checkerboard(painter: &egui::Painter, rect: Rect, zoom: f32) {
    let check_size = (8.0 * zoom).max(4.0);
    let light = Color32::from_rgb(55, 55, 60);
    let dark = Color32::from_rgb(40, 40, 45);

    let cols = (rect.width() / check_size).ceil() as i32;
    let rows = (rect.height() / check_size).ceil() as i32;

    for row in 0..rows {
        for col in 0..cols {
            let color = if (row + col) % 2 == 0 { light } else { dark };
            let check_rect = Rect::from_min_size(
                Pos2::new(rect.min.x + col as f32 * check_size, rect.min.y + row as f32 * check_size),
                egui::vec2(check_size, check_size),
            ).intersect(rect);
            painter.rect_filled(check_rect, 0.0, color);
        }
    }
}

/// Check if a path is a blueprint file
fn is_blueprint_file(path: &PathBuf) -> bool {
    let ext = path.extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    matches!(ext.as_str(), "behavior_bp" | "material_bp")
}

/// Detect the appropriate texture node type based on filename
fn detect_texture_node_type(path: &PathBuf) -> &'static str {
    let filename = path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_lowercase();

    // Check for common texture naming conventions
    if filename.contains("normal") || filename.contains("nrm") || filename.contains("norm") {
        // Detect DX vs GL normal maps
        if filename.contains("dx") || filename.contains("directx") {
            "shader/texture_normal_dx"
        } else if filename.contains("gl") || filename.contains("opengl") {
            "shader/texture_normal_gl"
        } else {
            // Default to OpenGL format (Bevy/glTF standard)
            "shader/texture_normal_gl"
        }
    } else if filename.contains("roughness") || filename.contains("rough") {
        "shader/texture_roughness"
    } else if filename.contains("metallic") || filename.contains("metal") || filename.contains("metalness") {
        "shader/texture_metallic"
    } else if filename.contains("displacement") || filename.contains("disp") || filename.contains("height") || filename.contains("bump") {
        "shader/texture_displacement"
    } else if filename.contains("ao") || filename.contains("occlusion") || filename.contains("ambient") {
        "shader/texture_ao"
    } else if filename.contains("emissive") || filename.contains("emission") || filename.contains("glow") {
        "shader/texture_emissive"
    } else if filename.contains("opacity") || filename.contains("alpha") || filename.contains("transparency") {
        "shader/texture_opacity"
    } else if filename.contains("albedo") || filename.contains("diffuse") || filename.contains("color") || filename.contains("basecolor") || filename.contains("base_color") {
        "shader/texture_color"
    } else {
        // Default to color texture for unrecognized names
        "shader/texture_color"
    }
}

/// Get display name for a texture node type
fn texture_type_display_name(node_type: &str) -> &'static str {
    match node_type {
        "shader/texture_color" => "Color",
        "shader/texture_normal_dx" => "Normal (DX)",
        "shader/texture_normal_gl" => "Normal (GL)",
        "shader/texture_roughness" => "Roughness",
        "shader/texture_metallic" => "Metallic",
        "shader/texture_displacement" => "Displacement",
        "shader/texture_ao" => "AO",
        "shader/texture_emissive" => "Emissive",
        "shader/texture_opacity" => "Opacity",
        "shader/texture" => "Texture",
        _ => "Texture",
    }
}
