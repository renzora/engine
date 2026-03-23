//! Data structures for the node graph widget.

use bevy_egui::egui::{self, Color32};

// ── Identifiers ────────────────────────────────────────────────────────────

/// Unique identifier for a node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NodeId(pub u64);

/// Direction a pin faces.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PinDirection {
    Input,
    Output,
}

/// Identifies a specific pin on a specific node.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PinId {
    pub node: u64,
    pub name: String,
    pub direction: PinDirection,
}

// ── Pin / Node definitions ─────────────────────────────────────────────────

/// Visual shape for a pin dot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PinShape {
    /// Circle — typical for data pins.
    Circle,
    /// Right-pointing triangle — typical for execution/flow pins.
    Triangle,
}

/// One pin on a node (caller defines these).
pub struct PinDef {
    /// Unique name within the node (used as connection key).
    pub name: String,
    /// Display text drawn next to the dot.
    pub label: String,
    /// Dot / triangle fill color.
    pub color: Color32,
    /// Visual shape.
    pub shape: PinShape,
    /// Whether this is an input or output.
    pub direction: PinDirection,
}

/// One node in the graph (caller owns a `Vec<NodeDef>`).
pub struct NodeDef {
    pub id: u64,
    pub title: String,
    pub header_color: Color32,
    /// Position in canvas (logical) coordinates.
    pub position: [f32; 2],
    pub pins: Vec<PinDef>,
    /// Optional texture thumbnail displayed between header and pins.
    pub thumbnail: Option<egui::TextureId>,
}

/// A connection between two pins.
pub struct ConnectionDef {
    pub from_node: u64,
    pub from_pin: String,
    pub to_node: u64,
    pub to_pin: String,
    /// `None` = inherit from-pin color.
    pub color: Option<Color32>,
}

// ── Mutable state (persisted by caller across frames) ──────────────────────

/// Full mutable state for a node graph instance.
pub struct NodeGraphState {
    pub nodes: Vec<NodeDef>,
    pub connections: Vec<ConnectionDef>,
    /// Canvas pan offset.
    pub offset: [f32; 2],
    /// Zoom level (clamped 0.25 .. 4.0).
    pub zoom: f32,
    /// Currently selected node ids.
    pub selected: Vec<u64>,

    // -- internal (managed by widget, but must persist across frames) --
    pub dragging: Option<DragState>,
    pub connecting: Option<ConnectingState>,
    pub box_select: Option<BoxSelectState>,
}

impl Default for NodeGraphState {
    fn default() -> Self {
        Self {
            nodes: Vec::new(),
            connections: Vec::new(),
            offset: [0.0, 0.0],
            zoom: 1.0,
            selected: Vec::new(),
            dragging: None,
            connecting: None,
            box_select: None,
        }
    }
}

/// Internal: node being dragged.
pub struct DragState {
    pub node: u64,
    pub offset: [f32; 2],
}

/// Internal: connection wire being drawn from a pin.
#[derive(Clone)]
pub struct ConnectingState {
    pub from: PinId,
    pub from_pos: [f32; 2],
}

/// Internal: rubber-band box selection in progress.
pub struct BoxSelectState {
    pub start: [f32; 2],
    pub end: [f32; 2],
}

// ── Visual config (theme-driven, caller provides) ──────────────────────────

/// Visual tuning knobs for the node graph renderer.
pub struct NodeGraphConfig {
    pub grid_spacing: f32,
    pub node_width: f32,
    pub header_height: f32,
    pub pin_height: f32,
    pub pin_radius: f32,
    pub corner_radius: f32,
    pub node_bg: Color32,
    pub node_border: Color32,
    pub selected_border: Color32,
    pub grid_dot: Color32,
    pub canvas_bg: Color32,
    pub text_color: Color32,
    pub text_muted: Color32,
    pub connection_width: f32,
    pub selection_fill: Color32,
    pub selection_stroke: Color32,
}

impl Default for NodeGraphConfig {
    fn default() -> Self {
        Self {
            grid_spacing: 20.0,
            node_width: 180.0,
            header_height: 26.0,
            pin_height: 22.0,
            pin_radius: 5.0,
            corner_radius: 4.0,
            node_bg: Color32::from_rgb(40, 40, 45),
            node_border: Color32::from_rgb(60, 60, 65),
            selected_border: Color32::from_rgb(100, 150, 255),
            grid_dot: Color32::from_rgb(60, 60, 65),
            canvas_bg: Color32::from_rgb(22, 22, 28),
            text_color: Color32::from_rgb(220, 220, 220),
            text_muted: Color32::from_rgb(140, 140, 150),
            connection_width: 2.0,
            selection_fill: Color32::from_rgba_premultiplied(100, 150, 255, 30),
            selection_stroke: Color32::from_rgb(100, 150, 255),
        }
    }
}

// ── Response (what happened this frame) ────────────────────────────────────

/// Returned by [`node_graph()`](super::node_graph) each frame.
pub struct NodeGraphResponse {
    pub selection_changed: bool,
    pub node_moved: Option<u64>,
    pub connection_made: Option<(PinId, PinId)>,
    pub connection_removed: Option<(u64, String)>,
    pub nodes_deleted: Vec<u64>,
    /// The canvas `egui::Response` — call `.context_menu()` on this for right-click menus.
    /// Only `Some` when right-click was NOT consumed by pin/cable interaction.
    pub canvas_response: Option<egui::Response>,
    /// True if a right-click was handled internally (pin disconnect or cable cut).
    pub right_click_handled: bool,
}

impl Default for NodeGraphResponse {
    fn default() -> Self {
        Self {
            selection_changed: false,
            node_moved: None,
            connection_made: None,
            connection_removed: None,
            nodes_deleted: Vec::new(),
            canvas_response: None,
            right_click_handled: false,
        }
    }
}
