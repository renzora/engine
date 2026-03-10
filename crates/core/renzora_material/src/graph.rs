//! Material graph data model.
//!
//! Owns the node/connection data independently of the UI widget.
//! The editor syncs between this and `NodeGraphState`.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ── Identifiers ─────────────────────────────────────────────────────────────

pub type NodeId = u64;

// ── Pin types ───────────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum PinType {
    Float,
    Vec2,
    Vec3,
    Vec4,
    Color,
    Bool,
    Texture2D,
    Sampler,
}

impl PinType {
    /// WGSL type string for this pin type.
    pub fn wgsl_type(&self) -> &'static str {
        match self {
            Self::Float => "f32",
            Self::Vec2 => "vec2<f32>",
            Self::Vec3 => "vec3<f32>",
            Self::Vec4 | Self::Color => "vec4<f32>",
            Self::Bool => "bool",
            Self::Texture2D => "texture_2d<f32>",
            Self::Sampler => "sampler",
        }
    }

    /// Can `from` connect to `to`?
    pub fn compatible(from: PinType, to: PinType) -> bool {
        if from == to {
            return true;
        }
        // Implicit conversions
        matches!(
            (from, to),
            // Float widens to anything
            (PinType::Float, PinType::Vec2 | PinType::Vec3 | PinType::Vec4 | PinType::Color)
            // Vec3 <-> Color (rgb)
            | (PinType::Vec3, PinType::Color)
            | (PinType::Color, PinType::Vec3)
            // Vec4 <-> Color
            | (PinType::Vec4, PinType::Color)
            | (PinType::Color, PinType::Vec4)
        )
    }

    /// WGSL expression to cast `expr` from `from` type to `to` type.
    pub fn cast_expr(from: PinType, to: PinType, expr: &str) -> String {
        if from == to {
            return expr.to_string();
        }
        match (from, to) {
            (PinType::Float, PinType::Vec2) => format!("vec2<f32>({e}, {e})", e = expr),
            (PinType::Float, PinType::Vec3) => format!("vec3<f32>({e}, {e}, {e})", e = expr),
            (PinType::Float, PinType::Vec4 | PinType::Color) => {
                format!("vec4<f32>({e}, {e}, {e}, 1.0)", e = expr)
            }
            (PinType::Vec3, PinType::Color) => format!("vec4<f32>({e}, 1.0)", e = expr),
            (PinType::Vec4, PinType::Color) | (PinType::Color, PinType::Vec4) => expr.to_string(),
            (PinType::Color, PinType::Vec3) => format!("{e}.rgb", e = expr),
            _ => expr.to_string(),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum PinDir {
    Input,
    Output,
}

// ── Pin values ──────────────────────────────────────────────────────────────

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum PinValue {
    Float(f32),
    Vec2([f32; 2]),
    Vec3([f32; 3]),
    Vec4([f32; 4]),
    Color([f32; 4]),
    Bool(bool),
    Int(i32),
    /// Texture asset path.
    TexturePath(String),
    None,
}

impl Default for PinValue {
    fn default() -> Self {
        Self::None
    }
}

impl PinValue {
    /// Convert to a WGSL literal expression.
    pub fn to_wgsl(&self) -> String {
        match self {
            Self::Float(v) => format!("{:.6}", v),
            Self::Vec2([x, y]) => format!("vec2<f32>({:.6}, {:.6})", x, y),
            Self::Vec3([x, y, z]) => format!("vec3<f32>({:.6}, {:.6}, {:.6})", x, y, z),
            Self::Vec4([x, y, z, w]) | Self::Color([x, y, z, w]) => {
                format!("vec4<f32>({:.6}, {:.6}, {:.6}, {:.6})", x, y, z, w)
            }
            Self::Bool(b) => if *b { "true" } else { "false" }.to_string(),
            Self::Int(i) => format!("{}", i),
            Self::TexturePath(_) => "vec4<f32>(1.0, 0.0, 1.0, 1.0)".to_string(), // magenta fallback
            Self::None => "0.0".to_string(),
        }
    }

    pub fn pin_type(&self) -> PinType {
        match self {
            Self::Float(_) => PinType::Float,
            Self::Vec2(_) => PinType::Vec2,
            Self::Vec3(_) => PinType::Vec3,
            Self::Vec4(_) => PinType::Vec4,
            Self::Color(_) => PinType::Color,
            Self::Bool(_) => PinType::Bool,
            Self::Int(_) => PinType::Float,
            Self::TexturePath(_) => PinType::Texture2D,
            Self::None => PinType::Float,
        }
    }
}

// ── Pin template ────────────────────────────────────────────────────────────

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PinTemplate {
    pub name: String,
    pub label: String,
    pub pin_type: PinType,
    pub direction: PinDir,
    pub default_value: PinValue,
}

impl PinTemplate {
    pub fn input(name: &str, label: &str, pin_type: PinType) -> Self {
        Self {
            name: name.to_string(),
            label: label.to_string(),
            pin_type,
            direction: PinDir::Input,
            default_value: PinValue::None,
        }
    }

    pub fn output(name: &str, label: &str, pin_type: PinType) -> Self {
        Self {
            name: name.to_string(),
            label: label.to_string(),
            pin_type,
            direction: PinDir::Output,
            default_value: PinValue::None,
        }
    }

    pub fn with_default(mut self, value: PinValue) -> Self {
        self.default_value = value;
        self
    }
}

// ── Material domain ─────────────────────────────────────────────────────────

/// What kind of shader this material graph compiles to.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum MaterialDomain {
    /// Standard PBR surface material (meshes, props).
    Surface,
    /// Terrain layer — compiles to `layer_main()` / `layer_pbr()`.
    TerrainLayer,
    /// Vegetation — PBR + vertex displacement (wind, sway).
    Vegetation,
    /// Unlit — raw color output, no PBR lighting.
    Unlit,
}

impl MaterialDomain {
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Surface => "Surface",
            Self::TerrainLayer => "Terrain Layer",
            Self::Vegetation => "Vegetation",
            Self::Unlit => "Unlit",
        }
    }
}

