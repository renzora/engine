//! Canvas rendering for the blueprint editor
//!
//! Handles pan/zoom, grid drawing, node rendering, and bezier connections.

use bevy::prelude::*;
use bevy_egui::egui::{self, Color32, CornerRadius, Pos2, Rect, Stroke, StrokeKind, Vec2 as EguiVec2};
use std::collections::HashMap;

use super::{
    BlueprintGraph, BlueprintNode, NodeId, Pin, PinDirection, PinId, PinType,
};

/// State for the blueprint canvas (pan, zoom, etc.)
#[derive(Resource, Default)]
pub struct BlueprintCanvasState {
    /// Canvas offset (pan position)
    pub offset: [f32; 2],
    /// Zoom level (0.25 to 4.0)
    pub zoom: f32,
}

impl BlueprintCanvasState {
    #[allow(dead_code)]
    pub fn new() -> Self {
        Self {
            offset: [0.0, 0.0],
            zoom: 1.0,
        }
    }

    /// Convert screen position to canvas position
    pub fn screen_to_canvas(&self, screen_pos: Pos2, canvas_rect: Rect) -> Pos2 {
        let center = canvas_rect.center();
        let relative = screen_pos - center;
        Pos2::new(
            relative.x / self.zoom - self.offset[0],
            relative.y / self.zoom - self.offset[1],
        )
    }

    /// Convert canvas position to screen position
    pub fn canvas_to_screen(&self, canvas_pos: [f32; 2], canvas_rect: Rect) -> Pos2 {
        let center = canvas_rect.center();
        Pos2::new(
            center.x + (canvas_pos[0] + self.offset[0]) * self.zoom,
            center.y + (canvas_pos[1] + self.offset[1]) * self.zoom,
        )
    }

    /// Apply zoom at a specific screen position
    pub fn zoom_at(&mut self, screen_pos: Pos2, canvas_rect: Rect, delta: f32) {
        let old_zoom = self.zoom;
        self.zoom = (self.zoom * (1.0 + delta * 0.1)).clamp(0.25, 4.0);

        // Adjust offset to keep mouse position stationary
        if (self.zoom - old_zoom).abs() > 0.001 {
            let center = canvas_rect.center();
            let relative = screen_pos - center;
            let canvas_pos_before = [
                relative.x / old_zoom - self.offset[0],
                relative.y / old_zoom - self.offset[1],
            ];
            let canvas_pos_after = [
                relative.x / self.zoom - self.offset[0],
                relative.y / self.zoom - self.offset[1],
            ];
            self.offset[0] -= canvas_pos_before[0] - canvas_pos_after[0];
            self.offset[1] -= canvas_pos_before[1] - canvas_pos_after[1];
        }
    }

    /// Pan the canvas
    pub fn pan(&mut self, delta: EguiVec2) {
        self.offset[0] += delta.x / self.zoom;
        self.offset[1] += delta.y / self.zoom;
    }
}

/// State for the blueprint editor
#[derive(Resource)]
pub struct BlueprintEditorState {
    /// Currently open blueprints (path -> graph)
    pub open_blueprints: HashMap<String, BlueprintGraph>,
    /// Active blueprint path (tab)
    pub active_blueprint: Option<String>,
    /// Selected nodes
    pub selected_nodes: Vec<NodeId>,
    /// Node being dragged
    pub dragging_node: Option<NodeId>,
    /// Drag start offset from node position
    pub drag_offset: [f32; 2],
    /// Connection being created (from pin)
    pub creating_connection: Option<PinId>,
    /// Whether middle mouse or space is held for panning
    pub panning: bool,
    /// Box selection start position (screen coords)
    pub box_select_start: Option<Pos2>,
    /// Box selection current position (screen coords)
    pub box_select_end: Option<Pos2>,
    /// Node search/add popup state
    pub add_node_popup: Option<AddNodePopup>,
    /// Cached pin positions for the current frame (for connection drawing)
    pub pin_positions: HashMap<PinId, Pos2>,
    /// Comparison mode for Compare nodes
    #[allow(dead_code)]
    pub compare_modes: HashMap<NodeId, String>,
    /// Key names for IsKeyPressed nodes
    #[allow(dead_code)]
    pub key_names: HashMap<NodeId, String>,
    /// Variable names for Get/Set Variable nodes
    #[allow(dead_code)]
    pub var_names: HashMap<NodeId, String>,
    /// Search query for the Node Library panel
    pub node_search: String,
    /// Node type being dragged from library (for drag-and-drop)
    pub dragging_new_node: Option<String>,
}

