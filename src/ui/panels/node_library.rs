//! Node Library panel for browsing blueprint nodes

use bevy_egui::egui::{self, Color32, CursorIcon, RichText, ScrollArea, Sense, Vec2};

use crate::blueprint::{BlueprintEditorState, BlueprintCanvasState, BlueprintType};
use crate::blueprint::nodes::{NodeRegistry, NodeEntry};

/// Render the node library panel
pub fn render_node_library_panel(
    ui: &mut egui::Ui,
    editor_state: &mut BlueprintEditorState,
    canvas_state: &BlueprintCanvasState,
    node_registry: &NodeRegistry,
) {
    ui.vertical(|ui| {
        // Search box
        ui.horizontal(|ui| {
            ui.label("Search:");
            ui.add(egui::TextEdit::singleline(&mut editor_state.node_search)
                .desired_width(ui.available_width() - 10.0)
                .hint_text("Filter nodes..."));
        });

        ui.separator();

        // Hint text
        ui.label(RichText::new("Drag nodes to canvas").weak().italics().size(10.0));
        ui.add_space(4.0);

        // Node categories
        ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let search_lower = editor_state.node_search.to_lowercase();

                // Get the current graph type for filtering
                let graph_type = editor_state.active_graph()
                    .map(|g| g.graph_type)
                    .unwrap_or(BlueprintType::Behavior);

                // Sort categories
                let mut categories: Vec<_> = node_registry.categories().collect();
                categories.sort();

                for category in categories {
                    // Filter categories based on blueprint type
                    if !graph_type.is_category_allowed(category) {
                        continue;
                    }

                    if let Some(entries) = node_registry.entries_in_category(category) {
                        // Collect matching entries as (type_id, display_name, description)
                        let filtered: Vec<(String, String, String)> = entries
                            .iter()
                            .filter_map(|entry| {
                                let (tid, dname, desc) = match entry {
                                    NodeEntry::Static(def) => (def.type_id.to_string(), def.display_name.to_string(), def.description.to_string()),
                                    NodeEntry::Dynamic(key) => {
                                        let cdef = node_registry.get_component_node(key)?;
                                        (cdef.type_id.clone(), cdef.display_name.clone(), cdef.description.clone())
                                    }
                                };
                                if search_lower.is_empty()
                                    || dname.to_lowercase().contains(&search_lower)
                                    || tid.to_lowercase().contains(&search_lower)
                                {
                                    Some((tid, dname, desc))
                                } else {
                                    None
                                }
                            })
                            .collect();

                        if filtered.is_empty() {
                            continue;
                        }

                        // Category header with colored accent
                        let header_color = get_category_color(category);
                        ui.horizontal(|ui| {
                            ui.add_space(4.0);
                            let rect = ui.available_rect_before_wrap();
                            let indicator_rect = egui::Rect::from_min_size(
                                rect.min,
                                egui::vec2(3.0, 16.0),
                            );
                            ui.painter().rect_filled(indicator_rect, 1.0, header_color);
                            ui.add_space(8.0);
                            ui.label(RichText::new(category).strong());
                        });

                        ui.indent(category, |ui| {
                            for (type_id, display_name, description) in &filtered {
                                // Create a draggable button
                                let button_size = Vec2::new(ui.available_width() - 8.0, 24.0);
                                let (rect, response) = ui.allocate_exact_size(button_size, Sense::click_and_drag());

                                // Visual feedback
                                let is_being_dragged = editor_state.dragging_new_node.as_deref() == Some(type_id.as_str());
                                let is_hovered = response.hovered();
                                let bg_color = if is_being_dragged {
                                    Color32::from_rgb(80, 100, 140)
                                } else if is_hovered {
                                    Color32::from_rgb(60, 62, 68)
                                } else {
                                    Color32::from_rgb(50, 52, 58)
                                };

                                // Show pointer cursor on hover
                                if is_hovered {
                                    ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
                                }

                                ui.painter().rect_filled(rect, 4.0, bg_color);
                                ui.painter().text(
                                    rect.left_center() + egui::vec2(8.0, 0.0),
                                    egui::Align2::LEFT_CENTER,
                                    display_name,
                                    egui::FontId::proportional(12.0),
                                    Color32::from_rgb(220, 220, 220),
                                );

                                // Show tooltip on hover (only when not dragging)
                                if !is_being_dragged {
                                    response.clone().on_hover_ui(|ui| {
                                        ui.label(RichText::new(display_name.as_str()).strong());
                                        ui.label(description.as_str());
                                    });
                                }

                                // Start drag
                                if response.drag_started() {
                                    editor_state.dragging_new_node = Some(type_id.clone());
                                }

                                // Double-click to add at center
                                if response.double_clicked() {
                                    add_node_to_canvas(editor_state, node_registry, type_id, canvas_state);
                                }
                            }
                        });

                        ui.add_space(4.0);
                    }
                }
            });
    });

    // Clear drag if mouse released (will be handled by blueprint panel if dropped there)
    if ui.input(|i| i.pointer.any_released()) {
        // Don't clear here - let the blueprint panel handle the drop
    }
}

/// Get a color for a category
fn get_category_color(category: &str) -> Color32 {
    match category {
        // Behavior blueprint categories
        "Events" => Color32::from_rgb(200, 50, 50),
        "Math" => Color32::from_rgb(100, 200, 100),
        "Logic" => Color32::from_rgb(200, 100, 100),
        "Transform" => Color32::from_rgb(200, 150, 100),
        "Input" => Color32::from_rgb(100, 150, 200),
        "Utility" => Color32::from_rgb(150, 150, 150),
        "Variables" => Color32::from_rgb(150, 100, 200),
        "Time" => Color32::from_rgb(200, 200, 100),
        // Material/Shader blueprint categories
        "Shader Input" => Color32::from_rgb(100, 150, 220),
        "Shader Texture" => Color32::from_rgb(150, 120, 200),
        "Shader Math" => Color32::from_rgb(120, 180, 120),
        "Shader Vector" => Color32::from_rgb(200, 180, 100),
        "Shader Output" => Color32::from_rgb(220, 80, 80),
        "Shader Noise" => Color32::from_rgb(180, 140, 200),
        "Components" => Color32::from_rgb(100, 180, 220),
        _ => Color32::from_rgb(100, 100, 100),
    }
}

/// Add a node to the active canvas at a default position
fn add_node_to_canvas(
    editor_state: &mut BlueprintEditorState,
    node_registry: &NodeRegistry,
    type_id: &str,
    canvas_state: &BlueprintCanvasState,
) {
    if let Some(graph) = editor_state.active_graph_mut() {
        let node_id = graph.next_node_id();
        if let Some(mut node) = node_registry.create_node(type_id, node_id) {
            // Place at center of view
            let center_x = -canvas_state.offset[0] + 200.0;
            let center_y = -canvas_state.offset[1] + 200.0;
            node.position = [center_x, center_y];
            graph.add_node(node);
        }
    }
}
