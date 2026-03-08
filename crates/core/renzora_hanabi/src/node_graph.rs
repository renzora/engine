use serde::{Deserialize, Serialize};
use bevy::prelude::*;
use std::collections::HashMap;

// Pin types with associated colors (used by editor)
#[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Debug, Reflect)]
#[reflect(Serialize, Deserialize)]
pub enum PinType {
    Float,
    Vec2,
    Vec3,
    Vec4,
    Bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Debug, Reflect)]
#[reflect(Serialize, Deserialize)]
pub enum PinDir {
    Input,
    Output,
}

#[derive(Clone, Serialize, Deserialize, Debug, Reflect)]
#[reflect(Serialize, Deserialize)]
pub enum PinValue {
    Float(f32),
    Vec2([f32; 2]),
    Vec3([f32; 3]),
    Vec4([f32; 4]),
    Bool(bool),
    None,
}

impl Default for PinValue {
    fn default() -> Self { Self::None }
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct PinTemplate {
    pub name: String,
    pub label: String,
    pub pin_type: PinType,
    pub direction: PinDir,
    pub default_value: PinValue,
}

// All the node types available in the graph
#[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Debug, Reflect)]
#[reflect(Serialize, Deserialize)]
pub enum ParticleNodeType {
    // Structural
    Emitter,

    // Spawn
    SpawnRate,
    SpawnBurst,

    // Init
    InitPosition,
    InitVelocity,
    InitSize,
    InitLifetime,
    InitColor,

    // Update
    Gravity,
    LinearDrag,
    RadialAccel,
    TangentAccel,
    ConformToSphere,
    KillSphere,
    KillAabb,
    Noise,
    Orbit,
    VelocityLimit,

    // Render
    SizeOverLifetime,
    ColorOverLifetime,
    Orient,
    Texture,

    // Math
    Add,
    Subtract,
    Multiply,
    Divide,
    Lerp,
    Clamp,
    RandomRange,
    Sin,
    Cos,
    Abs,
    Negate,
    SplitVec3,
    CombineVec3,

    // Constants / Inputs
    FloatConstant,
    Vec3Constant,
    Vec4Constant,
    Time,
    ParticleAge,
    DeltaTime,
}

impl ParticleNodeType {
    /// Returns the category name for UI grouping
    pub fn category(&self) -> &'static str {
        match self {
            Self::Emitter => "Emitter",
            Self::SpawnRate | Self::SpawnBurst => "Spawn",
            Self::InitPosition | Self::InitVelocity | Self::InitSize |
            Self::InitLifetime | Self::InitColor => "Init",
            Self::Gravity | Self::LinearDrag | Self::RadialAccel |
            Self::TangentAccel | Self::ConformToSphere | Self::KillSphere |
            Self::KillAabb | Self::Noise | Self::Orbit | Self::VelocityLimit => "Update",
            Self::SizeOverLifetime | Self::ColorOverLifetime | Self::Orient |
            Self::Texture => "Render",
            Self::Add | Self::Subtract | Self::Multiply | Self::Divide |
            Self::Lerp | Self::Clamp | Self::RandomRange | Self::Sin |
            Self::Cos | Self::Abs | Self::Negate | Self::SplitVec3 |
            Self::CombineVec3 => "Math",
            Self::FloatConstant | Self::Vec3Constant | Self::Vec4Constant |
            Self::Time | Self::ParticleAge | Self::DeltaTime => "Constants",
        }
    }