/// State for the add node popup
pub struct AddNodePopup {
    /// Search query
    pub search: String,
    /// Position where the popup was opened (canvas coords)
    pub position: [f32; 2],
    /// If connecting from a pin, store it here for auto-connect
    pub connecting_from: Option<PinId>,
}

impl Default for BlueprintEditorState {
    fn default() -> Self {
        Self {
            open_blueprints: HashMap::new(),
            active_blueprint: None,
            selected_nodes: Vec::new(),
            dragging_node: None,
            drag_offset: [0.0, 0.0],
            creating_connection: None,
            panning: false,
            box_select_start: None,
            box_select_end: None,
            add_node_popup: None,
            pin_positions: HashMap::new(),
            compare_modes: HashMap::new(),
            key_names: HashMap::new(),
            var_names: HashMap::new(),
            node_search: String::new(),
            dragging_new_node: None,
        }
    }
}

impl BlueprintEditorState {
    /// Get the active blueprint graph
    pub fn active_graph(&self) -> Option<&BlueprintGraph> {
        self.active_blueprint.as_ref().and_then(|path| self.open_blueprints.get(path))
    }

    /// Get the active blueprint graph mutably
    pub fn active_graph_mut(&mut self) -> Option<&mut BlueprintGraph> {
        if let Some(path) = &self.active_blueprint {
            self.open_blueprints.get_mut(path)
        } else {
            None
        }
    }

    /// Create a new empty blueprint
    pub fn create_new_blueprint(&mut self, name: &str) -> String {
        let path = format!("blueprints/{}.blueprint", name);
        let graph = BlueprintGraph::new(name);
        self.open_blueprints.insert(path.clone(), graph);
        self.active_blueprint = Some(path.clone());
        path
    }

    /// Clear selection
    pub fn clear_selection(&mut self) {
        self.selected_nodes.clear();
    }

    /// Select a single node
    pub fn select_node(&mut self, node_id: NodeId) {
        self.selected_nodes.clear();
        self.selected_nodes.push(node_id);
    }

    /// Toggle node selection (for Ctrl+click)
    pub fn toggle_node_selection(&mut self, node_id: NodeId) {
        if let Some(idx) = self.selected_nodes.iter().position(|&id| id == node_id) {
            self.selected_nodes.remove(idx);
        } else {
            self.selected_nodes.push(node_id);
        }
    }

    /// Check if a node is selected
    pub fn is_node_selected(&self, node_id: NodeId) -> bool {
        self.selected_nodes.contains(&node_id)
    }
}

// ============================================================================
// RENDERING CONSTANTS
// ============================================================================

/// Node dimensions
pub const NODE_WIDTH: f32 = 180.0;
pub const NODE_HEADER_HEIGHT: f32 = 28.0;
pub const NODE_PIN_HEIGHT: f32 = 22.0;
pub const NODE_PIN_RADIUS: f32 = 6.0;
pub const NODE_CORNER_RADIUS: f32 = 4.0;

/// Colors
pub const NODE_BG_COLOR: Color32 = Color32::from_rgb(40, 40, 45);
pub const NODE_BORDER_COLOR: Color32 = Color32::from_rgb(60, 60, 65);
pub const NODE_SELECTED_BORDER: Color32 = Color32::from_rgb(100, 150, 255);
pub const GRID_DOT_COLOR: Color32 = Color32::from_rgb(60, 60, 65);
pub const CONNECTION_COLOR: Color32 = Color32::from_rgb(200, 200, 200);

// ============================================================================
// GRID RENDERING
// ============================================================================

/// Draw the background grid
pub fn draw_grid(painter: &egui::Painter, rect: Rect, canvas: &BlueprintCanvasState) {
    let grid_spacing = 20.0 * canvas.zoom;
    let dot_size = 1.5 * canvas.zoom.sqrt();

    // Calculate grid offset based on pan
    let offset_x = (canvas.offset[0] * canvas.zoom) % grid_spacing;
    let offset_y = (canvas.offset[1] * canvas.zoom) % grid_spacing;

    let start_x = rect.left() + offset_x;
    let start_y = rect.top() + offset_y;

    let mut x = start_x;
    while x < rect.right() {
        let mut y = start_y;
        while y < rect.bottom() {
            painter.circle_filled(Pos2::new(x, y), dot_size, GRID_DOT_COLOR);
            y += grid_spacing;
        }
        x += grid_spacing;
    }
}

