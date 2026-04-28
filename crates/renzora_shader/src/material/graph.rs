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
    /// String literal — stored on the node, not consumed by the WGSL pipeline.
    /// Used for things like parameter names that need to flow through the
    /// inspector without ever entering the shader.
    String,
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
            // Strings never reach WGSL; this only exists so exhaustiveness
            // holds at the type level.
            Self::String => "f32",
        }
    }

    /// Can `from` connect to `to`?
    /// All numeric/vector/color types are freely inter-convertible at the graph level;
    /// `cast_expr` inserts the correct WGSL coercion so the compiled shader type-checks.
    /// Strings are isolated — they only connect to other strings (which in
    /// practice never happens, since parameter-name pins aren't connectable
    /// targets), keeping accidental "Float → name" wires out of the graph.
    pub fn compatible(from: PinType, to: PinType) -> bool {
        if from == to {
            return true;
        }
        let numeric = |t: PinType| matches!(
            t,
            PinType::Float | PinType::Vec2 | PinType::Vec3 | PinType::Vec4 | PinType::Color
        );
        numeric(from) && numeric(to)
    }

    /// WGSL expression to cast `expr` from `from` type to `to` type.
    ///
    /// Widening copies the scalar into every component; narrowing takes the first
    /// components. Parentheses wrap `expr` so swizzles on composite expressions parse.
    pub fn cast_expr(from: PinType, to: PinType, expr: &str) -> String {
        if from == to {
            return expr.to_string();
        }
        // Treat Color as Vec4 for cast purposes.
        let eff = |t: PinType| match t {
            PinType::Color => PinType::Vec4,
            other => other,
        };
        let e = expr;
        match (eff(from), eff(to)) {
            // widening from scalar
            (PinType::Float, PinType::Vec2) => format!("vec2<f32>({e})"),
            (PinType::Float, PinType::Vec3) => format!("vec3<f32>({e})"),
            (PinType::Float, PinType::Vec4) => format!("vec4<f32>({e}, {e}, {e}, 1.0)"),

            // vec2 ↔ higher
            (PinType::Vec2, PinType::Float) => format!("({e}).x"),
            (PinType::Vec2, PinType::Vec3) => format!("vec3<f32>({e}, 0.0)"),
            (PinType::Vec2, PinType::Vec4) => format!("vec4<f32>({e}, 0.0, 1.0)"),

            // vec3 ↔ others
            (PinType::Vec3, PinType::Float) => format!("({e}).x"),
            (PinType::Vec3, PinType::Vec2) => format!("({e}).xy"),
            (PinType::Vec3, PinType::Vec4) => format!("vec4<f32>({e}, 1.0)"),

            // vec4 ↔ others
            (PinType::Vec4, PinType::Float) => format!("({e}).x"),
            (PinType::Vec4, PinType::Vec2) => format!("({e}).xy"),
            (PinType::Vec4, PinType::Vec3) => format!("({e}).xyz"),

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
    /// Arbitrary string (used by Custom Code node for its WGSL snippet,
    /// by subgraph-call nodes for the function asset path, etc).
    String(String),
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
            Self::String(_) => "0.0".to_string(), // strings don't codegen inline
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
            Self::String(_) => PinType::Float, // no dedicated type; stored out-of-band
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

/// Per-graph alpha behavior. Maps directly onto Bevy's `AlphaMode` at
/// resolve time. Default is `Opaque` so existing materials (which omit
/// the field on disk) continue to render unchanged.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum AlphaMode {
    Opaque,
    /// Discard fragments below `cutoff`. Used for foliage, masks.
    Mask { cutoff: f32 },
    /// Standard alpha blending. Used for glass, smoke, decals.
    Blend,
}

impl Default for AlphaMode {
    fn default() -> Self { Self::Opaque }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MaterialGraph {
    pub name: String,
    pub domain: MaterialDomain,
    pub nodes: Vec<MaterialNode>,
    pub connections: Vec<Connection>,
    next_id: u64,
    /// How transparency should be rendered. `#[serde(default)]` keeps old
    /// `.material` files (written before this field existed) loadable.
    #[serde(default)]
    pub alpha_mode: AlphaMode,
    /// Render back faces too. `#[serde(default)]` for the same reason.
    #[serde(default)]
    pub double_sided: bool,
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
            alpha_mode: AlphaMode::Opaque,
            double_sided: false,
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

    /// Extract texture paths from a material graph for terrain layer use.
    ///
    /// Traces connections from the output node's `base_color`, `normal`, and
    /// `metallic`/`roughness`/`ao` inputs back to texture sample nodes.
    pub fn extract_layer_textures(&self) -> LayerTextureSet {
        let mut result = LayerTextureSet::default();
        let Some(output) = self.output_node() else {
            return result;
        };
        let output_id = output.id;

        // Trace base_color → albedo texture
        result.albedo = self.trace_texture_path(output_id, "base_color");
        // Trace normal → normal map
        result.normal = self.trace_texture_path(output_id, "normal");
        // Trace metallic, roughness, or ao → ARM texture (any of them)
        result.arm = self
            .trace_texture_path(output_id, "metallic")
            .or_else(|| self.trace_texture_path(output_id, "roughness"))
            .or_else(|| self.trace_texture_path(output_id, "ao"));

        result
    }

    /// Trace a pin connection back through the graph to find a texture path.
    /// Follows one hop: output_pin → texture/sample node → TexturePath input value.
    fn trace_texture_path(&self, node_id: NodeId, pin_name: &str) -> Option<String> {
        let conn = self.connection_to(node_id, pin_name)?;
        let source_node = self.get_node(conn.from_node)?;

        // If the source is a texture sample node, extract the texture path
        if source_node.node_type.starts_with("texture/") {
            if let Some(PinValue::TexturePath(path)) = source_node.get_input_value("texture") {
                if !path.is_empty() {
                    return Some(path.clone());
                }
            }
        }
        None
    }
}

/// Texture paths extracted from a material graph for terrain layer use.
#[derive(Clone, Debug, Default)]
pub struct LayerTextureSet {
    pub albedo: Option<String>,
    pub normal: Option<String>,
    pub arm: Option<String>,
}

// ── Material Function (subgraph) ────────────────────────────────────────────
//
// A named, reusable subgraph. The internal graph must contain one
// `function/input_point` node (source of the function's 4 Vec4 inputs)
// and one `function/output_point` node (sink of the function's 4 Vec4 outputs).
// At compile time each function call inlines a WGSL helper at module scope
// and invokes it at the call site, so functions compose like normal nodes
// but stay visually encapsulated.

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MaterialFunction {
    pub name: String,
    pub graph: MaterialGraph,
}

impl MaterialFunction {
    pub fn new(name: impl Into<String>) -> Self {
        let name = name.into();
        let mut graph = MaterialGraph::new(&name, MaterialDomain::Unlit);
        // Remove the output-surface node the constructor added — functions
        // use function/output_point as their sink instead.
        graph.nodes.retain(|n| !n.node_type.starts_with("output/"));
        // Seed with the required input/output bracket nodes.
        graph.add_node("function/input_point", [-400.0, 0.0]);
        graph.add_node("function/output_point", [400.0, 0.0]);
        Self { name, graph }
    }

    pub fn output_point(&self) -> Option<&MaterialNode> {
        self.graph
            .nodes
            .iter()
            .find(|n| n.node_type == "function/output_point")
    }
}
