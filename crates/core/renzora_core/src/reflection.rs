//! Reflection helpers for reading component fields via Bevy's reflection system.
//!
//! These live in renzora_core so both scripting and blueprint crates can use them
//! without creating a dependency between each other.

use bevy::prelude::*;
use bevy::reflect::ReflectRef;

use crate::PropertyValue;

/// Read a reflected component field value from the world.
/// Returns None if the component/field doesn't exist.
pub fn get_reflected_field(
    world: &World,
    entity: Entity,
    component_type: &str,
    field_path: &str,
) -> Option<PropertyValue> {
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

    let parts: Vec<&str> = field_path.split('.').collect();
    get_field_by_path(reflected, &parts)
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
