//! Canvas rendering for the blueprint editor
//!
//! Handles pan/zoom, grid drawing, node rendering, and bezier connections.

use bevy::prelude::*;
use bevy_egui::egui::{self, Color32, CornerRadius, Pos2, Rect, Stroke, StrokeKind, Vec2 as EguiVec2};
use std::collections::HashMap;

use std::collections::HashSet;

use super::{
    BlueprintGraph, BlueprintNode, NodeId, Pin, PinDirection, PinId, PinType, PinValue,
};

/// Check if a node type is a texture node (has preview)
pub fn is_texture_node(node_type: &str) -> bool {
    matches!(node_type,
        "shader/texture" |
        "shader/texture_color" |
        "shader/texture_normal_dx" |
        "shader/texture_normal_gl" |
        "shader/texture_roughness" |
        "shader/texture_metallic" |
        "shader/texture_displacement" |
        "shader/texture_ao" |
        "shader/texture_emissive" |
        "shader/texture_opacity"
    )
}

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
    #[allow(dead_code)]
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

/// Value change from an inline editor
#[derive(Debug, Clone)]
pub struct NodeValueChange {
    pub node_id: NodeId,
    pub pin_name: String,
    pub new_value: PinValue,
}

/// Node dimensions
pub const NODE_WIDTH: f32 = 220.0;
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

