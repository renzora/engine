//! Entity access API functions for Rhai scripts
//!
//! Provides: entity(), entity_id(), set(), parent(), child(), children()

use rhai::{Dynamic, Engine, Map};
use super::super::rhai_commands::RhaiCommand;
use super::super::entity_data_store;
use crate::component_system::PropertyValue;

/// Register entity access functions
pub fn register(engine: &mut Engine) {
    // entity(name) - Look up an entity by name, returns a Map of all its properties
    engine.register_fn("entity", |name: rhai::ImmutableString| -> Map {
        entity_data_store::get_entity_by_name(name.as_str())
            .unwrap_or_else(Map::new)
    });

    // entity_id(id) - Look up an entity by raw ID
    engine.register_fn("entity_id", |id: i64| -> Map {
        entity_data_store::get_entity_map(id as u64)
            .unwrap_or_else(Map::new)
    });

    // set(entity_map, property, value) - Deferred write to an entity's property
    // Float overload
    engine.register_fn("set", |entity_map: Map, prop: rhai::ImmutableString, value: f64| {
        if let Some(id_dyn) = entity_map.get("_id") {
            if let Some(id) = id_dyn.clone().try_cast::<i64>() {
                super::push_command(RhaiCommand::SetProperty {
                    entity_id: id as u64,
                    property: prop.to_string(),
                    value: PropertyValue::Float(value as f32),
                });
            }
        }
    });

    // set(entity_map, property, value) - Int overload
    engine.register_fn("set", |entity_map: Map, prop: rhai::ImmutableString, value: i64| {
        if let Some(id_dyn) = entity_map.get("_id") {
            if let Some(id) = id_dyn.clone().try_cast::<i64>() {
                super::push_command(RhaiCommand::SetProperty {
                    entity_id: id as u64,
                    property: prop.to_string(),
                    value: PropertyValue::Int(value as i32),
                });
            }
        }
    });

    // set(entity_map, property, value) - Bool overload
    engine.register_fn("set", |entity_map: Map, prop: rhai::ImmutableString, value: bool| {
        if let Some(id_dyn) = entity_map.get("_id") {
            if let Some(id) = id_dyn.clone().try_cast::<i64>() {
                super::push_command(RhaiCommand::SetProperty {
                    entity_id: id as u64,
                    property: prop.to_string(),
                    value: PropertyValue::Bool(value),
                });
            }
        }
    });

    // set(entity_map, property, value) - String overload
    engine.register_fn("set", |entity_map: Map, prop: rhai::ImmutableString, value: rhai::ImmutableString| {
        if let Some(id_dyn) = entity_map.get("_id") {
            if let Some(id) = id_dyn.clone().try_cast::<i64>() {
                super::push_command(RhaiCommand::SetProperty {
                    entity_id: id as u64,
                    property: prop.to_string(),
                    value: PropertyValue::String(value.to_string()),
                });
            }
        }
    });

    // set(entity_map, property, value) - Dynamic overload (catch-all)
    engine.register_fn("set", |entity_map: Map, prop: rhai::ImmutableString, value: Dynamic| {
        if let Some(id_dyn) = entity_map.get("_id") {
            if let Some(id) = id_dyn.clone().try_cast::<i64>() {
                let pv = dynamic_to_property_value(&value);
                if let Some(pv) = pv {
                    super::push_command(RhaiCommand::SetProperty {
                        entity_id: id as u64,
                        property: prop.to_string(),
                        value: pv,
                    });
                }
            }
        }
    });

    // parent() - Get the parent entity Map
    engine.register_fn("parent", || -> Map {
        entity_data_store::get_parent_map()
            .unwrap_or_else(Map::new)
    });

    // child(name) - Get a named child entity Map
    engine.register_fn("child", |name: rhai::ImmutableString| -> Map {
        entity_data_store::get_child_by_name(name.as_str())
            .unwrap_or_else(Map::new)
    });

    // children() - Get all children as an Array of Maps
    engine.register_fn("children", || -> rhai::Array {
        entity_data_store::get_children_maps()
            .into_iter()
            .map(Dynamic::from)
            .collect()
    });

    // get(entity_map, property) - Read a property from an entity map (supports "component:field" syntax)
    engine.register_fn("get", |entity_map: Map, prop: rhai::ImmutableString| -> Dynamic {
        entity_map.get(prop.as_str())
            .cloned()
            .unwrap_or(Dynamic::UNIT)
    });
}

/// Convert a Rhai Dynamic to PropertyValue
fn dynamic_to_property_value(value: &Dynamic) -> Option<PropertyValue> {
    if let Some(v) = value.clone().try_cast::<f64>() {
        return Some(PropertyValue::Float(v as f32));
    }
    if let Some(v) = value.clone().try_cast::<i64>() {
        return Some(PropertyValue::Int(v as i32));
    }
    if let Some(v) = value.clone().try_cast::<bool>() {
        return Some(PropertyValue::Bool(v));
    }
    if let Some(v) = value.clone().into_immutable_string().ok() {
        return Some(PropertyValue::String(v.to_string()));
    }
    None
}
