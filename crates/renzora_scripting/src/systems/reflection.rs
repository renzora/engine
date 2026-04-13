#![allow(unused_mut, dead_code, unused_variables)]

//! Exclusive system that applies generic reflection-based component field writes.

use bevy::prelude::*;
use bevy::reflect::ReflectRef;

use super::execution::{ScriptReflectionQueue};
use crate::command::PropertyValue;

/// Exclusive system that drains the ScriptReflectionQueue and applies
/// each set operation using Bevy's reflection system.
pub fn apply_reflection_sets(world: &mut World) {
    // Drain the queue
    let sets = {
        let Some(mut queue) = world.get_resource_mut::<ScriptReflectionQueue>() else {
            return;
        };
        std::mem::take(&mut queue.sets)
    };

    if sets.is_empty() {
        return;
    }

    // Build name → entity map for entity_name lookups
    let mut name_map: Option<std::collections::HashMap<String, Entity>> = None;

    let type_registry = world.resource::<AppTypeRegistry>().clone();

    for set_op in &sets {
        // Resolve target entity
        let target = if let Some(name) = &set_op.entity_name {
            // Lazy-build name map
            if name_map.is_none() {
                let mut map = std::collections::HashMap::new();
                let mut query = world.query::<(Entity, &Name)>();
                for (e, n) in query.iter(world) {
                    map.insert(n.as_str().to_string(), e);
                }
                name_map = Some(map);
            }
            match name_map.as_ref().unwrap().get(name) {
                Some(&e) => e,
                None => {
                    warn!("[Script] set: entity '{}' not found", name);
                    continue;
                }
            }
        } else if let Some(id) = set_op.entity_id {
            Entity::from_bits(id)
        } else {
            set_op.source_entity
        };

        // Find the component type registration by short name (case-insensitive)
        let registry = type_registry.read();
        let query = set_op.component_type.to_lowercase();
        let registration = registry.iter().find(|reg| {
            let path = reg.type_info().type_path();
            let short = path.rsplit("::").next().unwrap_or(path);
            short.to_lowercase() == query
        });

        let Some(registration) = registration else {
            warn!(
                "[Script] set: component type '{}' not found in registry",
                set_op.component_type
            );
            continue;
        };

        let Some(reflect_component) = registration.data::<ReflectComponent>() else {
            warn!(
                "[Script] set: '{}' has no ReflectComponent data",
                set_op.component_type
            );
            continue;
        };

        // Get the current component value via reflection, clone it, modify, apply back
        let entity_ref = world.entity(target);
        let Some(reflected) = reflect_component.reflect(entity_ref) else {
            warn!(
                "[Script] set: entity {:?} has no '{}' component",
                target, set_op.component_type
            );
            continue;
        };

        let Ok(mut cloned) = reflected.reflect_clone() else {
            warn!(
                "[Script] set: failed to clone '{}' component",
                set_op.component_type
            );
            continue;
        };

        // Navigate the field path and set the value
        let parts: Vec<&str> = set_op.field_path.split('.').collect();
        if set_field_by_path(cloned.as_mut(), &parts, &set_op.value) {
            // Apply the modified component back
            let mut entity_mut = world.entity_mut(target);
            reflect_component.apply(&mut entity_mut, cloned.as_partial_reflect());
        } else {
            warn!(
                "[Script] set: failed to set field '{}.{}' on '{}'",
                set_op.component_type, set_op.field_path,
                set_op.entity_name.as_deref().unwrap_or("self")
            );
        }
    }
}

/// Recursively navigate a reflected struct by field path and set the final value.
fn set_field_by_path(
    reflect: &mut dyn bevy::reflect::PartialReflect,
    path: &[&str],
    value: &PropertyValue,
) -> bool {
    if path.is_empty() {
        return false;
    }

    let field_name = path[0];
    let remaining = &path[1..];

    // Try struct field access
    match reflect.reflect_mut() {
        bevy::reflect::ReflectMut::Struct(s) => {
            if remaining.is_empty() {
                // Set the leaf field
                if let Some(field) = s.field_mut(field_name) {
                    return apply_value_to_reflect(field, value);
                }
                false
            } else {
                // Navigate deeper
                if let Some(field) = s.field_mut(field_name) {
                    return set_field_by_path(field, remaining, value);
                }
                false
            }
        }
        _ => false,
    }
}

