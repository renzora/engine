//! Node-graph vocabulary shared by the blueprint, material and particle editors.
//!
//! The three graph models differ in what a node *does* and agree on everything
//! else — pins, types, connections, comment boxes — so that agreement lives
//! here, and `renzora_ember`'s reusable `node_graph_view` can render any of them.

use std::collections::HashMap;

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// Node identifier in a visual scripting graph.
pub type NodeId = u64;

/// Pin data types for blueprint nodes.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize, Reflect)]
pub enum PinType {
    /// Execution flow (white wires) — controls order of operations.
    Exec,
    Float,
    Int,
    Bool,
    String,
    Vec2,
    Vec3,
    Color,
    Entity,
    /// Wildcard — accepts any data type.
    Any,
}

impl PinType {
    /// Can `from` connect to `to`?
    pub fn compatible(from: PinType, to: PinType) -> bool {
        if from == to {
            return true;
        }
        if to == PinType::Any && from != PinType::Exec {
            return true;
        }
        if from == PinType::Any && to != PinType::Exec {
            return true;
        }
        matches!(
            (from, to),
            (PinType::Int, PinType::Float)
                | (
                    PinType::Float,
                    PinType::Vec2 | PinType::Vec3 | PinType::Color
                )
                | (PinType::Vec3, PinType::Color)
                | (PinType::Color, PinType::Vec3)
                | (PinType::Bool, PinType::Int | PinType::Float)
        )
    }
}

/// Pin direction.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize, Reflect)]
pub enum PinDir {
    Input,
    Output,
}

/// Concrete values stored on pins (inline constants, defaults).
#[derive(Clone, Debug, Serialize, Deserialize, Reflect, PartialEq, Default)]
pub enum PinValue {
    #[default]
    None,
    Float(f32),
    Int(i32),
    Bool(bool),
    String(String),
    Vec2([f32; 2]),
    Vec3([f32; 3]),
    Color([f32; 4]),
    Entity(String),
}

impl PinValue {
    pub fn as_float(&self) -> f32 {
        match self {
            Self::Float(v) => *v,
            Self::Int(v) => *v as f32,
            Self::Bool(true) => 1.0,
            Self::Bool(false) => 0.0,
            _ => 0.0,
        }
    }

    pub fn as_int(&self) -> i32 {
        match self {
            Self::Int(v) => *v,
            Self::Float(v) => *v as i32,
            Self::Bool(true) => 1,
            Self::Bool(false) => 0,
            _ => 0,
        }
    }

    pub fn as_bool(&self) -> bool {
        match self {
            Self::Bool(v) => *v,
            Self::Float(v) => *v != 0.0,
            Self::Int(v) => *v != 0,
            _ => false,
        }
    }

    pub fn as_string(&self) -> String {
        match self {
            Self::String(v) => v.clone(),
            Self::Float(v) => format!("{v}"),
            Self::Int(v) => format!("{v}"),
            Self::Bool(v) => format!("{v}"),
            Self::Entity(v) => v.clone(),
            _ => String::new(),
        }
    }

    pub fn as_vec3(&self) -> [f32; 3] {
        match self {
            Self::Vec3(v) => *v,
            Self::Color([r, g, b, _]) => [*r, *g, *b],
            Self::Float(v) => [*v, *v, *v],
            _ => [0.0, 0.0, 0.0],
        }
    }

    pub fn as_vec2(&self) -> [f32; 2] {
        match self {
            Self::Vec2(v) => *v,
            Self::Float(v) => [*v, *v],
            _ => [0.0, 0.0],
        }
    }

    pub fn as_color(&self) -> [f32; 4] {
        match self {
            Self::Color(v) => *v,
            Self::Vec3([r, g, b]) => [*r, *g, *b, 1.0],
            Self::Float(v) => [*v, *v, *v, 1.0],
            _ => [1.0, 1.0, 1.0, 1.0],
        }
    }

    pub fn pin_type(&self) -> PinType {
        match self {
            Self::None => PinType::Any,
            Self::Float(_) => PinType::Float,
            Self::Int(_) => PinType::Int,
            Self::Bool(_) => PinType::Bool,
            Self::String(_) => PinType::String,
            Self::Vec2(_) => PinType::Vec2,
            Self::Vec3(_) => PinType::Vec3,
            Self::Color(_) => PinType::Color,
            Self::Entity(_) => PinType::Entity,
        }
    }
}

/// Describes a pin on a node type (static definition).
#[derive(Clone, Debug)]
pub struct PinTemplate {
    pub name: String,
    pub label: String,
    pub pin_type: PinType,
    pub direction: PinDir,
    pub default_value: PinValue,
}

impl PinTemplate {
    pub fn exec_in(name: &str, label: &str) -> Self {
        Self {
            name: name.to_string(),
            label: label.to_string(),
            pin_type: PinType::Exec,
            direction: PinDir::Input,
            default_value: PinValue::None,
        }
    }

    pub fn exec_out(name: &str, label: &str) -> Self {
        Self {
            name: name.to_string(),
            label: label.to_string(),
            pin_type: PinType::Exec,
            direction: PinDir::Output,
            default_value: PinValue::None,
        }
    }

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

/// Static definition of a blueprint node type.
pub struct BlueprintNodeDef {
    pub node_type: &'static str,
    pub display_name: &'static str,
    pub category: &'static str,
    pub description: &'static str,
    pub pins: fn() -> Vec<PinTemplate>,
    /// RGB header color for the node in the graph editor.
    pub color: [u8; 3],
}

/// A connection between two pins in a graph.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Reflect)]
pub struct BlueprintConnection {
    pub from_node: NodeId,
    pub from_pin: String,
    pub to_node: NodeId,
    pub to_pin: String,
}

/// A node instance in a graph.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Reflect)]
pub struct BlueprintNode {
    pub id: NodeId,
    pub node_type: String,
    pub position: [f32; 2],
    /// Override values for input pins (user-set constants).
    pub input_values: HashMap<String, PinValue>,
}

impl BlueprintNode {
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

/// A resizable comment / group box drawn behind the nodes of a graph. Purely
/// visual — no pins, never compiled — but dragging it moves the nodes it
/// encloses, so it doubles as a group. Shared by every graph model (blueprint /
/// material / particle) so the editor view can render, drag, resize and persist
/// them uniformly.
#[derive(Clone, Debug, Serialize, Deserialize, Reflect, PartialEq)]
#[reflect(Default)]
pub struct GraphComment {
    pub id: u64,
    /// `[x, y, w, h]` in canvas px.
    pub rect: [f32; 4],
    pub text: String,
    /// RGB tint of the title bar / border.
    pub color: [u8; 3],
}

impl Default for GraphComment {
    fn default() -> Self {
        Self {
            id: 0,
            rect: [0.0, 0.0, 220.0, 140.0],
            text: "Comment".to_string(),
            color: [88, 110, 150],
        }
    }
}
