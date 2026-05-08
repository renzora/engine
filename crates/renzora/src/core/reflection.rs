//! Reflection helpers for reading component fields via Bevy's reflection system.
//!
//! These live in renzora so both scripting and blueprint crates can use them
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

    let resolved_path = resolve_field_alias(component_type, field_path);
    let parts: Vec<&str> = resolved_path.split('.').collect();
    get_field_by_path(reflected, &parts)
}

/// Translate a friendly first-segment field name into the underlying
/// reflection path. Mirror of the same alias map in
/// `renzora_scripting::systems::reflection` — keeps `get` and `set` in
/// step so a script that writes `set("Text.content", ...)` can later read
/// it back with `get("Text.content")`.
///
/// Bevy's tuple-struct components (`Text(String)`, `BackgroundColor(Color)`,
/// `ZIndex(i32)`, …) only expose field "0" via reflection, but the
/// inspector — and anything a user reasonably types in a script — uses a
/// named alias like `content`, `color`, or `value`. Anything not in the
/// table passes through unchanged, so existing scripts using the raw `0`
/// index keep working and named-struct components are unaffected.
fn resolve_field_alias(component_short: &str, path: &str) -> String {
    let (head, rest) = match path.find('.') {
        Some(i) => (&path[..i], &path[i..]),
        None => (path, ""),
    };
    // Component lookup elsewhere is already case-insensitive, so match on
    // the lowercased short name to keep the alias map a single source of
    // truth regardless of whether the script wrote `text.content` or
    // `Text.content`.
    let component_lc = component_short.to_lowercase();
    let resolved_head = match (component_lc.as_str(), head) {
        ("text", "content") => "0",
        ("backgroundcolor", "color") => "0",
        ("textcolor", "color") => "0",
        ("zindex", "value" | "index") => "0",
        ("uiopacity", "value" | "opacity") => "0",
        ("uiclipcontent", "value" | "enabled" | "clip") => "0",
        _ => head,
    };
    format!("{}{}", resolved_head, rest)
}

/// Read a reflected `Vec<f32>` field from a component. Needed for reading
/// large per-vertex arrays (terrain heightmaps etc.) that don't fit the
/// single-value [`PropertyValue`] API.
pub fn get_reflected_f32_vec(
    world: &World,
    entity: Entity,
    component_type: &str,
    field_path: &str,
) -> Option<Vec<f32>> {
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
    let field = navigate_to_field(reflected, &parts)?;
    match field.reflect_ref() {
        ReflectRef::List(list) => {
            let mut out = Vec::with_capacity(list.len());
            for i in 0..list.len() {
                let item = list.get(i)?;
                let v = item.try_downcast_ref::<f32>()?;
                out.push(*v);
            }
            Some(out)
        }
        _ => None,
    }
}

fn navigate_to_field<'a>(
    reflect: &'a dyn bevy::reflect::PartialReflect,
    path: &[&str],
) -> Option<&'a dyn bevy::reflect::PartialReflect> {
    if path.is_empty() {
        return Some(reflect);
    }
    let field_name = path[0];
    let remaining = &path[1..];
    match reflect.reflect_ref() {
        ReflectRef::Struct(s) => {
            let field = s.field(field_name)?;
            if remaining.is_empty() {
                Some(field)
            } else {
                navigate_to_field(field, remaining)
            }
        }
        _ => None,
    }
}

/// Recursively navigate a reflected struct or tuple-struct by field path
/// and read the value. Numeric path parts (e.g. "0") index into tuple
/// structs — that's how `get("Text.0")` (and the aliased `Text.content`)
/// reach `Text(pub String)`.
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
        ReflectRef::TupleStruct(ts) => {
            let idx = field_name.parse::<usize>().ok()?;
            let field = ts.field(idx)?;
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
