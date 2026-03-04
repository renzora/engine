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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PinType {
    /// Execution flow (white triangle)
    Flow,
    /// Execution flow alias for behavior nodes
    Execution,
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
    /// 4D vector (pink) - for shader materials
    Vec4,
    /// Color value (purple)
    Color,
    /// 2D texture reference (blue) - for shader materials
    Texture2D,
    /// Sampler reference (teal) - for shader materials
    Sampler,
    /// Any type (gray) - for generic nodes
    Any,
    /// Entity reference (light blue)
    Entity,
    /// Array of entities
    EntityArray,
    /// Array of strings
    StringArray,
    /// Asset reference (gold)
    Asset,
    /// Audio handle (purple-pink)
    AudioHandle,
    /// Timer handle (green-cyan)
    TimerHandle,
    /// Scene handle (orange-red)
    SceneHandle,
    /// Prefab handle (yellow-green)
    PrefabHandle,
    /// GLTF handle (brown)
    GltfHandle,
    /// Custom type for render pipeline nodes (gray-blue)
    Custom(String),
}

impl PinType {
    /// Get the display color for this pin type (egui Color32)
    pub fn color(&self) -> [u8; 3] {
        match self {
            PinType::Flow | PinType::Execution => [255, 255, 255], // White
            PinType::Float => [100, 200, 100],    // Green
            PinType::Int => [100, 200, 200],      // Cyan
            PinType::Bool => [200, 100, 100],     // Red
            PinType::String => [200, 100, 200],   // Magenta
            PinType::Vec2 => [200, 200, 100],     // Yellow
            PinType::Vec3 => [200, 150, 100],     // Orange
            PinType::Vec4 => [220, 130, 180],     // Pink
            PinType::Color => [150, 100, 200],    // Purple
            PinType::Texture2D => [100, 150, 220],// Blue
            PinType::Sampler => [100, 180, 180],  // Teal
            PinType::Any => [150, 150, 150],      // Gray
            PinType::Entity => [100, 180, 220],   // Light blue
            PinType::EntityArray => [80, 160, 200], // Darker blue
            PinType::StringArray => [180, 80, 180], // Darker magenta
            PinType::Asset => [220, 180, 80],     // Gold
            PinType::AudioHandle => [200, 100, 180], // Purple-pink
            PinType::TimerHandle => [100, 180, 160], // Green-cyan
            PinType::SceneHandle => [220, 140, 100], // Orange-red
            PinType::PrefabHandle => [180, 200, 100], // Yellow-green
            PinType::GltfHandle => [180, 140, 100], // Brown
            PinType::Custom(_) => [120, 140, 180], // Gray-blue for render types
        }
    }

    /// Check if this type can connect to another type
    pub fn can_connect_to(&self, other: &PinType) -> bool {
        if *self == PinType::Any || *other == PinType::Any {
            return true;
        }
        // Flow and Execution are interchangeable
        if (*self == PinType::Flow || *self == PinType::Execution)
            && (*other == PinType::Flow || *other == PinType::Execution) {
            return true;
        }
        // Color and Vec4 are compatible (both are 4-component vectors)
        if (*self == PinType::Color && *other == PinType::Vec4)
            || (*self == PinType::Vec4 && *other == PinType::Color) {
            return true;
        }
        // Custom types match if they have the same string identifier
        if let (PinType::Custom(a), PinType::Custom(b)) = (self, other) {
            return a == b;
        }
        *self == *other
    }

