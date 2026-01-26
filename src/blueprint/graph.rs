//! Core graph data structures for the blueprint system
//!
//! Contains BlueprintGraph, BlueprintNode, Pin, Connection, and related types.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Unique identifier for a node in a blueprint graph
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NodeId(pub u64);

impl NodeId {
    pub fn new(id: u64) -> Self {
        Self(id)
    }
}

/// Unique identifier for a pin on a node
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PinId {
    pub node_id: NodeId,
    pub pin_name: String,
    pub direction: PinDirection,
}

impl PinId {
    pub fn new(node_id: NodeId, pin_name: impl Into<String>) -> Self {
        // Default to Output for backwards compatibility
        Self {
            node_id,
            pin_name: pin_name.into(),
            direction: PinDirection::Output,
        }
    }

    pub fn input(node_id: NodeId, pin_name: impl Into<String>) -> Self {
        Self {
            node_id,
            pin_name: pin_name.into(),
            direction: PinDirection::Input,
        }
    }

    pub fn output(node_id: NodeId, pin_name: impl Into<String>) -> Self {
        Self {
            node_id,
            pin_name: pin_name.into(),
            direction: PinDirection::Output,
        }
    }
}

/// Type of data a pin carries
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PinType {
    /// Execution flow (white triangle)
    Flow,
    /// Floating point number (green)
    Float,
    /// Integer number (cyan)
    Int,
    /// Boolean value (red)
    Bool,
    /// String value (magenta)
    String,
    /// 2D vector (yellow)
    Vec2,
    /// 3D vector (orange)
    Vec3,
    /// Color value (purple)
    Color,
    /// Any type (gray) - for generic nodes
    Any,
}

impl PinType {
    /// Get the display color for this pin type (egui Color32)
    pub fn color(&self) -> [u8; 3] {
        match self {
            PinType::Flow => [255, 255, 255],   // White
            PinType::Float => [100, 200, 100],  // Green
            PinType::Int => [100, 200, 200],    // Cyan
            PinType::Bool => [200, 100, 100],   // Red
            PinType::String => [200, 100, 200], // Magenta
            PinType::Vec2 => [200, 200, 100],   // Yellow
            PinType::Vec3 => [200, 150, 100],   // Orange
            PinType::Color => [150, 100, 200],  // Purple
            PinType::Any => [150, 150, 150],    // Gray
        }
    }

    /// Check if this type can connect to another type
    pub fn can_connect_to(&self, other: &PinType) -> bool {
        if *self == PinType::Any || *other == PinType::Any {
            return true;
        }
        *self == *other
    }

    /// Get display name for this type
    pub fn name(&self) -> &'static str {
        match self {
            PinType::Flow => "Flow",
            PinType::Float => "Float",
            PinType::Int => "Int",
            PinType::Bool => "Bool",
            PinType::String => "String",
            PinType::Vec2 => "Vec2",
            PinType::Vec3 => "Vec3",
            PinType::Color => "Color",
            PinType::Any => "Any",
        }
    }
}

/// Direction of a pin (input or output)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PinDirection {
    Input,
    Output,
}

/// A pin (connection point) on a node
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pin {
    /// Name of the pin (unique within the node)
    pub name: String,
    /// Display label for the pin
    pub label: String,
    /// Type of data this pin carries
    pub pin_type: PinType,
    /// Whether this is an input or output pin
    pub direction: PinDirection,
    /// Default value (for inputs that aren't connected)
    pub default_value: Option<PinValue>,
    /// Whether this pin is required (no default, must be connected)
    pub required: bool,
}

impl Pin {
    /// Create a new input pin
    pub fn input(name: impl Into<String>, label: impl Into<String>, pin_type: PinType) -> Self {
        Self {
            name: name.into(),
            label: label.into(),
            pin_type,
            direction: PinDirection::Input,
            default_value: None,
            required: false,
        }
    }

    /// Create a new output pin
    pub fn output(name: impl Into<String>, label: impl Into<String>, pin_type: PinType) -> Self {
        Self {
            name: name.into(),
            label: label.into(),
            pin_type,
            direction: PinDirection::Output,
            default_value: None,
            required: false,
        }
    }

    /// Set a default value for this pin
    pub fn with_default(mut self, value: PinValue) -> Self {
        self.default_value = Some(value);
        self
    }

    /// Mark this pin as required
    pub fn required(mut self) -> Self {
        self.required = true;
        self
    }
}

/// Value that can be stored in a pin
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PinValue {
    Flow,
    Float(f32),
    Int(i32),
    Bool(bool),
    String(String),
    Vec2([f32; 2]),
    Vec3([f32; 3]),
    Color([f32; 4]),
}

impl PinValue {
    /// Get the pin type for this value
    pub fn pin_type(&self) -> PinType {
        match self {
            PinValue::Flow => PinType::Flow,
            PinValue::Float(_) => PinType::Float,
            PinValue::Int(_) => PinType::Int,
            PinValue::Bool(_) => PinType::Bool,
            PinValue::String(_) => PinType::String,
            PinValue::Vec2(_) => PinType::Vec2,
            PinValue::Vec3(_) => PinType::Vec3,
            PinValue::Color(_) => PinType::Color,
        }
    }

