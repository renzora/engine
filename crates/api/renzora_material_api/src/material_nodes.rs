//! The material node catalogue — what node types exist, described as data.
//!
//! `renzora_shader` owns the node *definitions*: their pins, their categories,
//! and the WGSL each one emits. That crate is not on the short list a native
//! plugin may link (`bevy`, `renzora`, `renzora_ember`), and none of it can move
//! here, because a definition is inseparable from the codegen that reads it.
//!
//! What can move is the description. This resource is a flat, data-only copy of
//! the catalogue, filled by `renzora_shader` at startup, so anything that can
//! reach the contract crate can answer "what nodes are there and what pins does
//! this one have" without linking the shader crate or duplicating a list that
//! would then drift.
//!
//! It is deliberately not the definitions themselves. A `PinTemplate` carries a
//! `PinValue`, a `PinValue` is the shader compiler's own vocabulary, and pulling
//! that chain into the contract crate would end with the compiler in it. Strings
//! are enough for the question being asked: an agent or a plugin choosing a node
//! needs its name, its pins and their directions, not the type that will be
//! emitted for them.

use bevy::prelude::*;

/// One pin on a node, as data.
#[derive(Clone, Debug)]
pub struct MaterialNodePin {
    /// The name a connection refers to (`uv`, `base_color`).
    pub name: String,
    /// What the graph editor shows.
    pub label: String,
    /// `Float`, `Vec3`, `Color`, `Texture2D`, …, as the pin type's name.
    pub pin_type: String,
    /// Inputs take a wire or a constant; outputs are wired from.
    pub input: bool,
}

/// One node type, as data.
#[derive(Clone, Debug)]
pub struct MaterialNodeDefinition {
    /// The `category/name` string a `.material` file stores, e.g.
    /// `procedural/noise_fbm`. This is the identifier; everything else here is
    /// description.
    pub node_type: String,
    pub display_name: String,
    pub category: String,
    pub description: String,
    pub pins: Vec<MaterialNodePin>,
}

impl MaterialNodeDefinition {
    /// The pins a caller may wire *into*.
    pub fn inputs(&self) -> impl Iterator<Item = &MaterialNodePin> {
        self.pins.iter().filter(|pin| pin.input)
    }

    /// The pins a caller may wire *from*.
    pub fn outputs(&self) -> impl Iterator<Item = &MaterialNodePin> {
        self.pins.iter().filter(|pin| !pin.input)
    }
}

/// Every material node type the running editor knows about.
///
/// Empty in a build with no shader crate, which is how a consumer tells "no
/// nodes" from "not published": an empty catalogue means nobody filled it, and
/// there is no build in which the real answer is zero.
#[derive(Resource, Default)]
pub struct MaterialNodeCatalog {
    entries: Vec<MaterialNodeDefinition>,
}

impl MaterialNodeCatalog {
    /// Replace the catalogue. Called once, by the crate that owns the
    /// definitions.
    pub fn set(&mut self, entries: Vec<MaterialNodeDefinition>) {
        self.entries = entries;
    }

    pub fn iter(&self) -> impl Iterator<Item = &MaterialNodeDefinition> {
        self.entries.iter()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// One node type by its `category/name` id.
    pub fn get(&self, node_type: &str) -> Option<&MaterialNodeDefinition> {
        self.entries
            .iter()
            .find(|entry| entry.node_type == node_type)
    }

    /// The distinct categories, in the order they first appear.
    pub fn categories(&self) -> Vec<&str> {
        let mut out: Vec<&str> = Vec::new();
        for entry in &self.entries {
            if !out.contains(&entry.category.as_str()) {
                out.push(&entry.category);
            }
        }
        out
    }
}