    /// Get display name for this type
    #[allow(dead_code)]
    pub fn name(&self) -> &'static str {
        match self {
            PinType::Flow => "Flow",
            PinType::Execution => "Execution",
            PinType::Float => "Float",
            PinType::Int => "Int",
            PinType::Bool => "Bool",
            PinType::String => "String",
            PinType::Vec2 => "Vec2",
            PinType::Vec3 => "Vec3",
            PinType::Vec4 => "Vec4",
            PinType::Color => "Color",
            PinType::Texture2D => "Texture2D",
            PinType::Sampler => "Sampler",
            PinType::Any => "Any",
            PinType::Entity => "Entity",
            PinType::EntityArray => "Entity[]",
            PinType::StringArray => "String[]",
            PinType::Asset => "Asset",
            PinType::AudioHandle => "AudioHandle",
            PinType::TimerHandle => "TimerHandle",
            PinType::SceneHandle => "SceneHandle",
            PinType::PrefabHandle => "PrefabHandle",
            PinType::GltfHandle => "GltfHandle",
            PinType::Custom(_) => "Custom",
        }
    }

    /// Returns true if this is a shader-only type
    #[allow(dead_code)]
    pub fn is_shader_type(&self) -> bool {
        matches!(self, PinType::Texture2D | PinType::Sampler | PinType::Vec4)
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
    #[allow(dead_code)]
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
    Vec4([f32; 4]),
    Color([f32; 4]),
    /// Texture asset path (for shader materials)
    Texture2D(String),
    /// Sampler configuration (for shader materials) - not typically edited directly
    Sampler,
    /// Entity ID (for behavior nodes)
    Entity(u64),
    /// Array of entity IDs
    EntityArray(Vec<u64>),
    /// Array of strings
    StringArray(Vec<String>),
    /// Asset path
    Asset(String),
    /// Audio handle ID
    AudioHandle(u64),
    /// Timer handle ID
    TimerHandle(u64),
    /// Scene handle ID
    SceneHandle(u64),
    /// Prefab handle ID
    PrefabHandle(u64),
    /// GLTF handle ID
    GltfHandle(u64),
}

impl PinValue {
    /// Get the pin type for this value
    #[allow(dead_code)]
    pub fn pin_type(&self) -> PinType {
        match self {
            PinValue::Flow => PinType::Flow,
            PinValue::Float(_) => PinType::Float,
            PinValue::Int(_) => PinType::Int,
            PinValue::Bool(_) => PinType::Bool,
            PinValue::String(_) => PinType::String,
            PinValue::Vec2(_) => PinType::Vec2,
            PinValue::Vec3(_) => PinType::Vec3,
            PinValue::Vec4(_) => PinType::Vec4,
            PinValue::Color(_) => PinType::Color,
            PinValue::Texture2D(_) => PinType::Texture2D,
            PinValue::Sampler => PinType::Sampler,
            PinValue::Entity(_) => PinType::Entity,
            PinValue::EntityArray(_) => PinType::EntityArray,
            PinValue::StringArray(_) => PinType::StringArray,
            PinValue::Asset(_) => PinType::Asset,
            PinValue::AudioHandle(_) => PinType::AudioHandle,
            PinValue::TimerHandle(_) => PinType::TimerHandle,
            PinValue::SceneHandle(_) => PinType::SceneHandle,
            PinValue::PrefabHandle(_) => PinType::PrefabHandle,
            PinValue::GltfHandle(_) => PinType::GltfHandle,
        }
    }