impl Default for MaterialDomain {
    fn default() -> Self {
        Self::Surface
    }
}

// ── Connection ──────────────────────────────────────────────────────────────

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Connection {
    pub from_node: NodeId,
    pub from_pin: String,
    pub to_node: NodeId,
    pub to_pin: String,
}

// ── Material node ───────────────────────────────────────────────────────────

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MaterialNode {
    pub id: NodeId,
    pub node_type: String,
    pub position: [f32; 2],
    /// Override values for input pins (user-set constants).
    pub input_values: HashMap<String, PinValue>,
}

impl MaterialNode {
    pub fn new(id: NodeId, node_type: &str, position: [f32; 2]) -> Self {
        Self {
            id,
            node_type: node_type.to_string(),
            position,
            input_values: HashMap::new(),
        }
    }

    pub fn get_input_value(&self, pin_name: &str) -> Option<&PinValue> {
        self.input_values.get(pin_name)
    }
}

// ── Material graph ──────────────────────────────────────────────────────────

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MaterialGraph {
    pub name: String,
    pub domain: MaterialDomain,
    pub nodes: Vec<MaterialNode>,
    pub connections: Vec<Connection>,
    next_id: u64,
}

impl Default for MaterialGraph {
    fn default() -> Self {
        Self::new("New Material", MaterialDomain::Surface)
    }
}

impl MaterialGraph {
    pub fn new(name: &str, domain: MaterialDomain) -> Self {
        let mut graph = Self {
            name: name.to_string(),
            domain,
            nodes: Vec::new(),
            connections: Vec::new(),
            next_id: 1,
        };
        // Always start with the output node
        graph.add_output_node(domain);
        graph
    }

    fn add_output_node(&mut self, domain: MaterialDomain) {
        let node_type = match domain {
            MaterialDomain::Surface => "output/surface",
            MaterialDomain::TerrainLayer => "output/terrain_layer",
            MaterialDomain::Vegetation => "output/vegetation",
            MaterialDomain::Unlit => "output/unlit",
        };
        let id = self.next_id;
        self.next_id += 1;
        self.nodes.push(MaterialNode::new(id, node_type, [300.0, 0.0]));
    }

    pub fn add_node(&mut self, node_type: &str, position: [f32; 2]) -> NodeId {
        let id = self.next_id;
        self.next_id += 1;
        self.nodes.push(MaterialNode::new(id, node_type, position));
        id
    }

    pub fn remove_node(&mut self, id: NodeId) {
        // Don't remove output nodes
        if let Some(node) = self.get_node(id) {
            if node.node_type.starts_with("output/") {
                return;
            }
        }
        self.nodes.retain(|n| n.id != id);
        self.connections
            .retain(|c| c.from_node != id && c.to_node != id);
    }

    pub fn connect(
        &mut self,
        from_node: NodeId,
        from_pin: &str,
        to_node: NodeId,
        to_pin: &str,
    ) {
        // Remove existing connection to this input (inputs accept only one connection)
        self.connections
            .retain(|c| !(c.to_node == to_node && c.to_pin == to_pin));
        self.connections.push(Connection {
            from_node,
            from_pin: from_pin.to_string(),
            to_node,
            to_pin: to_pin.to_string(),
        });
    }

    pub fn disconnect(&mut self, node_id: NodeId, pin_name: &str) {
        self.connections.retain(|c| {
            !((c.from_node == node_id && c.from_pin == pin_name)
                || (c.to_node == node_id && c.to_pin == pin_name))
        });
    }

    pub fn get_node(&self, id: NodeId) -> Option<&MaterialNode> {
        self.nodes.iter().find(|n| n.id == id)
    }

    pub fn get_node_mut(&mut self, id: NodeId) -> Option<&mut MaterialNode> {
        self.nodes.iter_mut().find(|n| n.id == id)
    }

    /// Find the output node for this graph.
    pub fn output_node(&self) -> Option<&MaterialNode> {
        self.nodes.iter().find(|n| n.node_type.starts_with("output/"))
    }

    /// Find which output pin connects to a given input.
    pub fn connection_to(&self, to_node: NodeId, to_pin: &str) -> Option<&Connection> {
        self.connections
            .iter()
            .find(|c| c.to_node == to_node && c.to_pin == to_pin)
    }
}
