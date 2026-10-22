//! The node catalog: every node type a `.vfx` graph may use, with its typed
//! pins, their defaults and its properties.
//!
//! This is data, not behaviour. What a node *computes* is its lowering in
//! [`crate::compile`]; the catalog says what it looks like, which is all the
//! compiler's type checker and the step 2 editor (pin colours, inline value
//! widgets, the add-node menu) need. Keeping the two apart is what lets the
//! editor list nodes without linking the compiler's internals.
//!
//! Pins are `Float`, `Vec2`, `Vec3` or `Color`. A [`PinKind::Generic`] pin takes
//! any of them, and a generic output's type is the unified type of the node's
//! generic inputs, which is how `math/multiply` serves floats, vectors and
//! colours with one node. A `Float` connected to a vector pin is splatted; any
//! other mismatch is a compile error naming the node.

mod catalog;

pub use catalog::{CONFORM_SHELL, CONFORM_STICKY, NODES, WIND_WOBBLE_HZ};

use crate::ir::{Attribute, ValueType};

/// A pin's type.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PinKind {
    Fixed(ValueType),
    /// Accepts any type; see the module docs.
    Generic,
}

/// What an unconnected input reads when the node has no inline value for it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PinDefault {
    /// Lanes beyond the pin's width are ignored.
    Value([f32; 4]),
    /// A particle or emitter attribute. Used by the force nodes, whose
    /// `position` input means "this particle" unless something else is wired
    /// in, so the common case needs no extra node.
    Attr(Attribute),
}

#[derive(Clone, Copy, Debug)]
pub struct PinDef {
    pub name: &'static str,
    pub label: &'static str,
    pub kind: PinKind,
    pub default: PinDefault,
}

#[derive(Clone, Copy, Debug)]
pub struct OutputDef {
    pub name: &'static str,
    pub label: &'static str,
    pub kind: PinKind,
}

/// A property: per-node data that is not a connectable value.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PropKind {
    Bool(bool),
    Curve,
    Gradient,
    /// A line of text, with its default. The name a Parameter node is set by
    /// from a script, which is a word a person types, not a value a pin can
    /// carry.
    Text(&'static str),
}

#[derive(Clone, Copy, Debug)]
pub struct PropDef {
    pub name: &'static str,
    pub label: &'static str,
    pub kind: PropKind,
}

#[derive(Clone, Copy, Debug)]
pub struct NodeDef {
    /// The `node_type` string stored in the file.
    pub node_type: &'static str,
    pub display_name: &'static str,
    /// Add-node menu group.
    pub category: &'static str,
    pub description: &'static str,
    pub inputs: &'static [PinDef],
    pub outputs: &'static [OutputDef],
    pub props: &'static [PropDef],
}

impl NodeDef {
    pub fn input(&self, name: &str) -> Option<&PinDef> {
        self.inputs.iter().find(|p| p.name == name)
    }

    pub fn output(&self, name: &str) -> Option<&OutputDef> {
        self.outputs.iter().find(|p| p.name == name)
    }

    pub fn prop(&self, name: &str) -> Option<&PropDef> {
        self.props.iter().find(|p| p.name == name)
    }

    /// A collision node: a sink that acts by being in the graph (see
    /// [`crate::ir::ColliderIr`]). Like the Particle Output, it is a root of
    /// the graph rather than something to wire out of.
    pub fn is_collider(&self) -> bool {
        self.node_type.starts_with(COLLIDER_PREFIX)
    }
}

/// The node-type prefix every collision node shares.
pub const COLLIDER_PREFIX: &str = "collide/";

/// The node a script sets by name; see [`crate::params`].
pub const PARAMETER_NODE: &str = "input/parameter";

/// The collision node that hits the scene's own colliders rather than a shape
/// of its own. CPU only; see [`crate::scene`].
pub const SCENE_COLLIDER_NODE: &str = "collide/scene";

/// The definition of `node_type`, if it is in the catalog.
pub fn node_def(node_type: &str) -> Option<&'static NodeDef> {
    NODES.iter().find(|d| d.node_type == node_type)
}

/// The node type of the graph's sink.
pub const OUTPUT_NODE: &str = "output/particle";
