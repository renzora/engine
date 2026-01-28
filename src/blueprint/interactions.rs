//! Interaction handling for the blueprint canvas
//!
//! Handles node selection, dragging, connection creation, and input events.

use bevy_egui::egui::{self, Key, Pos2, Rect, Response};
use std::collections::HashMap;

use super::{
    BlueprintCanvasState, BlueprintEditorState, BlueprintGraph, BlueprintNode, NodeId,
    PinDirection, PinId, AddNodePopup,
    canvas::{NODE_PIN_RADIUS, NODE_HEADER_HEIGHT, NODE_PIN_HEIGHT},
    nodes::NodeRegistry,
};

/// Result of processing canvas interactions
pub struct InteractionResult {
    /// Node that was clicked (if any)
    pub clicked_node: Option<NodeId>,
    /// Pin that was clicked (if any)
    pub clicked_pin: Option<(NodeId, String, PinDirection)>,
    /// Whether a connection was completed
    pub connection_completed: Option<(PinId, PinId)>,
    /// Connections to remove (Alt+click or right-click on pin)
    pub connections_to_remove: Vec<PinId>,
    /// Whether the add node popup should open
    pub open_add_popup: bool,
    /// Position for add node popup
    pub popup_position: Option<[f32; 2]>,
    /// Whether selection changed
    pub selection_changed: bool,
    /// Nodes to delete
    pub nodes_to_delete: Vec<NodeId>,
}

impl Default for InteractionResult {
    fn default() -> Self {
        Self {
            clicked_node: None,
            clicked_pin: None,
            connection_completed: None,
            connections_to_remove: Vec::new(),
            open_add_popup: false,
            popup_position: None,
            selection_changed: false,
            nodes_to_delete: Vec::new(),
        }
    }
}