    /// Display name for the node
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Emitter => "Emitter",
            Self::SpawnRate => "Spawn Rate",
            Self::SpawnBurst => "Spawn Burst",
            Self::InitPosition => "Set Position",
            Self::InitVelocity => "Set Velocity",
            Self::InitSize => "Set Size",
            Self::InitLifetime => "Set Lifetime",
            Self::InitColor => "Set Color",
            Self::Gravity => "Gravity",
            Self::LinearDrag => "Linear Drag",
            Self::RadialAccel => "Radial Accel",
            Self::TangentAccel => "Tangent Accel",
            Self::ConformToSphere => "Conform to Sphere",
            Self::KillSphere => "Kill Sphere",
            Self::KillAabb => "Kill AABB",
            Self::Noise => "Noise Turbulence",
            Self::Orbit => "Orbit",
            Self::VelocityLimit => "Velocity Limit",
            Self::SizeOverLifetime => "Size Over Lifetime",
            Self::ColorOverLifetime => "Color Over Lifetime",
            Self::Orient => "Orient",
            Self::Texture => "Texture",
            Self::Add => "Add",
            Self::Subtract => "Subtract",
            Self::Multiply => "Multiply",
            Self::Divide => "Divide",
            Self::Lerp => "Lerp",
            Self::Clamp => "Clamp",
            Self::RandomRange => "Random Range",
            Self::Sin => "Sin",
            Self::Cos => "Cos",
            Self::Abs => "Abs",
            Self::Negate => "Negate",
            Self::SplitVec3 => "Split Vec3",
            Self::CombineVec3 => "Combine Vec3",
            Self::FloatConstant => "Float",
            Self::Vec3Constant => "Vec3",
            Self::Vec4Constant => "Vec4 / Color",
            Self::Time => "Time",
            Self::ParticleAge => "Particle Age",
            Self::DeltaTime => "Delta Time",
        }
    }

    /// Get pin templates for this node type
    pub fn pins(&self) -> Vec<PinTemplate> {
        match self {
            Self::Emitter => vec![
                pin_in("capacity", "Capacity", PinType::Float, PinValue::Float(1000.0)),
                pin_in("spawn", "Spawn", PinType::Float, PinValue::None),
                pin_in("init", "Init", PinType::Float, PinValue::None),
                pin_in("update", "Update", PinType::Float, PinValue::None),
                pin_in("render", "Render", PinType::Float, PinValue::None),
            ],

            Self::SpawnRate => vec![
                pin_in("rate", "Rate", PinType::Float, PinValue::Float(50.0)),
                pin_out("module", "Module", PinType::Float),
            ],
            Self::SpawnBurst => vec![
                pin_in("count", "Count", PinType::Float, PinValue::Float(10.0)),
                pin_out("module", "Module", PinType::Float),
            ],

            Self::InitPosition => vec![
                pin_in("shape", "Shape", PinType::Vec3, PinValue::Vec3([0.0, 0.0, 0.0])),
                pin_in("radius", "Radius", PinType::Float, PinValue::Float(1.0)),
                pin_out("module", "Module", PinType::Float),
            ],
            Self::InitVelocity => vec![
                pin_in("direction", "Direction", PinType::Vec3, PinValue::Vec3([0.0, 1.0, 0.0])),
                pin_in("speed", "Speed", PinType::Float, PinValue::Float(2.0)),
                pin_in("spread", "Spread", PinType::Float, PinValue::Float(0.3)),
                pin_out("module", "Module", PinType::Float),
            ],
            Self::InitSize => vec![
                pin_in("size", "Size", PinType::Float, PinValue::Float(0.1)),
                pin_in("random_min", "Random Min", PinType::Float, PinValue::Float(0.0)),
                pin_in("random_max", "Random Max", PinType::Float, PinValue::Float(0.0)),
                pin_out("module", "Module", PinType::Float),
            ],
            Self::InitLifetime => vec![
                pin_in("min", "Min", PinType::Float, PinValue::Float(1.0)),
                pin_in("max", "Max", PinType::Float, PinValue::Float(2.0)),
                pin_out("module", "Module", PinType::Float),
            ],
            Self::InitColor => vec![
                pin_in("color", "Color", PinType::Vec4, PinValue::Vec4([1.0, 1.0, 1.0, 1.0])),
                pin_out("module", "Module", PinType::Float),
            ],

            Self::Gravity => vec![
                pin_in("acceleration", "Acceleration", PinType::Vec3, PinValue::Vec3([0.0, -9.81, 0.0])),
                pin_out("module", "Module", PinType::Float),
            ],
            Self::LinearDrag => vec![
                pin_in("drag", "Drag", PinType::Float, PinValue::Float(1.0)),
                pin_out("module", "Module", PinType::Float),
            ],
            Self::RadialAccel => vec![
                pin_in("origin", "Origin", PinType::Vec3, PinValue::Vec3([0.0, 0.0, 0.0])),
                pin_in("acceleration", "Acceleration", PinType::Float, PinValue::Float(1.0)),
                pin_out("module", "Module", PinType::Float),
            ],
            Self::TangentAccel => vec![
                pin_in("origin", "Origin", PinType::Vec3, PinValue::Vec3([0.0, 0.0, 0.0])),
                pin_in("axis", "Axis", PinType::Vec3, PinValue::Vec3([0.0, 1.0, 0.0])),
                pin_in("acceleration", "Acceleration", PinType::Float, PinValue::Float(1.0)),
                pin_out("module", "Module", PinType::Float),
            ],
            Self::ConformToSphere => vec![
                pin_in("origin", "Origin", PinType::Vec3, PinValue::Vec3([0.0, 0.0, 0.0])),
                pin_in("radius", "Radius", PinType::Float, PinValue::Float(1.0)),
                pin_in("influence_dist", "Influence", PinType::Float, PinValue::Float(3.0)),
                pin_in("accel", "Accel", PinType::Float, PinValue::Float(5.0)),
                pin_in("max_speed", "Max Speed", PinType::Float, PinValue::Float(2.0)),
                pin_out("module", "Module", PinType::Float),
            ],
            Self::KillSphere => vec![
                pin_in("center", "Center", PinType::Vec3, PinValue::Vec3([0.0, 0.0, 0.0])),
                pin_in("radius", "Radius", PinType::Float, PinValue::Float(5.0)),
                pin_in("kill_inside", "Kill Inside", PinType::Bool, PinValue::Bool(false)),
                pin_out("module", "Module", PinType::Float),
            ],
            Self::KillAabb => vec![
                pin_in("center", "Center", PinType::Vec3, PinValue::Vec3([0.0, 0.0, 0.0])),
                pin_in("half_size", "Half Size", PinType::Vec3, PinValue::Vec3([5.0, 5.0, 5.0])),
                pin_in("kill_inside", "Kill Inside", PinType::Bool, PinValue::Bool(false)),
                pin_out("module", "Module", PinType::Float),
            ],
            Self::Noise => vec![
                pin_in("frequency", "Frequency", PinType::Float, PinValue::Float(1.0)),
                pin_in("amplitude", "Amplitude", PinType::Float, PinValue::Float(1.0)),
                pin_out("module", "Module", PinType::Float),
            ],
            Self::Orbit => vec![
                pin_in("center", "Center", PinType::Vec3, PinValue::Vec3([0.0, 0.0, 0.0])),
                pin_in("axis", "Axis", PinType::Vec3, PinValue::Vec3([0.0, 1.0, 0.0])),
                pin_in("speed", "Speed", PinType::Float, PinValue::Float(1.0)),
                pin_out("module", "Module", PinType::Float),
            ],
            Self::VelocityLimit => vec![
                pin_in("max_speed", "Max Speed", PinType::Float, PinValue::Float(10.0)),
                pin_out("module", "Module", PinType::Float),
            ],

            Self::SizeOverLifetime => vec![
                pin_in("start", "Start", PinType::Float, PinValue::Float(0.1)),
                pin_in("end", "End", PinType::Float, PinValue::Float(0.0)),
                pin_out("module", "Module", PinType::Float),
            ],
            Self::ColorOverLifetime => vec![
                pin_in("start_color", "Start", PinType::Vec4, PinValue::Vec4([1.0, 1.0, 1.0, 1.0])),
                pin_in("end_color", "End", PinType::Vec4, PinValue::Vec4([1.0, 1.0, 1.0, 0.0])),
                pin_out("module", "Module", PinType::Float),
            ],
            Self::Orient => vec![
                pin_in("rotation_speed", "Rotation Speed", PinType::Float, PinValue::Float(0.0)),
                pin_out("module", "Module", PinType::Float),
            ],
            Self::Texture => vec![
                pin_in("path", "Path", PinType::Float, PinValue::None),
                pin_out("module", "Module", PinType::Float),
            ],

            // Math nodes
            Self::Add => vec![
                pin_in("a", "A", PinType::Float, PinValue::Float(0.0)),
                pin_in("b", "B", PinType::Float, PinValue::Float(0.0)),
                pin_out("result", "Result", PinType::Float),
            ],
            Self::Subtract => vec![
                pin_in("a", "A", PinType::Float, PinValue::Float(0.0)),
                pin_in("b", "B", PinType::Float, PinValue::Float(0.0)),
                pin_out("result", "Result", PinType::Float),
            ],
            Self::Multiply => vec![
                pin_in("a", "A", PinType::Float, PinValue::Float(1.0)),
                pin_in("b", "B", PinType::Float, PinValue::Float(1.0)),
                pin_out("result", "Result", PinType::Float),
            ],
            Self::Divide => vec![
                pin_in("a", "A", PinType::Float, PinValue::Float(1.0)),
                pin_in("b", "B", PinType::Float, PinValue::Float(1.0)),
                pin_out("result", "Result", PinType::Float),
            ],
            Self::Lerp => vec![
                pin_in("a", "A", PinType::Float, PinValue::Float(0.0)),
                pin_in("b", "B", PinType::Float, PinValue::Float(1.0)),
                pin_in("t", "T", PinType::Float, PinValue::Float(0.5)),
                pin_out("result", "Result", PinType::Float),
            ],
            Self::Clamp => vec![
                pin_in("value", "Value", PinType::Float, PinValue::Float(0.0)),
                pin_in("min", "Min", PinType::Float, PinValue::Float(0.0)),
                pin_in("max", "Max", PinType::Float, PinValue::Float(1.0)),
                pin_out("result", "Result", PinType::Float),
            ],
            Self::RandomRange => vec![
                pin_in("min", "Min", PinType::Float, PinValue::Float(0.0)),
                pin_in("max", "Max", PinType::Float, PinValue::Float(1.0)),
                pin_out("result", "Result", PinType::Float),
            ],
            Self::Sin => vec![
                pin_in("value", "Value", PinType::Float, PinValue::Float(0.0)),
                pin_out("result", "Result", PinType::Float),
            ],
            Self::Cos => vec![
                pin_in("value", "Value", PinType::Float, PinValue::Float(0.0)),
                pin_out("result", "Result", PinType::Float),
            ],
            Self::Abs => vec![
                pin_in("value", "Value", PinType::Float, PinValue::Float(0.0)),
                pin_out("result", "Result", PinType::Float),
            ],
            Self::Negate => vec![
                pin_in("value", "Value", PinType::Float, PinValue::Float(0.0)),
                pin_out("result", "Result", PinType::Float),
            ],
            Self::SplitVec3 => vec![
                pin_in("vec", "Vec3", PinType::Vec3, PinValue::Vec3([0.0, 0.0, 0.0])),
                pin_out("x", "X", PinType::Float),
                pin_out("y", "Y", PinType::Float),
                pin_out("z", "Z", PinType::Float),
            ],
            Self::CombineVec3 => vec![
                pin_in("x", "X", PinType::Float, PinValue::Float(0.0)),
                pin_in("y", "Y", PinType::Float, PinValue::Float(0.0)),
                pin_in("z", "Z", PinType::Float, PinValue::Float(0.0)),
                pin_out("result", "Result", PinType::Vec3),
            ],

            Self::FloatConstant => vec![
                pin_out("value", "Value", PinType::Float),
            ],
            Self::Vec3Constant => vec![
                pin_out("value", "Value", PinType::Vec3),
            ],
            Self::Vec4Constant => vec![
                pin_out("value", "Value", PinType::Vec4),
            ],
            Self::Time => vec![
                pin_out("time", "Time", PinType::Float),
            ],
            Self::ParticleAge => vec![
                pin_out("age", "Age (0-1)", PinType::Float),
            ],
            Self::DeltaTime => vec![
                pin_out("dt", "Delta Time", PinType::Float),
            ],
        }
    }

    /// Get all node types for a given category
    pub fn nodes_in_category(category: &str) -> Vec<Self> {
        ALL_NODE_TYPES.iter()
            .filter(|n| n.category() == category)
            .copied()
            .collect()
    }

    /// All categories in display order
    pub fn categories() -> &'static [&'static str] {
        &["Spawn", "Init", "Update", "Render", "Math", "Constants"]
    }
}