/// Apply a PropertyValue to a reflected field.
fn apply_value_to_reflect(
    field: &mut dyn bevy::reflect::PartialReflect,
    value: &PropertyValue,
) -> bool {
    match value {
        PropertyValue::Float(v) => {
            // Try f32 first, then f64
            if let Some(current) = field.try_downcast_mut::<f32>() {
                *current = *v;
                return true;
            }
            if let Some(current) = field.try_downcast_mut::<f64>() {
                *current = *v as f64;
                return true;
            }
            false
        }
        PropertyValue::Int(v) => {
            if let Some(current) = field.try_downcast_mut::<i32>() {
                *current = *v as i32;
                return true;
            }
            if let Some(current) = field.try_downcast_mut::<i64>() {
                *current = *v;
                return true;
            }
            if let Some(current) = field.try_downcast_mut::<u32>() {
                *current = *v as u32;
                return true;
            }
            if let Some(current) = field.try_downcast_mut::<usize>() {
                *current = *v as usize;
                return true;
            }
            if let Some(current) = field.try_downcast_mut::<f32>() {
                *current = *v as f32;
                return true;
            }
            false
        }
        PropertyValue::Bool(v) => {
            if let Some(current) = field.try_downcast_mut::<bool>() {
                *current = *v;
                return true;
            }
            false
        }
        PropertyValue::String(v) => {
            if let Some(current) = field.try_downcast_mut::<String>() {
                *current = v.clone();
                return true;
            }
            false
        }
        PropertyValue::Vec3(v) => {
            if let Some(current) = field.try_downcast_mut::<Vec3>() {
                *current = Vec3::new(v[0], v[1], v[2]);
                return true;
            }
            false
        }
        PropertyValue::Color(v) => {
            if let Some(current) = field.try_downcast_mut::<Color>() {
                *current = Color::srgba(v[0], v[1], v[2], v[3]);
                return true;
            }
            if let Some(current) = field.try_downcast_mut::<Vec4>() {
                *current = Vec4::new(v[0], v[1], v[2], v[3]);
                return true;
            }
            false
        }
    }
}

/// Read a reflected component field value from the world.
/// Returns None if the component/field doesn't exist.
///
/// Delegates to `renzora::reflection::get_reflected_field`.
pub fn get_reflected_field(
    world: &World,
    entity: Entity,
    component_type: &str,
    field_path: &str,
) -> Option<PropertyValue> {
    renzora::reflection::get_reflected_field(world, entity, component_type, field_path)
}

/// Read ALL fields of a reflected component, returning a flat HashMap.
/// Nested structs are flattened with dot notation (e.g. "color.x").
pub fn get_all_component_fields(
    world: &World,
    entity: Entity,
    component_type: &str,
) -> Option<std::collections::HashMap<String, PropertyValue>> {
    let type_registry = world.resource::<AppTypeRegistry>().clone();
    let registry = type_registry.read();

    let query = component_type.to_lowercase();
    let registration = registry.iter().find(|reg| {
        let path = reg.type_info().type_path();
        let short = path.rsplit("::").next().unwrap_or(path);
        short.to_lowercase() == query
    })?;

    let reflect_component = registration.data::<ReflectComponent>()?;
    let entity_ref = world.entity(entity);
    let reflected = reflect_component.reflect(entity_ref)?;

    let mut fields = std::collections::HashMap::new();
    collect_struct_fields(reflected, "", &mut fields);
    Some(fields)
}