/// Process canvas interactions and return results
pub fn process_canvas_interactions(
    ui: &mut egui::Ui,
    response: &Response,
    canvas: &mut BlueprintCanvasState,
    state: &mut BlueprintEditorState,
    graph: &BlueprintGraph,
    node_rects: &HashMap<NodeId, Rect>,
    canvas_rect: Rect,
) -> InteractionResult {
    let mut result = InteractionResult::default();

    let mouse_pos = ui.input(|i| i.pointer.hover_pos()).unwrap_or(Pos2::ZERO);
    let canvas_mouse = canvas.screen_to_canvas(mouse_pos, canvas_rect);

    // Handle zoom with scroll wheel
    if response.hovered() {
        let scroll_delta = ui.input(|i| i.raw_scroll_delta.y);
        if scroll_delta.abs() > 0.0 {
            canvas.zoom_at(mouse_pos, canvas_rect, scroll_delta * 0.01);
        }
    }

    // Handle panning
    let middle_pressed = ui.input(|i| i.pointer.middle_down());
    let space_held = ui.input(|i| i.key_down(Key::Space));

    if middle_pressed || (space_held && response.dragged()) {
        state.panning = true;
    }

    if state.panning {
        let delta = ui.input(|i| i.pointer.delta());
        canvas.pan(delta);

        if !middle_pressed && !space_held {
            state.panning = false;
        }
    }

    // Handle keyboard shortcuts
    let modifiers = ui.input(|i| i.modifiers);

    // Delete key - delete selected nodes
    if ui.input(|i| i.key_pressed(Key::Delete) || i.key_pressed(Key::Backspace)) {
        result.nodes_to_delete = state.selected_nodes.clone();
    }

    // Ctrl+A - select all
    if modifiers.ctrl && ui.input(|i| i.key_pressed(Key::A)) {
        state.selected_nodes = graph.nodes.iter().map(|n| n.id).collect();
        result.selection_changed = true;
    }

    // Escape - cancel connection or clear selection
    if ui.input(|i| i.key_pressed(Key::Escape)) {
        if state.creating_connection.is_some() {
            state.creating_connection = None;
        } else if state.add_node_popup.is_some() {
            state.add_node_popup = None;
        } else {
            state.clear_selection();
            result.selection_changed = true;
        }
    }

    // Don't process click interactions while panning
    if state.panning {
        return result;
    }

    // Check for pin clicks first (higher priority than node clicks)
    // Alt+click or right-click on a pin disconnects it
    let alt_held = modifiers.alt;
    let right_clicked = response.secondary_clicked();

    if response.clicked() || response.drag_started() || right_clicked {
        for (&node_id, &node_rect) in node_rects {
            if let Some(node) = graph.get_node(node_id) {
                if let Some((pin_name, direction)) = find_clicked_pin(
                    mouse_pos,
                    node,
                    node_rect,
                    canvas.zoom,
                ) {
                    result.clicked_pin = Some((node_id, pin_name.clone(), direction));
                    let pin_id = PinId {
                        node_id,
                        pin_name: pin_name.clone(),
                        direction,
                    };

                    // Alt+click or right-click on pin = disconnect
                    if alt_held || right_clicked {
                        // Check if this pin has connections and queue them for removal
                        let is_connected = graph.is_pin_connected(&pin_id);
                        if is_connected {
                            result.connections_to_remove.push(pin_id);
                            state.creating_connection = None;
                            return result;
                        }
                    }

                    // Normal click: Start or complete connection
                    if let Some(from_pin) = &state.creating_connection {
                        // Complete connection - determine correct direction (from output to input)
                        if direction == PinDirection::Input && from_pin.node_id != node_id {
                            let to_pin = PinId::input(node_id, pin_name);
                            result.connection_completed = Some((from_pin.clone(), to_pin));
                        } else if direction == PinDirection::Output && from_pin.node_id != node_id {
                            let from_output = PinId::output(node_id, pin_name);
                            result.connection_completed = Some((from_output, from_pin.clone()));
                        }
                        state.creating_connection = None;
                    } else {
                        // Start connection with correct direction
                        state.creating_connection = Some(pin_id);
                    }

                    return result;
                }
            }
        }
    }

    // Handle node interactions
    if response.clicked() {
        let mut clicked_on_node = false;

        // Check nodes in reverse order (top nodes first)
        for (&node_id, &node_rect) in node_rects.iter() {
            if node_rect.contains(mouse_pos) {
                clicked_on_node = true;
                result.clicked_node = Some(node_id);

                if modifiers.ctrl {
                    // Ctrl+click toggles selection
                    state.toggle_node_selection(node_id);
                } else if !state.is_node_selected(node_id) {
                    // Click selects (unless already selected)
                    state.select_node(node_id);
                }
                result.selection_changed = true;
                break;
            }
        }

        // Clear selection if clicked on empty space
        if !clicked_on_node && !modifiers.ctrl {
            state.clear_selection();
            state.creating_connection = None;
            result.selection_changed = true;
        }
    }

    // Start dragging selected nodes
    if response.drag_started() {
        for (&node_id, &node_rect) in node_rects.iter() {
            if node_rect.contains(mouse_pos) {
                if !state.is_node_selected(node_id) {
                    state.select_node(node_id);
                    result.selection_changed = true;
                }
                state.dragging_node = Some(node_id);

                // Calculate offset from node position to mouse
                if let Some(node) = graph.get_node(node_id) {
                    let node_screen = canvas.canvas_to_screen(node.position, canvas_rect);
                    state.drag_offset = [mouse_pos.x - node_screen.x, mouse_pos.y - node_screen.y];
                }
                break;
            }
        }

        // Start box selection if not on a node
        if state.dragging_node.is_none() && state.creating_connection.is_none() {
            state.box_select_start = Some(mouse_pos);
            state.box_select_end = Some(mouse_pos);
        }
    }

    // Update box selection
    if let Some(_start) = state.box_select_start {
        if response.dragged() {
            state.box_select_end = Some(mouse_pos);
        }
    }

    // End box selection
    if response.drag_stopped() {
        if let (Some(start), Some(end)) = (state.box_select_start, state.box_select_end) {
            let select_rect = Rect::from_two_pos(start, end);

            // Select all nodes within the box
            if !modifiers.ctrl {
                state.clear_selection();
            }

            for (&node_id, &node_rect) in node_rects.iter() {
                if select_rect.intersects(node_rect) {
                    if !state.selected_nodes.contains(&node_id) {
                        state.selected_nodes.push(node_id);
                    }
                }
            }
            result.selection_changed = true;
        }

        state.box_select_start = None;
        state.box_select_end = None;
        state.dragging_node = None;
    }

    // Right-click to open add node popup
    if response.secondary_clicked() {
        result.open_add_popup = true;
        result.popup_position = Some([canvas_mouse.x, canvas_mouse.y]);

        // If creating a connection, pass it to the popup for auto-connect
        if let Some(from_pin) = state.creating_connection.take() {
            state.add_node_popup = Some(AddNodePopup {
                search: String::new(),
                position: [canvas_mouse.x, canvas_mouse.y],
                connecting_from: Some(from_pin),
            });
        } else {
            state.add_node_popup = Some(AddNodePopup {
                search: String::new(),
                position: [canvas_mouse.x, canvas_mouse.y],
                connecting_from: None,
            });
        }
    }

    result
}