// ============================================================================
// NODE RENDERING
// ============================================================================

/// Draw a single node and return pin positions
pub fn draw_node(
    _ui: &mut egui::Ui,
    painter: &egui::Painter,
    node: &BlueprintNode,
    node_def: Option<&super::nodes::NodeTypeDefinition>,
    canvas: &BlueprintCanvasState,
    canvas_rect: Rect,
    is_selected: bool,
    _state: &mut BlueprintEditorState,
) -> (Rect, HashMap<(String, PinDirection), Pos2>) {
    let mut pin_positions = HashMap::new();

    // Calculate screen position
    let screen_pos = canvas.canvas_to_screen(node.position, canvas_rect);

    // Calculate node height based on pins
    let input_count = node.input_pins().count();
    let output_count = node.output_pins().count();
    let max_pins = input_count.max(output_count);
    let node_height = NODE_HEADER_HEIGHT + (max_pins as f32 * NODE_PIN_HEIGHT) + 8.0;

    let scaled_width = NODE_WIDTH * canvas.zoom;
    let scaled_height = node_height * canvas.zoom;

    let node_rect = Rect::from_min_size(screen_pos, EguiVec2::new(scaled_width, scaled_height));

    // Skip if not visible
    if !canvas_rect.intersects(node_rect) {
        return (node_rect, pin_positions);
    }

    // Draw node background
    painter.rect_filled(node_rect, NODE_CORNER_RADIUS * canvas.zoom, NODE_BG_COLOR);

    // Draw header
    let header_color = if let Some(def) = node_def {
        Color32::from_rgb(def.color[0], def.color[1], def.color[2])
    } else {
        Color32::from_rgb(100, 100, 100)
    };

    let header_rect = Rect::from_min_size(
        screen_pos,
        EguiVec2::new(scaled_width, NODE_HEADER_HEIGHT * canvas.zoom),
    );

    let corner_r = (NODE_CORNER_RADIUS * canvas.zoom) as u8;
    painter.rect_filled(
        header_rect,
        CornerRadius { nw: corner_r, ne: corner_r, sw: 0, se: 0 },
        header_color,
    );

    // Draw title
    let title = node_def.map(|d| d.display_name).unwrap_or(&node.node_type);
    let font_size = 12.0 * canvas.zoom;
    painter.text(
        header_rect.center(),
        egui::Align2::CENTER_CENTER,
        title,
        egui::FontId::proportional(font_size),
        Color32::WHITE,
    );

    // Draw border
    let border_color = if is_selected {
        NODE_SELECTED_BORDER
    } else {
        NODE_BORDER_COLOR
    };
    painter.rect_stroke(node_rect, CornerRadius::same(corner_r), Stroke::new(1.5 * canvas.zoom, border_color), StrokeKind::Outside);

    // Draw pins
    let pin_start_y = screen_pos.y + NODE_HEADER_HEIGHT * canvas.zoom + 4.0 * canvas.zoom;
    let pin_spacing = NODE_PIN_HEIGHT * canvas.zoom;
    let pin_radius = NODE_PIN_RADIUS * canvas.zoom;

    // Input pins (left side)
    let mut input_y = pin_start_y;
    for pin in node.input_pins() {
        let pin_pos = Pos2::new(screen_pos.x, input_y + pin_spacing / 2.0);

        // Draw pin
        draw_pin(painter, pin_pos, pin, pin_radius, canvas.zoom);

        // Draw label
        let label_pos = Pos2::new(pin_pos.x + pin_radius * 2.0, pin_pos.y);
        if !pin.label.is_empty() {
            painter.text(
                label_pos,
                egui::Align2::LEFT_CENTER,
                &pin.label,
                egui::FontId::proportional(10.0 * canvas.zoom),
                Color32::from_rgb(200, 200, 200),
            );
        }

        // Store pin position for connections (with direction to distinguish same-named pins)
        pin_positions.insert((pin.name.clone(), PinDirection::Input), pin_pos);

        input_y += pin_spacing;
    }

    // Output pins (right side)
    let mut output_y = pin_start_y;
    for pin in node.output_pins() {
        let pin_pos = Pos2::new(screen_pos.x + scaled_width, output_y + pin_spacing / 2.0);

        // Draw pin
        draw_pin(painter, pin_pos, pin, pin_radius, canvas.zoom);

        // Draw label
        let label_pos = Pos2::new(pin_pos.x - pin_radius * 2.0, pin_pos.y);
        if !pin.label.is_empty() {
            painter.text(
                label_pos,
                egui::Align2::RIGHT_CENTER,
                &pin.label,
                egui::FontId::proportional(10.0 * canvas.zoom),
                Color32::from_rgb(200, 200, 200),
            );
        }

        // Store pin position for connections (with direction to distinguish same-named pins)
        pin_positions.insert((pin.name.clone(), PinDirection::Output), pin_pos);

        output_y += pin_spacing;
    }

    (node_rect, pin_positions)
}