fn collect_struct_fields(
    reflect: &dyn bevy::reflect::PartialReflect,
    prefix: &str,
    out: &mut std::collections::HashMap<String, PropertyValue>,
) {
    match reflect.reflect_ref() {
        ReflectRef::Struct(s) => {
            for i in 0..s.field_len() {
                let name = s.name_at(i).unwrap_or("?");
                let full_name = if prefix.is_empty() {
                    name.to_string()
                } else {
                    format!("{}.{}", prefix, name)
                };
                let field = s.field_at(i).unwrap();
                // Try reading as primitive first
                if let Some(val) = read_value_from_reflect(field) {
                    out.insert(full_name, val);
                } else {
                    // Try recursing into nested struct
                    collect_struct_fields(field, &full_name, out);
                }
            }
        }
        _ => {
            // Try to read the value directly (for non-struct reflected types)
            if let Some(val) = read_value_from_reflect(reflect) {
                if !prefix.is_empty() {
                    out.insert(prefix.to_string(), val);
                }
            }
        }
    }
}

/// Get the names of all reflected components on an entity.
pub fn get_entity_component_names(
    world: &World,
    entity: Entity,
) -> Vec<String> {
    let type_registry = world.resource::<AppTypeRegistry>().clone();
    let registry = type_registry.read();
    let mut names = Vec::new();

    let entity_ref = world.entity(entity);
    let archetype = entity_ref.archetype();

    for &component_id in archetype.components() {
        let Some(info) = world.components().get_info(component_id) else { continue };
        let type_id = match info.type_id() {
            Some(id) => id,
            None => continue,
        };
        // Only include components that are registered in the type registry with ReflectComponent
        if let Some(registration) = registry.get(type_id) {
            if registration.data::<ReflectComponent>().is_some() {
                let path = registration.type_info().type_path();
                let short = path.rsplit("::").next().unwrap_or(path);
                names.push(short.to_string());
            }
        }
    }

    names.sort();
    names
}

/// Recursively navigate a reflected struct by field path and read the value.
fn get_field_by_path(
    reflect: &dyn bevy::reflect::PartialReflect,
    path: &[&str],
) -> Option<PropertyValue> {
    if path.is_empty() {
        return read_value_from_reflect(reflect);
    }

    let field_name = path[0];
    let remaining = &path[1..];

    match reflect.reflect_ref() {
        ReflectRef::Struct(s) => {
            let field = s.field(field_name)?;
            if remaining.is_empty() {
                read_value_from_reflect(field)
            } else {
                get_field_by_path(field, remaining)
            }
        }
        _ => None,
    }
}

/// Read a primitive value from a reflected field.
fn read_value_from_reflect(field: &dyn bevy::reflect::PartialReflect) -> Option<PropertyValue> {
    if let Some(v) = field.try_downcast_ref::<f32>() {
        return Some(PropertyValue::Float(*v));
    }
    if let Some(v) = field.try_downcast_ref::<f64>() {
        return Some(PropertyValue::Float(*v as f32));
    }
    if let Some(v) = field.try_downcast_ref::<i32>() {
        return Some(PropertyValue::Int(*v as i64));
    }
    if let Some(v) = field.try_downcast_ref::<i64>() {
        return Some(PropertyValue::Int(*v));
    }
    if let Some(v) = field.try_downcast_ref::<u32>() {
        return Some(PropertyValue::Int(*v as i64));
    }
    if let Some(v) = field.try_downcast_ref::<usize>() {
        return Some(PropertyValue::Int(*v as i64));
    }
    if let Some(v) = field.try_downcast_ref::<bool>() {
        return Some(PropertyValue::Bool(*v));
    }
    if let Some(v) = field.try_downcast_ref::<String>() {
        return Some(PropertyValue::String(v.clone()));
    }
    if let Some(v) = field.try_downcast_ref::<Vec3>() {
        return Some(PropertyValue::Vec3([v.x, v.y, v.z]));
    }
    if let Some(v) = field.try_downcast_ref::<Vec4>() {
        return Some(PropertyValue::Color([v.x, v.y, v.z, v.w]));
    }
    if let Some(v) = field.try_downcast_ref::<Color>() {
        let c = v.to_srgba();
        return Some(PropertyValue::Color([c.red, c.green, c.blue, c.alpha]));
    }
    None
}
