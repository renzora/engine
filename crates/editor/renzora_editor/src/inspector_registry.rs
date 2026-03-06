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
    /// Optional function to add this component to an entity (for "Add Component" overlay).
    /// If `None`, the component won't appear in the Add Component overlay.
    pub add_fn: Option<fn(&mut World, Entity)>,
    /// Optional function to remove this component from an entity (trash button).
    /// If `None`, the component section won't show toggle/remove controls.
    pub remove_fn: Option<fn(&mut World, Entity)>,
    /// Check if the component is enabled (for toggle switch display).
    pub is_enabled_fn: Option<fn(&World, Entity) -> bool>,
    /// Set the component's enabled state (called on toggle switch click).
    pub set_enabled_fn: Option<fn(&mut World, Entity, bool)>,
    pub fields: Vec<FieldDef>,
    /// Optional custom UI function. When set, the inspector calls this instead
    /// of rendering the declarative `fields` list.
    pub custom_ui_fn: Option<fn(&mut bevy_egui::egui::Ui, &World, Entity, &crate::EditorCommands, &renzora_theme::Theme)>,
}

/// Registry holding all inspector entries, keyed by component type_id.
#[derive(Resource, Default)]
pub struct InspectorRegistry {
    entries: Vec<InspectorEntry>,
}

impl InspectorRegistry {
    /// Register an inspector entry for a component.
    ///
    /// The "name" component is always kept at the front; all others are appended.
    pub fn register(&mut self, entry: InspectorEntry) {
        match entry.type_id {
            "name" => self.entries.insert(0, entry),
            "transform" => {
                // Insert after "name" if present, otherwise at the front.
                let pos = self.entries.iter()
                    .position(|e| e.type_id != "name")
                    .unwrap_or(self.entries.len());
                self.entries.insert(pos, entry);
            }
            _ => self.entries.push(entry),
        }
    }

    /// Iterate over all registered entries.
    pub fn iter(&self) -> impl Iterator<Item = &InspectorEntry> {
        self.entries.iter()
    }
}