    /// Get a default value for a pin type
    #[allow(dead_code)]
    pub fn default_for_type(pin_type: PinType) -> Self {
        match pin_type {
            PinType::Flow | PinType::Execution => PinValue::Flow,
            PinType::Float => PinValue::Float(0.0),
            PinType::Int => PinValue::Int(0),
            PinType::Bool => PinValue::Bool(false),
            PinType::String => PinValue::String(String::new()),
            PinType::Vec2 => PinValue::Vec2([0.0, 0.0]),
            PinType::Vec3 => PinValue::Vec3([0.0, 0.0, 0.0]),
            PinType::Vec4 => PinValue::Vec4([0.0, 0.0, 0.0, 0.0]),
            PinType::Color => PinValue::Color([1.0, 1.0, 1.0, 1.0]),
            PinType::Texture2D => PinValue::Texture2D(String::new()),
            PinType::Sampler => PinValue::Sampler,
            PinType::Any => PinValue::Float(0.0),
            PinType::Entity => PinValue::Entity(0),
            PinType::EntityArray => PinValue::EntityArray(Vec::new()),
            PinType::StringArray => PinValue::StringArray(Vec::new()),
            PinType::Asset => PinValue::Asset(String::new()),
            PinType::AudioHandle => PinValue::AudioHandle(0),
            PinType::TimerHandle => PinValue::TimerHandle(0),
            PinType::SceneHandle => PinValue::SceneHandle(0),
            PinType::PrefabHandle => PinValue::PrefabHandle(0),
            PinType::GltfHandle => PinValue::GltfHandle(0),
            PinType::Custom(_) => PinValue::String(String::new()), // Custom types use strings as placeholders
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
            PinValue::Vec4(v) => format!("vec4({:.6}, {:.6}, {:.6}, {:.6})", v[0], v[1], v[2], v[3]),
            PinValue::Color(v) => format!("color({:.6}, {:.6}, {:.6}, {:.6})", v[0], v[1], v[2], v[3]),
            PinValue::Texture2D(path) => format!("\"{}\"", path),
            PinValue::Sampler => "sampler".to_string(),
            PinValue::Entity(id) => format!("entity({})", id),
            PinValue::EntityArray(ids) => format!("[{}]", ids.iter().map(|id| format!("entity({})", id)).collect::<Vec<_>>().join(", ")),
            PinValue::StringArray(strings) => format!("[{}]", strings.iter().map(|s| format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))).collect::<Vec<_>>().join(", ")),
            PinValue::Asset(path) => format!("asset(\"{}\")", path),
            PinValue::AudioHandle(id) => format!("audio_handle({})", id),
            PinValue::TimerHandle(id) => format!("timer_handle({})", id),
            PinValue::SceneHandle(id) => format!("scene_handle({})", id),
            PinValue::PrefabHandle(id) => format!("prefab_handle({})", id),
            PinValue::GltfHandle(id) => format!("gltf_handle({})", id),
        }
    }

    /// Convert to WGSL code representation (for shader materials)
    pub fn to_wgsl(&self) -> String {
        match self {
            PinValue::Flow => String::new(),
            PinValue::Float(v) => format!("{:.6}", v),
            PinValue::Int(v) => format!("{}i", v),
            PinValue::Bool(v) => format!("{}", v),
            PinValue::String(v) => format!("\"{}\"", v),
            PinValue::Vec2(v) => format!("vec2<f32>({:.6}, {:.6})", v[0], v[1]),
            PinValue::Vec3(v) => format!("vec3<f32>({:.6}, {:.6}, {:.6})", v[0], v[1], v[2]),
            PinValue::Vec4(v) => format!("vec4<f32>({:.6}, {:.6}, {:.6}, {:.6})", v[0], v[1], v[2], v[3]),
            PinValue::Color(v) => format!("vec4<f32>({:.6}, {:.6}, {:.6}, {:.6})", v[0], v[1], v[2], v[3]),
            PinValue::Texture2D(_) => "/* texture */".to_string(),
            PinValue::Sampler => "/* sampler */".to_string(),
            // These types are not used in shaders
            PinValue::Entity(_) | PinValue::EntityArray(_) | PinValue::StringArray(_) |
            PinValue::Asset(_) | PinValue::AudioHandle(_) | PinValue::TimerHandle(_) |
            PinValue::SceneHandle(_) | PinValue::PrefabHandle(_) | PinValue::GltfHandle(_) => {
                "/* runtime type */".to_string()
            }
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
    /// Optional display name override (for render graph nodes)
    pub display_name: Option<String>,
    /// Optional header color override [r, g, b] (for render graph nodes)
    pub color: Option<[u8; 3]>,
}

impl BlueprintNode {
    /// Create a new node with the given type and ID (no pins initially)
    pub fn new(id: NodeId, node_type: impl Into<String>) -> Self {
        Self {
            id,
            node_type: node_type.into(),
            position: [0.0, 0.0],
            pins: Vec::new(),
            input_values: HashMap::new(),
            comment: None,
            display_name: None,
            color: None,
        }
    }

    /// Create a new node with the given type, ID, and pins
    pub fn with_pins(id: NodeId, node_type: impl Into<String>, pins: Vec<Pin>) -> Self {
        Self {
            id,
            node_type: node_type.into(),
            position: [0.0, 0.0],
            pins,
            input_values: HashMap::new(),
            comment: None,
            display_name: None,
            color: None,
        }
    }

    /// Set the position of this node (takes x, y)
    #[allow(dead_code)]
    pub fn with_position(mut self, pos: [f32; 2]) -> Self {
        self.position = pos;
        self
    }

    /// Set the position of this node (takes x, y separately)
    #[allow(dead_code)]
    pub fn with_position_xy(mut self, x: f32, y: f32) -> Self {
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
    #[allow(dead_code)]
    pub fn set_input_value(&mut self, pin_name: impl Into<String>, value: PinValue) {
        self.input_values.insert(pin_name.into(), value);
    }
}

/// Type of blueprint graph
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum BlueprintType {
    /// Behavior blueprint - compiles to Rhai script for entity logic
    #[default]
    Behavior,
    /// Material blueprint - compiles to WGSL shader for custom materials
    Material,
}

impl BlueprintType {
    /// Get display name for this type
    pub fn name(&self) -> &'static str {
        match self {
            BlueprintType::Behavior => "Behavior",
            BlueprintType::Material => "Material",
        }
    }

    /// Get file extension for this type
    pub fn extension(&self) -> &'static str {
        match self {
            BlueprintType::Behavior => "blueprint",
            BlueprintType::Material => "material_bp",
        }
    }

    /// Check if a node category is valid for this blueprint type
    pub fn is_category_allowed(&self, category: &str) -> bool {
        match self {
            BlueprintType::Behavior => {
                // Behavior blueprints use non-shader categories
                !category.starts_with("Shader") && !category.starts_with("Render")
            }
            BlueprintType::Material => {
                // Material blueprints use shader categories + shared Math nodes
                category.starts_with("Shader") || category == "Math"
            }
        }
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
    #[allow(dead_code)]
    pub fn new(name: impl Into<String>, var_type: PinType) -> Self {
        let name = name.into();
        let default_value = PinValue::default_for_type(var_type.clone());
        Self {
            display_name: name.clone(),
            name,
            var_type,
            default_value,
            description: String::new(),
            exposed: true,
        }
    }

    #[allow(dead_code)]
    pub fn with_default(mut self, value: PinValue) -> Self {
        self.default_value = value;
        self
    }

    #[allow(dead_code)]
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
    /// Type of blueprint (Behavior or Material)
    #[serde(default)]
    pub graph_type: BlueprintType,
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
            graph_type: BlueprintType::default(),
            nodes: Vec::new(),
            connections: Vec::new(),
            variables: Vec::new(),
            next_node_id: 1,
        }
    }
}