    /// Get a default value for a pin type
    pub fn default_for_type(pin_type: PinType) -> Self {
        match pin_type {
            PinType::Flow => PinValue::Flow,
            PinType::Float => PinValue::Float(0.0),
            PinType::Int => PinValue::Int(0),
            PinType::Bool => PinValue::Bool(false),
            PinType::String => PinValue::String(String::new()),
            PinType::Vec2 => PinValue::Vec2([0.0, 0.0]),
            PinType::Vec3 => PinValue::Vec3([0.0, 0.0, 0.0]),
            PinType::Color => PinValue::Color([1.0, 1.0, 1.0, 1.0]),
            PinType::Any => PinValue::Float(0.0),
        }
    }

    /// Convert to Rhai code representation
    pub fn to_rhai(&self) -> String {
        match self {
            PinValue::Flow => String::new(),
            PinValue::Float(v) => format!("{:.6}", v),
            PinValue::Int(v) => format!("{}", v),
            PinValue::Bool(v) => format!("{}", v),
            PinValue::String(v) => format!("\"{}\"", v.replace('\\', "\\\\").replace('"', "\\\"")),
            PinValue::Vec2(v) => format!("vec2({:.6}, {:.6})", v[0], v[1]),
            PinValue::Vec3(v) => format!("vec3({:.6}, {:.6}, {:.6})", v[0], v[1], v[2]),
            PinValue::Color(v) => format!("color({:.6}, {:.6}, {:.6}, {:.6})", v[0], v[1], v[2], v[3]),
        }
    }
}

impl Default for PinValue {
    fn default() -> Self {
        PinValue::Float(0.0)
    }
}

/// A connection between two pins
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Connection {
    /// Source pin (output)
    pub from: PinId,
    /// Target pin (input)
    pub to: PinId,
}

/// A node in the blueprint graph
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlueprintNode {
    /// Unique identifier for this node
    pub id: NodeId,
    /// Type identifier (e.g., "math/add", "event/on_update")
    pub node_type: String,
    /// Position on the canvas
    pub position: [f32; 2],
    /// Pins defined by the node type
    pub pins: Vec<Pin>,
    /// Override values for input pins (pin_name -> value)
    pub input_values: HashMap<String, PinValue>,
    /// Comment text (for comment nodes)
    pub comment: Option<String>,
}

impl BlueprintNode {
    /// Create a new node with the given type and ID
    pub fn new(id: NodeId, node_type: impl Into<String>, pins: Vec<Pin>) -> Self {
        Self {
            id,
            node_type: node_type.into(),
            position: [0.0, 0.0],
            pins,
            input_values: HashMap::new(),
            comment: None,
        }
    }

    /// Set the position of this node
    pub fn with_position(mut self, x: f32, y: f32) -> Self {
        self.position = [x, y];
        self
    }

    /// Get an input pin by name
    pub fn get_input_pin(&self, name: &str) -> Option<&Pin> {
        self.pins.iter().find(|p| p.name == name && p.direction == PinDirection::Input)
    }

    /// Get an output pin by name
    pub fn get_output_pin(&self, name: &str) -> Option<&Pin> {
        self.pins.iter().find(|p| p.name == name && p.direction == PinDirection::Output)
    }

    /// Get all input pins
    pub fn input_pins(&self) -> impl Iterator<Item = &Pin> {
        self.pins.iter().filter(|p| p.direction == PinDirection::Input)
    }

    /// Get all output pins
    pub fn output_pins(&self) -> impl Iterator<Item = &Pin> {
        self.pins.iter().filter(|p| p.direction == PinDirection::Output)
    }

    /// Get the effective value for an input pin (override or default)
    pub fn get_input_value(&self, pin_name: &str) -> Option<PinValue> {
        if let Some(value) = self.input_values.get(pin_name) {
            return Some(value.clone());
        }
        if let Some(pin) = self.get_input_pin(pin_name) {
            return pin.default_value.clone();
        }
        None
    }

    /// Set an input value override
    pub fn set_input_value(&mut self, pin_name: impl Into<String>, value: PinValue) {
        self.input_values.insert(pin_name.into(), value);
    }
}

/// A variable defined at the graph level
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlueprintVariable {
    /// Variable name (identifier in code)
    pub name: String,
    /// Display name in the editor
    pub display_name: String,
    /// Type of the variable
    pub var_type: PinType,
    /// Default value
    pub default_value: PinValue,
    /// Description/tooltip
    pub description: String,
    /// Whether this variable is exposed in the inspector
    pub exposed: bool,
}

impl BlueprintVariable {
    pub fn new(name: impl Into<String>, var_type: PinType) -> Self {
        let name = name.into();
        Self {
            display_name: name.clone(),
            name,
            var_type,
            default_value: PinValue::default_for_type(var_type),
            description: String::new(),
            exposed: true,
        }
    }

    pub fn with_default(mut self, value: PinValue) -> Self {
        self.default_value = value;
        self
    }

    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = desc.into();
        self
    }
}