const ALL_NODE_TYPES: &[ParticleNodeType] = &[
    ParticleNodeType::SpawnRate, ParticleNodeType::SpawnBurst,
    ParticleNodeType::InitPosition, ParticleNodeType::InitVelocity,
    ParticleNodeType::InitSize, ParticleNodeType::InitLifetime, ParticleNodeType::InitColor,
    ParticleNodeType::Gravity, ParticleNodeType::LinearDrag,
    ParticleNodeType::RadialAccel, ParticleNodeType::TangentAccel,
    ParticleNodeType::ConformToSphere, ParticleNodeType::KillSphere,
    ParticleNodeType::KillAabb, ParticleNodeType::Noise, ParticleNodeType::Orbit,
    ParticleNodeType::VelocityLimit,
    ParticleNodeType::SizeOverLifetime, ParticleNodeType::ColorOverLifetime,
    ParticleNodeType::Orient, ParticleNodeType::Texture,
    ParticleNodeType::Add, ParticleNodeType::Subtract,
    ParticleNodeType::Multiply, ParticleNodeType::Divide,
    ParticleNodeType::Lerp, ParticleNodeType::Clamp,
    ParticleNodeType::RandomRange, ParticleNodeType::Sin, ParticleNodeType::Cos,
    ParticleNodeType::Abs, ParticleNodeType::Negate,
    ParticleNodeType::SplitVec3, ParticleNodeType::CombineVec3,
    ParticleNodeType::FloatConstant, ParticleNodeType::Vec3Constant,
    ParticleNodeType::Vec4Constant, ParticleNodeType::Time,
    ParticleNodeType::ParticleAge, ParticleNodeType::DeltaTime,
];