/// Draw a single node and return pin positions and any value changes
pub fn draw_node(
    ui: &mut egui::Ui,
    painter: &egui::Painter,
    node: &BlueprintNode,
    node_def: Option<&super::nodes::NodeTypeDefinition>,
    canvas: &BlueprintCanvasState,
    canvas_rect: Rect,
    is_selected: bool,
    connected_inputs: &HashSet<String>,
    _state: &mut BlueprintEditorState,
) -> (Rect, HashMap<(String, PinDirection), Pos2>, Vec<NodeValueChange>) {
    let mut pin_positions = HashMap::new();
    let mut value_changes = Vec::new();

    // Calculate screen position
    let screen_pos = canvas.canvas_to_screen(node.position, canvas_rect);

    // Calculate node height based on pins
    let input_count = node.input_pins().count();
    let output_count = node.output_pins().count();
    let max_pins = input_count.max(output_count);

    // Add extra height for texture nodes to show preview
    let texture_preview_height = if is_texture_node(&node.node_type) {
        120.0 // Space for 100px preview + margins
    } else {
        0.0
    };

    let node_height = NODE_HEADER_HEIGHT + (max_pins as f32 * NODE_PIN_HEIGHT) + 8.0 + texture_preview_height;

    let scaled_width = NODE_WIDTH * canvas.zoom;
    let scaled_height = node_height * canvas.zoom;

    let node_rect = Rect::from_min_size(screen_pos, EguiVec2::new(scaled_width, scaled_height));

    // Skip if not visible
    if !canvas_rect.intersects(node_rect) {
        return (node_rect, pin_positions, value_changes);
    }

    // Draw node background
    painter.rect_filled(node_rect, NODE_CORNER_RADIUS * canvas.zoom, NODE_BG_COLOR);

    // Draw header
    let header_color = if let Some(def) = node_def {
        Color32::from_rgb(def.color[0], def.color[1], def.color[2])
    } else if let Some(color) = node.color {
        Color32::from_rgb(color[0], color[1], color[2])
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
    let title = node_def.map(|d| d.display_name)
        .or_else(|| node.display_name.as_deref())
        .unwrap_or(&node.node_type);
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

        // Check if this input is connected
        let is_connected = connected_inputs.contains(&pin.name);

        // Calculate label width for positioning the editor
        let label_pos = Pos2::new(pin_pos.x + pin_radius * 2.0, pin_pos.y);
        let font_size = 10.0 * canvas.zoom;
        let label_width = if !pin.label.is_empty() {
            // Approximate label width (about 6 pixels per character at base size)
            (pin.label.len() as f32 * 5.5 * canvas.zoom).min(60.0 * canvas.zoom)
        } else {
            0.0
        };

        // Always draw label if present
        if !pin.label.is_empty() {
            painter.text(
                label_pos,
                egui::Align2::LEFT_CENTER,
                &pin.label,
                egui::FontId::proportional(font_size),
                Color32::from_rgb(200, 200, 200),
            );
        }

        // Draw inline value editor for unconnected inputs (except Flow pins)
        let has_editor = !is_connected && pin.pin_type != PinType::Flow && canvas.zoom >= 0.5;
        if has_editor {
            // Position editor after the label
            let editor_x = label_pos.x + label_width + 4.0 * canvas.zoom;
            let editor_width = (screen_pos.x + scaled_width - editor_x - 4.0 * canvas.zoom).max(30.0);
            let editor_height = (pin_spacing - 4.0 * canvas.zoom).max(14.0);
            let editor_y = input_y + 2.0 * canvas.zoom;

            let editor_rect = Rect::from_min_size(
                Pos2::new(editor_x, editor_y),
                EguiVec2::new(editor_width, editor_height),
            );

            // Get current value
            let current_value = node.get_input_value(&pin.name)
                .or_else(|| pin.default_value.clone())
                .unwrap_or_else(|| PinValue::default_for_type(pin.pin_type.clone()));

            // Draw the value editor
            if let Some(new_value) = draw_value_editor(ui, editor_rect, &pin.name, node.id, &current_value, canvas.zoom) {
                value_changes.push(NodeValueChange {
                    node_id: node.id,
                    pin_name: pin.name.clone(),
                    new_value,
                });
            }
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

    (node_rect, pin_positions, value_changes)
}

/// Check if a pin name suggests it's a color value
fn is_color_pin(pin_name: &str) -> bool {
    let lower = pin_name.to_lowercase();
    lower.contains("color") || lower.contains("albedo") || lower.contains("tint")
        || lower == "emissive" || lower == "emission"
}

/// Draw an inline value editor and return new value if changed
fn draw_value_editor(
    ui: &mut egui::Ui,
    rect: Rect,
    pin_name: &str,
    node_id: NodeId,
    current_value: &PinValue,
    _zoom: f32,
) -> Option<PinValue> {
    // Only draw if rect is reasonably sized
    if rect.width() < 30.0 || rect.height() < 10.0 {
        return None;
    }

    let id = egui::Id::new(("node_value", node_id.0, pin_name));

    // Create a small UI area for the widget
    let mut new_value = None;

    ui.scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| {
        ui.set_clip_rect(rect);

        // Use push_id to scope the widgets
        ui.push_id(id, |ui| {
            // Style adjustments for small widgets
            ui.style_mut().spacing.interact_size.y = rect.height();
            ui.style_mut().spacing.button_padding = EguiVec2::new(2.0, 0.0);

            // Check if this is a color-like value (Color type or Vec4 with color-related name)
            let is_color_value = matches!(current_value, PinValue::Color(_))
                || (matches!(current_value, PinValue::Vec4(_)) && is_color_pin(pin_name));

            if is_color_value {
                // Handle as color picker
                let color_val = match current_value {
                    PinValue::Color(c) | PinValue::Vec4(c) => *c,
                    _ => [1.0, 1.0, 1.0, 1.0],
                };
                let mut color32 = Color32::from_rgba_unmultiplied(
                    (color_val[0].clamp(0.0, 1.0) * 255.0) as u8,
                    (color_val[1].clamp(0.0, 1.0) * 255.0) as u8,
                    (color_val[2].clamp(0.0, 1.0) * 255.0) as u8,
                    (color_val[3].clamp(0.0, 1.0) * 255.0) as u8,
                );

                // Use color_edit_button_srgba which handles everything including popup
                let response = ui.color_edit_button_srgba(&mut color32);

                if response.changed() {
                    let new_color = [
                        color32.r() as f32 / 255.0,
                        color32.g() as f32 / 255.0,
                        color32.b() as f32 / 255.0,
                        color32.a() as f32 / 255.0,
                    ];
                    if matches!(current_value, PinValue::Color(_)) {
                        new_value = Some(PinValue::Color(new_color));
                    } else {
                        new_value = Some(PinValue::Vec4(new_color));
                    }
                }
            } else {
                // Handle other value types
                match current_value {
                    PinValue::Float(v) => {
                        let mut val = *v;
                        let response = ui.add_sized(
                            rect.size(),
                            egui::DragValue::new(&mut val)
                                .speed(0.01)
                                .range(f32::NEG_INFINITY..=f32::INFINITY)
                        );
                        if response.changed() {
                            new_value = Some(PinValue::Float(val));
                        }
                    }
                    PinValue::Int(v) => {
                        let mut val = *v;
                        let response = ui.add_sized(
                            rect.size(),
                            egui::DragValue::new(&mut val)
                                .speed(0.1)
                        );
                        if response.changed() {
                            new_value = Some(PinValue::Int(val));
                        }
                    }
                    PinValue::Bool(v) => {
                        let mut val = *v;
                        if ui.add_sized(rect.size(), egui::Checkbox::new(&mut val, "")).changed() {
                            new_value = Some(PinValue::Bool(val));
                        }
                    }
                    PinValue::Vec2(v) => {
                        let mut val = *v;
                        ui.horizontal(|ui| {
                            let w = (rect.width() - 4.0) / 2.0;
                            let changed1 = ui.add_sized([w, rect.height()], egui::DragValue::new(&mut val[0]).speed(0.01)).changed();
                            let changed2 = ui.add_sized([w, rect.height()], egui::DragValue::new(&mut val[1]).speed(0.01)).changed();
                            if changed1 || changed2 {
                                new_value = Some(PinValue::Vec2(val));
                            }
                        });
                    }
                    PinValue::Vec3(v) => {
                        let mut val = *v;
                        ui.horizontal(|ui| {
                            let w = (rect.width() - 8.0) / 3.0;
                            let c1 = ui.add_sized([w, rect.height()], egui::DragValue::new(&mut val[0]).speed(0.01)).changed();
                            let c2 = ui.add_sized([w, rect.height()], egui::DragValue::new(&mut val[1]).speed(0.01)).changed();
                            let c3 = ui.add_sized([w, rect.height()], egui::DragValue::new(&mut val[2]).speed(0.01)).changed();
                            if c1 || c2 || c3 {
                                new_value = Some(PinValue::Vec3(val));
                            }
                        });
                    }
                    PinValue::Vec4(v) => {
                        // Non-color Vec4 - show 4 values
                        let mut val = *v;
                        ui.horizontal(|ui| {
                            let w = (rect.width() - 12.0) / 4.0;
                            let c1 = ui.add_sized([w, rect.height()], egui::DragValue::new(&mut val[0]).speed(0.01)).changed();
                            let c2 = ui.add_sized([w, rect.height()], egui::DragValue::new(&mut val[1]).speed(0.01)).changed();
                            let c3 = ui.add_sized([w, rect.height()], egui::DragValue::new(&mut val[2]).speed(0.01)).changed();
                            let c4 = ui.add_sized([w, rect.height()], egui::DragValue::new(&mut val[3]).speed(0.01)).changed();
                            if c1 || c2 || c3 || c4 {
                                new_value = Some(PinValue::Vec4(val));
                            }
                        });
                    }
                    PinValue::String(s) => {
                        let mut val = s.clone();
                        let response = ui.add_sized(
                            rect.size(),
                            egui::TextEdit::singleline(&mut val)
                                .desired_width(rect.width())
                        );
                        if response.changed() {
                            new_value = Some(PinValue::String(val));
                        }
                    }
                    PinValue::Texture2D(path) => {
                        let mut val = path.clone();
                        let response = ui.add_sized(
                            rect.size(),
                            egui::TextEdit::singleline(&mut val)
                                .hint_text("texture path")
                                .desired_width(rect.width())
                        );
                        if response.changed() {
                            new_value = Some(PinValue::Texture2D(val));
                        }
                    }
                    // Flow, Sampler, Color (handled above), and runtime types don't need editors here
                    PinValue::Flow | PinValue::Sampler | PinValue::Color(_) |
                    PinValue::Entity(_) | PinValue::EntityArray(_) | PinValue::StringArray(_) |
                    PinValue::Asset(_) | PinValue::AudioHandle(_) | PinValue::TimerHandle(_) |
                    PinValue::SceneHandle(_) | PinValue::PrefabHandle(_) | PinValue::GltfHandle(_) => {}
                }
            }
        });
    });

    new_value
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
