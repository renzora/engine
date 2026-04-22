//! Convenience constructors for the most common bindings.
//!
//! Every panel will wire up entity-name labels, transform fields, and
//! selection-tracked inspectors. Rather than spell out the same 6-line
//! `BindSource::EntityField { entity, getter: |world, e| { … } }` in
//! every call site, helpers here give you one-liners. They're purely
//! sugar — the raw enum constructors are always available if you need
//! custom behaviour.

use bevy::prelude::*;

use crate::source::{BindSink, BindSource};
use crate::value::BoundValue;

// ─── Name ────────────────────────────────────────────────────────────

fn read_name(world: &World, entity: Entity) -> BoundValue {
    world
        .get::<Name>(entity)
        .map(|n| BoundValue::String(n.as_str().to_string()))
        .unwrap_or(BoundValue::Unit)
}

fn write_name(world: &mut World, entity: Entity, value: BoundValue) {
    if let BoundValue::String(s) = value {
        if let Ok(mut entity_ref) = world.get_entity_mut(entity) {
            entity_ref.insert(Name::new(s));
        }
    }
}

impl BindSource {
    /// Read the `Name` component of a specific entity.
    pub fn entity_name(entity: Entity) -> Self {
        BindSource::EntityField { entity, getter: read_name }
    }

    /// Read the `Name` of whichever entity is the current primary selection.
    /// Widgets bound via this variant survive selection changes — no
    /// rebinding needed.
    pub fn selected_name() -> Self {
        BindSource::SelectedField { getter: read_name }
    }
}

impl BindSink {
    pub fn entity_name(entity: Entity) -> Self {
        BindSink::EntityField { entity, setter: write_name }
    }

    pub fn selected_name() -> Self {
        BindSink::SelectedField { setter: write_name }
    }
}

// ─── Transform ───────────────────────────────────────────────────────

fn read_translation(world: &World, entity: Entity) -> BoundValue {
    world
        .get::<Transform>(entity)
        .map(|t| BoundValue::Vec3([t.translation.x, t.translation.y, t.translation.z]))
        .unwrap_or(BoundValue::Unit)
}

fn write_translation(world: &mut World, entity: Entity, value: BoundValue) {
    if let BoundValue::Vec3([x, y, z]) = value {
        if let Some(mut t) = world.get_mut::<Transform>(entity) {
            t.translation = Vec3::new(x, y, z);
        }
    }
}

impl BindSource {
    pub fn entity_translation(entity: Entity) -> Self {
        BindSource::EntityField { entity, getter: read_translation }
    }

    pub fn selected_translation() -> Self {
        BindSource::SelectedField { getter: read_translation }
    }
}

impl BindSink {
    pub fn entity_translation(entity: Entity) -> Self {
        BindSink::EntityField { entity, setter: write_translation }
    }

    pub fn selected_translation() -> Self {
        BindSink::SelectedField { setter: write_translation }
    }
}

// ─── Visibility ──────────────────────────────────────────────────────

fn read_visibility(world: &World, entity: Entity) -> BoundValue {
    world
        .get::<Visibility>(entity)
        .map(|v| match v {
            Visibility::Hidden => BoundValue::Bool(false),
            Visibility::Visible | Visibility::Inherited => BoundValue::Bool(true),
        })
        .unwrap_or(BoundValue::Unit)
}

fn write_visibility(world: &mut World, entity: Entity, value: BoundValue) {
    if let BoundValue::Bool(on) = value {
        if let Some(mut v) = world.get_mut::<Visibility>(entity) {
            *v = if on { Visibility::Inherited } else { Visibility::Hidden };
        }
    }
}

impl BindSource {
    pub fn entity_visibility(entity: Entity) -> Self {
        BindSource::EntityField { entity, getter: read_visibility }
    }
}

impl BindSink {
    pub fn entity_visibility(entity: Entity) -> Self {
        BindSink::EntityField { entity, setter: write_visibility }
    }
}