// Helper functions for building pin templates
fn pin_in(name: &str, label: &str, pin_type: PinType, default: PinValue) -> PinTemplate {
    PinTemplate {
        name: name.to_string(),
        label: label.to_string(),
        pin_type,
        direction: PinDir::Input,
        default_value: default,
    }
}

fn pin_out(name: &str, label: &str, pin_type: PinType) -> PinTemplate {
    PinTemplate {
        name: name.to_string(),
        label: label.to_string(),
        pin_type,
        direction: PinDir::Output,
        default_value: PinValue::None,
    }
}

// Node instance in the graph
#[derive(Clone, Serialize, Deserialize, Debug, Reflect)]
#[reflect(Serialize, Deserialize)]
pub struct ParticleNode {
    pub id: u64,
    pub node_type: ParticleNodeType,
    pub position: [f32; 2],
    #[reflect(ignore)]
    pub values: HashMap<String, PinValue>,
}

// Connection between nodes
#[derive(Clone, Serialize, Deserialize, Debug, Reflect)]
#[reflect(Serialize, Deserialize)]
pub struct NodeConnection {
    pub from_node: u64,
    pub from_pin: String,
    pub to_node: u64,
    pub to_pin: String,
}

// The complete graph
#[derive(Clone, Serialize, Deserialize, Debug, Default, Reflect)]
#[reflect(Serialize, Deserialize)]
pub struct ParticleNodeGraph {
    pub name: String,
    pub nodes: Vec<ParticleNode>,
    pub connections: Vec<NodeConnection>,
    pub next_id: u64,
}