/// Draw a pin (circle or triangle for flow)
fn draw_pin(painter: &egui::Painter, pos: Pos2, pin: &Pin, radius: f32, zoom: f32) {
    let color = Color32::from_rgb(
        pin.pin_type.color()[0],
        pin.pin_type.color()[1],
        pin.pin_type.color()[2],
    );

    if pin.pin_type == PinType::Flow {
        // Draw triangle for flow pins
        let size = radius * 0.8;
        let points = if pin.direction == PinDirection::Output {
            // Right-pointing triangle
            vec![
                Pos2::new(pos.x - size, pos.y - size),
                Pos2::new(pos.x + size, pos.y),
                Pos2::new(pos.x - size, pos.y + size),
            ]
        } else {
            // Right-pointing triangle (input still receives flow)
            vec![
                Pos2::new(pos.x - size, pos.y - size),
                Pos2::new(pos.x + size, pos.y),
                Pos2::new(pos.x - size, pos.y + size),
            ]
        };
        painter.add(egui::Shape::convex_polygon(points, color, Stroke::NONE));
    } else {
        // Draw circle for data pins
        painter.circle_filled(pos, radius, color);
        painter.circle_stroke(pos, radius, Stroke::new(1.0 * zoom, Color32::from_rgb(80, 80, 85)));
    }
}

// ============================================================================
// CONNECTION RENDERING
// ============================================================================

/// Draw a bezier connection between two pins
pub fn draw_connection(
    painter: &egui::Painter,
    from_pos: Pos2,
    to_pos: Pos2,
    color: Color32,
    zoom: f32,
) {
    let control_dist = ((to_pos.x - from_pos.x).abs() * 0.5).max(50.0 * zoom);

    let cp1 = Pos2::new(from_pos.x + control_dist, from_pos.y);
    let cp2 = Pos2::new(to_pos.x - control_dist, to_pos.y);

    let bezier = egui::epaint::CubicBezierShape::from_points_stroke(
        [from_pos, cp1, cp2, to_pos],
        false,
        Color32::TRANSPARENT,
        Stroke::new(2.0 * zoom, color),
    );

    painter.add(bezier);
}

/// Draw all connections for a graph
pub fn draw_connections(
    painter: &egui::Painter,
    graph: &BlueprintGraph,
    pin_positions: &HashMap<PinId, Pos2>,
    zoom: f32,
) {
    for conn in &graph.connections {
        // Connections: from is always output, to is always input
        let from_key = PinId::output(conn.from.node_id, &conn.from.pin_name);
        let to_key = PinId::input(conn.to.node_id, &conn.to.pin_name);

        if let (Some(&from_pos), Some(&to_pos)) = (
            pin_positions.get(&from_key),
            pin_positions.get(&to_key),
        ) {
            // Get pin type for color
            let color = graph
                .get_node(conn.from.node_id)
                .and_then(|n| n.get_output_pin(&conn.from.pin_name))
                .map(|p| {
                    let c = p.pin_type.color();
                    Color32::from_rgb(c[0], c[1], c[2])
                })
                .unwrap_or(CONNECTION_COLOR);

            draw_connection(painter, from_pos, to_pos, color, zoom);
        }
    }
}

/// Draw connection being created
pub fn draw_pending_connection(
    painter: &egui::Painter,
    from_pos: Pos2,
    to_pos: Pos2,
    zoom: f32,
) {
    draw_connection(painter, from_pos, to_pos, Color32::from_rgb(255, 255, 100), zoom);
}

// ============================================================================
// BOX SELECTION
// ============================================================================

/// Draw box selection overlay
pub fn draw_box_selection(painter: &egui::Painter, start: Pos2, end: Pos2) {
    let rect = Rect::from_two_pos(start, end);
    painter.rect_filled(rect, CornerRadius::ZERO, Color32::from_rgba_unmultiplied(100, 150, 255, 30));
    painter.rect_stroke(rect, CornerRadius::ZERO, Stroke::new(1.0, Color32::from_rgb(100, 150, 255)), StrokeKind::Outside);
}
