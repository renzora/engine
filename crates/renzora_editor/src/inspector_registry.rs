//! Data-driven inspector registry — components register fields declaratively.

use bevy::prelude::*;

/// A value that can be read from or written to a component field.
#[derive(Debug, Clone)]
pub enum FieldValue {
    Float(f32),
    Vec3([f32; 3]),
    Bool(bool),
    Color([f32; 3]),
    String(String),
    ReadOnly(String),
}

/// Metadata about a field's type, used to select the correct widget.
#[derive(Debug, Clone)]
pub enum FieldType {
    Float { speed: f32, min: f32, max: f32 },
    Vec3 { speed: f32 },
    Bool,
    Color,
    String,
    ReadOnly,
}

/// A single inspectable field on a component.
pub struct FieldDef {
    pub name: &'static str,
    pub field_type: FieldType,
    pub get_fn: fn(&World, Entity) -> Option<FieldValue>,
    pub set_fn: fn(&mut World, Entity, FieldValue),
}

/// Registration entry for one component type.
pub struct InspectorEntry {
    pub type_id: &'static str,
    pub display_name: &'static str,
    pub icon: &'static str,
    pub category: &'static str,
    pub has_fn: fn(&World, Entity) -> bool,
    pub fields: Vec<FieldDef>,
}

/// Registry holding all inspector entries, keyed by component type_id.
#[derive(Resource, Default)]
pub struct InspectorRegistry {
    entries: Vec<InspectorEntry>,
}

impl InspectorRegistry {
    /// Register an inspector entry for a component.
    pub fn register(&mut self, entry: InspectorEntry) {
        self.entries.push(entry);
    }

    /// Iterate over all registered entries.
    pub fn iter(&self) -> impl Iterator<Item = &InspectorEntry> {
        self.entries.iter()
    }
}