impl ParticleNodeGraph {
    pub fn new(name: &str) -> Self {
        let emitter = ParticleNode {
            id: 1,
            node_type: ParticleNodeType::Emitter,
            position: [0.0, 0.0],
            values: HashMap::new(),
        };
        Self {
            name: name.to_string(),
            nodes: vec![emitter],
            connections: Vec::new(),
            next_id: 2,
        }
    }

    pub fn add_node(&mut self, node_type: ParticleNodeType, position: [f32; 2]) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        self.nodes.push(ParticleNode {
            id,
            node_type,
            position,
            values: HashMap::new(),
        });
        id
    }

    pub fn remove_node(&mut self, id: u64) {
        self.nodes.retain(|n| n.id != id);
        self.connections.retain(|c| c.from_node != id && c.to_node != id);
    }

    pub fn connect(&mut self, from_node: u64, from_pin: &str, to_node: u64, to_pin: &str) {
        // Allow multiple connections to Emitter category pins; otherwise replace existing
        let is_emitter_category_pin = self.get_node(to_node)
            .map_or(false, |n| n.node_type == ParticleNodeType::Emitter)
            && ["spawn", "init", "update", "render"].contains(&to_pin);
        if !is_emitter_category_pin {
            self.connections.retain(|c| !(c.to_node == to_node && c.to_pin == to_pin));
        }
        self.connections.push(NodeConnection {
            from_node,
            from_pin: from_pin.to_string(),
            to_node,
            to_pin: to_pin.to_string(),
        });
    }

    pub fn disconnect(&mut self, node_id: u64, pin_name: &str) {
        self.connections.retain(|c| {
            !((c.from_node == node_id && c.from_pin == pin_name) ||
              (c.to_node == node_id && c.to_pin == pin_name))
        });
    }

    pub fn get_node(&self, id: u64) -> Option<&ParticleNode> {
        self.nodes.iter().find(|n| n.id == id)
    }

    pub fn get_node_mut(&mut self, id: u64) -> Option<&mut ParticleNode> {
        self.nodes.iter_mut().find(|n| n.id == id)
    }

    /// Get the value feeding into a specific input pin (follows connection or uses default)
    /// Build a node graph from an existing HanabiEffectDefinition.
    /// Creates nodes for each active feature with their values populated.
    pub fn from_effect(def: &crate::data::HanabiEffectDefinition) -> Self {
        let mut graph = Self::new(&def.name);
        // Set emitter capacity
        if let Some(emitter) = graph.get_node_mut(1) {
            emitter.values.insert("capacity".into(), PinValue::Float(def.capacity as f32));
        }

        let mut col_x: f32 = -300.0;
        let mut y_offset = |col: &mut f32| -> [f32; 2] { let y = *col; *col += 80.0; [0.0, y] };

        let emitter_id: u64 = 1;

        // Spawn
        col_x = -300.0;
        match def.spawn_mode {
            crate::data::SpawnMode::Rate => {
                let id = graph.add_node(ParticleNodeType::SpawnRate, [-300.0, { col_x += 80.0; col_x - 80.0 }]);
                if let Some(n) = graph.get_node_mut(id) {
                    n.values.insert("rate".into(), PinValue::Float(def.spawn_rate));
                }
                graph.connect(id, "module", emitter_id, "spawn");
            }
            crate::data::SpawnMode::Burst | crate::data::SpawnMode::BurstRate => {
                let id = graph.add_node(ParticleNodeType::SpawnBurst, [-300.0, { col_x += 80.0; col_x - 80.0 }]);
                if let Some(n) = graph.get_node_mut(id) {
                    n.values.insert("count".into(), PinValue::Float(def.spawn_count as f32));
                }
                graph.connect(id, "module", emitter_id, "spawn");
            }
        }

        // Init column
        let mut init_y: f32 = -200.0;
        let init_x: f32 = -600.0;

        // Lifetime
        let id = graph.add_node(ParticleNodeType::InitLifetime, [init_x, init_y]);
        if let Some(n) = graph.get_node_mut(id) {
            n.values.insert("min".into(), PinValue::Float(def.lifetime_min));
            n.values.insert("max".into(), PinValue::Float(def.lifetime_max));
        }
        graph.connect(id, "module", emitter_id, "init");
        init_y += 80.0;

        // Velocity
        let id = graph.add_node(ParticleNodeType::InitVelocity, [init_x, init_y]);
        if let Some(n) = graph.get_node_mut(id) {
            n.values.insert("direction".into(), PinValue::Vec3(def.velocity_direction));
            n.values.insert("speed".into(), PinValue::Float(def.velocity_magnitude));
            n.values.insert("spread".into(), PinValue::Float(def.velocity_spread));
        }
        graph.connect(id, "module", emitter_id, "init");
        init_y += 80.0;

        // Size
        let id = graph.add_node(ParticleNodeType::InitSize, [init_x, init_y]);
        if let Some(n) = graph.get_node_mut(id) {
            n.values.insert("size".into(), PinValue::Float(def.size_start));
        }
        graph.connect(id, "module", emitter_id, "init");
        init_y += 80.0;

        // Update column
        let mut update_y: f32 = -200.0;
        let update_x: f32 = 0.0;

        // Gravity
        if def.acceleration != [0.0, 0.0, 0.0] {
            let id = graph.add_node(ParticleNodeType::Gravity, [update_x, update_y]);
            if let Some(n) = graph.get_node_mut(id) {
                n.values.insert("acceleration".into(), PinValue::Vec3(def.acceleration));
            }
            graph.connect(id, "module", emitter_id, "update");
            update_y += 80.0;
        }

        // Linear drag
        if def.linear_drag > 0.001 {
            let id = graph.add_node(ParticleNodeType::LinearDrag, [update_x, update_y]);
            if let Some(n) = graph.get_node_mut(id) {
                n.values.insert("drag".into(), PinValue::Float(def.linear_drag));
            }
            graph.connect(id, "module", emitter_id, "update");
            update_y += 80.0;
        }

        // Noise
        if def.noise_amplitude > 0.001 && def.noise_frequency > 0.001 {
            let id = graph.add_node(ParticleNodeType::Noise, [update_x, update_y]);
            if let Some(n) = graph.get_node_mut(id) {
                n.values.insert("frequency".into(), PinValue::Float(def.noise_frequency));
                n.values.insert("amplitude".into(), PinValue::Float(def.noise_amplitude));
            }
            graph.connect(id, "module", emitter_id, "update");
            update_y += 80.0;
        }

        // Orbit
        if let Some(ref orbit) = def.orbit {
            let id = graph.add_node(ParticleNodeType::Orbit, [update_x, update_y]);
            if let Some(n) = graph.get_node_mut(id) {
                n.values.insert("center".into(), PinValue::Vec3(orbit.center));
                n.values.insert("axis".into(), PinValue::Vec3(orbit.axis));
                n.values.insert("speed".into(), PinValue::Float(orbit.speed));
            }
            graph.connect(id, "module", emitter_id, "update");
            update_y += 80.0;
        }

        // Velocity limit
        if def.velocity_limit > 0.001 {
            let id = graph.add_node(ParticleNodeType::VelocityLimit, [update_x, update_y]);
            if let Some(n) = graph.get_node_mut(id) {
                n.values.insert("max_speed".into(), PinValue::Float(def.velocity_limit));
            }
            graph.connect(id, "module", emitter_id, "update");
            update_y += 80.0;
        }

        // Render column
        let mut render_y: f32 = -200.0;
        let render_x: f32 = 300.0;

        // Size over lifetime
        let id = graph.add_node(ParticleNodeType::SizeOverLifetime, [render_x, render_y]);
        if let Some(n) = graph.get_node_mut(id) {
            n.values.insert("start".into(), PinValue::Float(def.size_start));
            n.values.insert("end".into(), PinValue::Float(def.size_end));
        }
        graph.connect(id, "module", emitter_id, "render");
        render_y += 80.0;

        // Color over lifetime
        if !def.use_flat_color && def.color_gradient.len() >= 2 {
            let id = graph.add_node(ParticleNodeType::ColorOverLifetime, [render_x, render_y]);
            if let Some(n) = graph.get_node_mut(id) {
                n.values.insert("start_color".into(), PinValue::Vec4(def.color_gradient[0].color));
                let last = &def.color_gradient[def.color_gradient.len() - 1];
                n.values.insert("end_color".into(), PinValue::Vec4(last.color));
            }
            graph.connect(id, "module", emitter_id, "render");
            render_y += 80.0;
        }

        // Orient
        if def.rotation_speed.abs() > 0.001 {
            let id = graph.add_node(ParticleNodeType::Orient, [render_x, render_y]);
            if let Some(n) = graph.get_node_mut(id) {
                n.values.insert("rotation_speed".into(), PinValue::Float(def.rotation_speed));
            }
            graph.connect(id, "module", emitter_id, "render");
            render_y += 80.0;
        }

        let _ = (y_offset, init_y, update_y, render_y);
        graph
    }

    /// Compile the node graph into a HanabiEffectDefinition.
    /// Only nodes connected (directly or indirectly) to the Emitter are compiled.
    pub fn compile_to_definition(&self) -> crate::data::HanabiEffectDefinition {
        use crate::data::*;

        let mut def = HanabiEffectDefinition::default();
        def.name = self.name.clone();

        // Find emitter node
        let emitter_id = match self.nodes.iter().find(|n| n.node_type == ParticleNodeType::Emitter) {
            Some(n) => n.id,
            None => return def,
        };

        // Get emitter capacity
        if let Some(emitter) = self.get_node(emitter_id) {
            if let Some(PinValue::Float(v)) = emitter.values.get("capacity") {
                def.capacity = *v as u32;
            }
        }

        // Find all nodes connected to emitter by category
        let connected_nodes = |category_pin: &str| -> Vec<&ParticleNode> {
            self.connections.iter()
                .filter(|c| c.to_node == emitter_id && c.to_pin == category_pin)
                .filter_map(|c| self.get_node(c.from_node))
                .collect()
        };

        // Helper to get a node's effective float value for a pin
        let node_float = |node: &ParticleNode, pin: &str, default: f32| -> f32 {
            match node.values.get(pin) {
                Some(PinValue::Float(v)) => *v,
                _ => default,
            }
        };
        let node_vec3 = |node: &ParticleNode, pin: &str, default: [f32; 3]| -> [f32; 3] {
            match node.values.get(pin) {
                Some(PinValue::Vec3(v)) => *v,
                _ => default,
            }
        };
        let node_vec4 = |node: &ParticleNode, pin: &str, default: [f32; 4]| -> [f32; 4] {
            match node.values.get(pin) {
                Some(PinValue::Vec4(v)) => *v,
                _ => default,
            }
        };
        let node_bool = |node: &ParticleNode, pin: &str, default: bool| -> bool {
            match node.values.get(pin) {
                Some(PinValue::Bool(v)) => *v,
                _ => default,
            }
        };

        // Zero out defaults — only connected nodes contribute
        def.spawn_rate = 0.0;
        def.spawn_count = 0;
        def.lifetime_min = 0.0;
        def.lifetime_max = 0.0;
        def.velocity_magnitude = 0.0;
        def.velocity_spread = 0.0;
        def.velocity_direction = [0.0, 0.0, 0.0];
        def.size_start = 0.0;
        def.size_end = 0.0;
        def.acceleration = [0.0, 0.0, 0.0];
        def.linear_drag = 0.0;
        def.color_gradient = vec![
            GradientStop { position: 0.0, color: [1.0, 1.0, 1.0, 1.0] },
            GradientStop { position: 1.0, color: [1.0, 1.0, 1.0, 1.0] },
        ];

        // Compile spawn nodes
        for node in connected_nodes("spawn") {
            match node.node_type {
                ParticleNodeType::SpawnRate => {
                    def.spawn_mode = SpawnMode::Rate;
                    def.spawn_rate = node_float(node, "rate", 50.0);
                }
                ParticleNodeType::SpawnBurst => {
                    def.spawn_mode = SpawnMode::Burst;
                    def.spawn_count = node_float(node, "count", 10.0) as u32;
                }
                _ => {}
            }
        }

        // Compile init nodes
        for node in connected_nodes("init") {
            match node.node_type {
                ParticleNodeType::InitLifetime => {
                    def.lifetime_min = node_float(node, "min", 1.0);
                    def.lifetime_max = node_float(node, "max", 2.0);
                }
                ParticleNodeType::InitVelocity => {
                    def.velocity_direction = node_vec3(node, "direction", [0.0, 1.0, 0.0]);
                    def.velocity_magnitude = node_float(node, "speed", 2.0);
                    def.velocity_spread = node_float(node, "spread", 0.3);
                }
                ParticleNodeType::InitSize => {
                    def.size_start = node_float(node, "size", 0.1);
                }
                ParticleNodeType::InitColor => {
                    let c = node_vec4(node, "color", [1.0, 1.0, 1.0, 1.0]);
                    def.use_flat_color = true;
                    def.flat_color = c;
                }
                ParticleNodeType::InitPosition => {
                    // Position shape handled via emit_shape - basic sphere for now
                    let radius = node_float(node, "radius", 1.0);
                    if radius > 0.001 {
                        def.emit_shape = HanabiEmitShape::Sphere {
                            radius,
                            dimension: ShapeDimension::Volume,
                        };
                    }
                }
                _ => {}
            }
        }

        // Compile update nodes
        def.acceleration = [0.0, 0.0, 0.0];
        def.linear_drag = 0.0;
        def.noise_frequency = 0.0;
        def.noise_amplitude = 0.0;
        def.orbit = None;
        def.velocity_limit = 0.0;
        def.kill_zones.clear();

        for node in connected_nodes("update") {
            match node.node_type {
                ParticleNodeType::Gravity => {
                    def.acceleration = node_vec3(node, "acceleration", [0.0, -9.81, 0.0]);
                }
                ParticleNodeType::LinearDrag => {
                    def.linear_drag = node_float(node, "drag", 1.0);
                }
                ParticleNodeType::RadialAccel => {
                    def.radial_acceleration = node_float(node, "acceleration", 1.0);
                }
                ParticleNodeType::TangentAccel => {
                    def.tangent_acceleration = node_float(node, "acceleration", 1.0);
                    def.tangent_accel_axis = node_vec3(node, "axis", [0.0, 1.0, 0.0]);
                }
                ParticleNodeType::Noise => {
                    def.noise_frequency = node_float(node, "frequency", 1.0);
                    def.noise_amplitude = node_float(node, "amplitude", 1.0);
                }
                ParticleNodeType::Orbit => {
                    def.orbit = Some(OrbitSettings {
                        center: node_vec3(node, "center", [0.0, 0.0, 0.0]),
                        axis: node_vec3(node, "axis", [0.0, 1.0, 0.0]),
                        speed: node_float(node, "speed", 1.0),
                        radial_pull: 0.0,
                        orbit_radius: 1.0,
                    });
                }
                ParticleNodeType::VelocityLimit => {
                    def.velocity_limit = node_float(node, "max_speed", 10.0);
                }
                ParticleNodeType::KillSphere => {
                    def.kill_zones.push(KillZone::Sphere {
                        center: node_vec3(node, "center", [0.0, 0.0, 0.0]),
                        radius: node_float(node, "radius", 5.0),
                        kill_inside: node_bool(node, "kill_inside", false),
                    });
                }
                ParticleNodeType::KillAabb => {
                    def.kill_zones.push(KillZone::Aabb {
                        center: node_vec3(node, "center", [0.0, 0.0, 0.0]),
                        half_size: node_vec3(node, "half_size", [5.0, 5.0, 5.0]),
                        kill_inside: node_bool(node, "kill_inside", false),
                    });
                }
                ParticleNodeType::ConformToSphere => {
                    def.conform_to_sphere = Some(ConformToSphere {
                        origin: node_vec3(node, "origin", [0.0, 0.0, 0.0]),
                        radius: node_float(node, "radius", 1.0),
                        influence_dist: node_float(node, "influence_dist", 3.0),
                        attraction_accel: node_float(node, "accel", 5.0),
                        max_attraction_speed: node_float(node, "max_speed", 2.0),
                        shell_half_thickness: 0.1,
                        sticky_factor: 0.5,
                    });
                }
                _ => {}
            }
        }

        // Compile render nodes
        for node in connected_nodes("render") {
            match node.node_type {
                ParticleNodeType::SizeOverLifetime => {
                    def.size_start = node_float(node, "start", 0.1);
                    def.size_end = node_float(node, "end", 0.0);
                }
                ParticleNodeType::ColorOverLifetime => {
                    let start = node_vec4(node, "start_color", [1.0, 1.0, 1.0, 1.0]);
                    let end = node_vec4(node, "end_color", [1.0, 1.0, 1.0, 0.0]);
                    def.use_flat_color = false;
                    def.color_gradient = vec![
                        GradientStop { position: 0.0, color: start },
                        GradientStop { position: 1.0, color: end },
                    ];
                }
                ParticleNodeType::Orient => {
                    def.rotation_speed = node_float(node, "rotation_speed", 0.0);
                }
                _ => {}
            }
        }

        def
    }

    pub fn get_input_value(&self, node_id: u64, pin_name: &str) -> PinValue {
        // Check if there's a connection
        if let Some(_conn) = self.connections.iter().find(|c| c.to_node == node_id && c.to_pin == pin_name) {
            // Value comes from connected output - return None to indicate "connected"
            return PinValue::None;
        }
        // Check node's local override values
        if let Some(node) = self.get_node(node_id) {
            if let Some(val) = node.values.get(pin_name) {
                return val.clone();
            }
            // Return pin template default
            let pins = node.node_type.pins();
            if let Some(pin) = pins.iter().find(|p| p.name == pin_name) {
                return pin.default_value.clone();
            }
        }
        PinValue::None
    }
}
