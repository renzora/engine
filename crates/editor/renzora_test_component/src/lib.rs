//! Test component extension — demonstrates how to define custom components
//! and register them with the inspector from an extension crate.
//!
//! Defines three game components (`Health`, `Movement`, `EntityTag`), spawns test
//! entities, and registers inspector entries so their fields are editable.
//!
//! **Note:** Extensions must use `PostStartup` (not `Startup`) for entity spawning
//! to avoid scheduling conflicts with the editor/egui plugins.

use bevy::prelude::*;
use egui_phosphor::regular;
use renzora_editor::{FieldDef, FieldType, FieldValue, InspectorEntry, InspectorRegistry};

// ── Custom components ──────────────────────────────────────────────────────

/// Health component with current/max HP and a shield flag.
#[derive(Component)]
pub struct Health {
    pub current: f32,
    pub max: f32,
    pub has_shield: bool,
}

/// Movement component with speed, jump height, and a grounded flag.
#[derive(Component)]
pub struct Movement {
    pub speed: f32,
    pub jump_height: f32,
    pub is_grounded: bool,
}

/// Tag component with an entity label string.
#[derive(Component)]
pub struct EntityTag {
    pub tag: String,
}

// ── Inspector registrations ────────────────────────────────────────────────

fn health_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "health",
        display_name: "Health",
        icon: regular::HEART,
        category: "gameplay",
        has_fn: |world, entity| world.get::<Health>(entity).is_some(),
        add_fn: None,
        remove_fn: None,
        is_enabled_fn: None,
        set_enabled_fn: None,
        fields: vec![
            FieldDef {
                name: "Current",
                field_type: FieldType::Float {
                    speed: 1.0,
                    min: 0.0,
                    max: 10_000.0,
                },
                get_fn: |world, entity| {
                    world
                        .get::<Health>(entity)
                        .map(|h| FieldValue::Float(h.current))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Float(v) = val {
                        if let Some(mut h) = world.get_mut::<Health>(entity) {
                            h.current = v;
                        }
                    }
                },
            },
            FieldDef {
                name: "Max",
                field_type: FieldType::Float {
                    speed: 1.0,
                    min: 1.0,
                    max: 10_000.0,
                },
                get_fn: |world, entity| {
                    world
                        .get::<Health>(entity)
                        .map(|h| FieldValue::Float(h.max))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Float(v) = val {
                        if let Some(mut h) = world.get_mut::<Health>(entity) {
                            h.max = v;
                        }
                    }
                },
            },
            FieldDef {
                name: "Shield",
                field_type: FieldType::Bool,
                get_fn: |world, entity| {
                    world
                        .get::<Health>(entity)
                        .map(|h| FieldValue::Bool(h.has_shield))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Bool(v) = val {
                        if let Some(mut h) = world.get_mut::<Health>(entity) {
                            h.has_shield = v;
                        }
                    }
                },
            },
        ],
        custom_ui_fn: None,
    }
}

fn movement_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "movement",
        display_name: "Movement",
        icon: regular::SNEAKER_MOVE,
        category: "gameplay",
        has_fn: |world, entity| world.get::<Movement>(entity).is_some(),
        add_fn: None,
        remove_fn: None,
        is_enabled_fn: None,
        set_enabled_fn: None,
        fields: vec![
            FieldDef {
                name: "Speed",
                field_type: FieldType::Float {
                    speed: 0.1,
                    min: 0.0,
                    max: 100.0,
                },
                get_fn: |world, entity| {
                    world
                        .get::<Movement>(entity)
                        .map(|m| FieldValue::Float(m.speed))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Float(v) = val {
                        if let Some(mut m) = world.get_mut::<Movement>(entity) {
                            m.speed = v;
                        }
                    }
                },
            },
            FieldDef {
                name: "Jump Height",
                field_type: FieldType::Float {
                    speed: 0.05,
                    min: 0.0,
                    max: 50.0,
                },
                get_fn: |world, entity| {
                    world
                        .get::<Movement>(entity)
                        .map(|m| FieldValue::Float(m.jump_height))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Float(v) = val {
                        if let Some(mut m) = world.get_mut::<Movement>(entity) {
                            m.jump_height = v;
                        }
                    }
                },
            },
            FieldDef {
                name: "Grounded",
                field_type: FieldType::Bool,
                get_fn: |world, entity| {
                    world
                        .get::<Movement>(entity)
                        .map(|m| FieldValue::Bool(m.is_grounded))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Bool(v) = val {
                        if let Some(mut m) = world.get_mut::<Movement>(entity) {
                            m.is_grounded = v;
                        }
                    }
                },
            },
        ],
        custom_ui_fn: None,
    }
}

fn entity_tag_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "entity_tag",
        display_name: "Tag",
        icon: regular::TAG,
        category: "gameplay",
        has_fn: |world, entity| world.get::<EntityTag>(entity).is_some(),
        add_fn: None,
        remove_fn: None,
        is_enabled_fn: None,
        set_enabled_fn: None,
        fields: vec![FieldDef {
            name: "Tag",
            field_type: FieldType::String,
            get_fn: |world, entity| {
                world
                    .get::<EntityTag>(entity)
                    .map(|t| FieldValue::String(t.tag.clone()))
            },
            set_fn: |world, entity, val| {
                if let FieldValue::String(v) = val {
                    if let Some(mut t) = world.get_mut::<EntityTag>(entity) {
                        t.tag = v;
                    }
                }
            },
        }],
        custom_ui_fn: None,
    }
}

// ── Plugin ─────────────────────────────────────────────────────────────────

/// Test component plugin — registers custom components with the inspector
/// and spawns test entities to demonstrate the system.
pub struct TestComponentPlugin;

impl Plugin for TestComponentPlugin {
    fn build(&self, app: &mut App) {
        // Ensure InspectorRegistry exists, then register entries
        app.init_resource::<InspectorRegistry>();
        let world = app.world_mut();
        if let Some(mut registry) = world.get_resource_mut::<InspectorRegistry>() {
            registry.register(health_entry());
            registry.register(movement_entry());
            registry.register(entity_tag_entry());
        }

    }
}