/// Find which pin (if any) was clicked
fn find_clicked_pin(
    mouse_pos: Pos2,
    node: &BlueprintNode,
    node_rect: Rect,
    zoom: f32,
) -> Option<(String, PinDirection)> {
    let pin_radius = NODE_PIN_RADIUS * zoom * 1.5; // Larger hit area
    let pin_start_y = node_rect.top() + NODE_HEADER_HEIGHT * zoom + 4.0 * zoom;
    let pin_spacing = NODE_PIN_HEIGHT * zoom;

    // Check input pins (left side)
    let mut input_y = pin_start_y;
    for pin in node.input_pins() {
        let pin_pos = Pos2::new(node_rect.left(), input_y + pin_spacing / 2.0);
        let dist = mouse_pos.distance(pin_pos);
        if dist <= pin_radius {
            return Some((pin.name.clone(), PinDirection::Input));
        }
        input_y += pin_spacing;
    }

    // Check output pins (right side)
    let mut output_y = pin_start_y;
    for pin in node.output_pins() {
        let pin_pos = Pos2::new(node_rect.right(), output_y + pin_spacing / 2.0);
        let dist = mouse_pos.distance(pin_pos);
        if dist <= pin_radius {
            return Some((pin.name.clone(), PinDirection::Output));
        }
        output_y += pin_spacing;
    }

    None
}

/// Apply node dragging to selected nodes
#[allow(dead_code)]
pub fn apply_node_drag(
    state: &mut BlueprintEditorState,
    graph: &mut BlueprintGraph,
    canvas: &BlueprintCanvasState,
    canvas_rect: Rect,
    mouse_pos: Pos2,
) {
    if let Some(dragging_id) = state.dragging_node {
        // Calculate new canvas position
        let target_screen = Pos2::new(
            mouse_pos.x - state.drag_offset[0],
            mouse_pos.y - state.drag_offset[1],
        );
        let target_canvas = canvas.screen_to_canvas(target_screen, canvas_rect);

        // Get current position of dragging node
        if let Some(dragging_node) = graph.get_node(dragging_id) {
            let delta_x = target_canvas.x - dragging_node.position[0];
            let delta_y = target_canvas.y - dragging_node.position[1];

            // Move all selected nodes by the same delta
            let selected = state.selected_nodes.clone();
            for node_id in selected {
                if let Some(node) = graph.get_node_mut(node_id) {
                    node.position[0] += delta_x;
                    node.position[1] += delta_y;
                }
            }
        }
    }
}

/// Render the add node popup
pub fn render_add_node_popup(
    ui: &mut egui::Ui,
    state: &mut BlueprintEditorState,
    registry: &NodeRegistry,
    canvas: &BlueprintCanvasState,
    canvas_rect: Rect,
) -> Option<(String, [f32; 2], Option<PinId>)> {
    let mut result = None;
    let mut should_close = false;

    // Extract popup data before borrowing
    let popup_data = state.add_node_popup.as_ref().map(|p| {
        (p.position, p.search.clone(), p.connecting_from.clone())
    });

    if let Some((position, mut search, connecting_from)) = popup_data {
        let screen_pos = canvas.canvas_to_screen(position, canvas_rect);

        egui::Area::new(egui::Id::new("add_node_popup"))
            .fixed_pos(screen_pos)
            .order(egui::Order::Foreground)
            .show(ui.ctx(), |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.set_min_width(200.0);

                    // Search box
                    let search_response = ui.text_edit_singleline(&mut search);
                    if search_response.lost_focus() && ui.input(|i| i.key_pressed(Key::Escape)) {
                        should_close = true;
                        return;
                    }

                    // Request focus on first frame
                    if search_response.gained_focus() || !search_response.has_focus() {
                        search_response.request_focus();
                    }

                    ui.separator();

                    // Category list
                    egui::ScrollArea::vertical()
                        .max_height(300.0)
                        .show(ui, |ui| {
                            let search_lower = search.to_lowercase();

                            // Sort categories
                            let mut categories: Vec<_> = registry.categories().collect();
                            categories.sort();

                            for category in categories {
                                if let Some(nodes) = registry.nodes_in_category(category) {
                                    // Filter nodes by search
                                    let filtered: Vec<_> = nodes
                                        .iter()
                                        .filter(|n| {
                                            search_lower.is_empty()
                                                || n.display_name.to_lowercase().contains(&search_lower)
                                                || n.type_id.to_lowercase().contains(&search_lower)
                                        })
                                        .collect();

                                    if filtered.is_empty() {
                                        continue;
                                    }

                                    ui.collapsing(category, |ui| {
                                        for node_def in filtered {
                                            if ui
                                                .selectable_label(false, node_def.display_name)
                                                .on_hover_text(node_def.description)
                                                .clicked()
                                            {
                                                result = Some((
                                                    node_def.type_id.to_string(),
                                                    position,
                                                    connecting_from.clone(),
                                                ));
                                            }
                                        }
                                    });
                                }
                            }
                        });
                });
            });

        // Update search text back to state
        if let Some(popup) = &mut state.add_node_popup {
            popup.search = search;
        }
    }

    if result.is_some() || should_close {
        state.add_node_popup = None;
    }

    result
}