impl BlueprintGraph {
    /// Create a new empty behavior blueprint with the given name
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            graph_type: BlueprintType::Behavior,
            ..Default::default()
        }
    }

    /// Create a new empty material blueprint with the given name
    pub fn new_material(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            graph_type: BlueprintType::Material,
            ..Default::default()
        }
    }

    /// Create a new blueprint with the given name and type
    pub fn new_with_type(name: impl Into<String>, graph_type: BlueprintType) -> Self {
        Self {
            name: name.into(),
            graph_type,
            ..Default::default()
        }
    }

    /// Check if this is a material blueprint
    pub fn is_material(&self) -> bool {
        self.graph_type == BlueprintType::Material
    }

    /// Check if this is a behavior blueprint
    pub fn is_behavior(&self) -> bool {
        self.graph_type == BlueprintType::Behavior
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
    #[allow(dead_code)]
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
    #[allow(dead_code)]
    pub fn add_variable(&mut self, var: BlueprintVariable) {
        self.variables.push(var);
    }

    /// Remove a variable by name
    #[allow(dead_code)]
    pub fn remove_variable(&mut self, name: &str) {
        self.variables.retain(|v| v.name != name);
    }

    /// Get a variable by name
    #[allow(dead_code)]
    pub fn get_variable(&self, name: &str) -> Option<&BlueprintVariable> {
        self.variables.iter().find(|v| v.name == name)
    }

    /// Find event nodes (entry points) in the graph
    pub fn event_nodes(&self) -> impl Iterator<Item = &BlueprintNode> {
        self.nodes.iter().filter(|n| n.node_type.starts_with("event/"))
    }
}