/// A complete blueprint graph
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlueprintGraph {
    /// Name of the blueprint
    pub name: String,
    /// All nodes in the graph
    pub nodes: Vec<BlueprintNode>,
    /// Connections between nodes
    pub connections: Vec<Connection>,
    /// Graph-level variables
    pub variables: Vec<BlueprintVariable>,
    /// Next available node ID
    next_node_id: u64,
}

impl Default for BlueprintGraph {
    fn default() -> Self {
        Self {
            name: "New Blueprint".to_string(),
            nodes: Vec::new(),
            connections: Vec::new(),
            variables: Vec::new(),
            next_node_id: 1,
        }
    }
}

impl BlueprintGraph {
    /// Create a new empty blueprint with the given name
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            ..Default::default()
        }
    }

    /// Generate a new unique node ID
    pub fn next_node_id(&mut self) -> NodeId {
        let id = NodeId::new(self.next_node_id);
        self.next_node_id += 1;
        id
    }

    /// Add a node to the graph
    pub fn add_node(&mut self, node: BlueprintNode) {
        // Update next_node_id if needed
        if node.id.0 >= self.next_node_id {
            self.next_node_id = node.id.0 + 1;
        }
        self.nodes.push(node);
    }

    /// Remove a node and all its connections
    pub fn remove_node(&mut self, node_id: NodeId) {
        self.nodes.retain(|n| n.id != node_id);
        self.connections.retain(|c| c.from.node_id != node_id && c.to.node_id != node_id);
    }

    /// Get a node by ID
    pub fn get_node(&self, id: NodeId) -> Option<&BlueprintNode> {
        self.nodes.iter().find(|n| n.id == id)
    }

    /// Get a mutable reference to a node by ID
    pub fn get_node_mut(&mut self, id: NodeId) -> Option<&mut BlueprintNode> {
        self.nodes.iter_mut().find(|n| n.id == id)
    }

    /// Add a connection between two pins
    pub fn add_connection(&mut self, from: PinId, to: PinId) -> bool {
        // Verify pins exist and types are compatible
        let from_node = self.get_node(from.node_id);
        let to_node = self.get_node(to.node_id);

        if let (Some(from_node), Some(to_node)) = (from_node, to_node) {
            let from_pin = from_node.get_output_pin(&from.pin_name);
            let to_pin = to_node.get_input_pin(&to.pin_name);

            if let (Some(from_pin), Some(to_pin)) = (from_pin, to_pin) {
                if from_pin.pin_type.can_connect_to(&to_pin.pin_type) {
                    // Remove existing connection to this input (if any)
                    self.connections.retain(|c| c.to != to);

                    // For flow pins, we allow multiple connections from output
                    // but only one connection to each input
                    self.connections.push(Connection { from, to });
                    return true;
                }
            }
        }
        false
    }

    /// Remove a connection
    pub fn remove_connection(&mut self, from: &PinId, to: &PinId) {
        self.connections.retain(|c| &c.from != from || &c.to != to);
    }

    /// Get all connections from a pin
    pub fn connections_from<'a>(&'a self, pin: &'a PinId) -> impl Iterator<Item = &'a Connection> + 'a {
        self.connections.iter().filter(move |c| &c.from == pin)
    }

    /// Get the connection to a pin (if any)
    pub fn connection_to(&self, pin: &PinId) -> Option<&Connection> {
        self.connections.iter().find(|c| &c.to == pin)
    }

    /// Check if a pin has any connections
    pub fn is_pin_connected(&self, pin: &PinId) -> bool {
        // Match by node_id and pin_name, considering direction for which side to check
        self.connections.iter().any(|c| {
            match pin.direction {
                PinDirection::Output => c.from.node_id == pin.node_id && c.from.pin_name == pin.pin_name,
                PinDirection::Input => c.to.node_id == pin.node_id && c.to.pin_name == pin.pin_name,
            }
        })
    }

    /// Remove all connections involving a pin (based on direction)
    pub fn remove_connections_for_pin(&mut self, pin: &PinId) {
        self.connections.retain(|c| {
            match pin.direction {
                PinDirection::Output => !(c.from.node_id == pin.node_id && c.from.pin_name == pin.pin_name),
                PinDirection::Input => !(c.to.node_id == pin.node_id && c.to.pin_name == pin.pin_name),
            }
        });
    }

    /// Add a variable to the graph
    pub fn add_variable(&mut self, var: BlueprintVariable) {
        self.variables.push(var);
    }

    /// Remove a variable by name
    pub fn remove_variable(&mut self, name: &str) {
        self.variables.retain(|v| v.name != name);
    }

    /// Get a variable by name
    pub fn get_variable(&self, name: &str) -> Option<&BlueprintVariable> {
        self.variables.iter().find(|v| v.name == name)
    }

    /// Find event nodes (entry points) in the graph
    pub fn event_nodes(&self) -> impl Iterator<Item = &BlueprintNode> {
        self.nodes.iter().filter(|n| n.node_type.starts_with("event/"))
    }
}
