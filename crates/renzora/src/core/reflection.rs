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

// ============================================================================
// Reflected SET (write a component field by string path)
// ============================================================================

/// Write a reflected component field value into the world. Mirrors
/// [`get_reflected_field`]: case-insensitive component short-name lookup,
/// friendly field-name aliases, dotted/tuple-index path navigation. Returns
/// `false` if the component/field is missing or the value kind doesn't fit.
pub fn set_reflected_field(
    world: &mut World,
    entity: Entity,
    component_type: &str,
    field_path: &str,
    value: &PropertyValue,
) -> bool {
    let type_registry = world.resource::<AppTypeRegistry>().clone();
    let registry = type_registry.read();

    let query = component_type.to_lowercase();
    let Some(registration) = registry.iter().find(|reg| {
        let path = reg.type_info().type_path();
        let short = path.rsplit("::").next().unwrap_or(path);
        short.to_lowercase() == query
    }) else {
        return false;
    };

    let Some(reflect_component) = registration.data::<ReflectComponent>() else {
        return false;
    };

    let entity_ref = world.entity(entity);
    let Some(reflected) = reflect_component.reflect(entity_ref) else {
        return false;
    };
    let Ok(mut cloned) = reflected.reflect_clone() else {
        return false;
    };

    let resolved_path = resolve_field_alias(component_type, field_path);
    let parts: Vec<&str> = resolved_path.split('.').collect();
    if set_field_by_path(cloned.as_mut(), &parts, value) {
        let mut entity_mut = world.entity_mut(entity);
        reflect_component.apply(&mut entity_mut, cloned.as_partial_reflect());
        true
    } else {
        false
    }
}

/// Reflect-clone a whole component off `entity` by its (short, case-insensitive)
/// type name — a value snapshot for undo. Returns `None` if the type isn't
/// registered/reflectable or the component isn't present. Pair with
/// [`insert_component_reflected`] to restore it.
pub fn capture_component(
    world: &World,
    entity: Entity,
    component_type: &str,
) -> Option<Box<dyn bevy::reflect::Reflect>> {
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
    reflected.reflect_clone().ok()
}

/// Insert a previously [`capture_component`]-ed value back onto `entity`,
/// recreating the component (used to undo a component removal). Returns whether
/// it succeeded.
pub fn insert_component_reflected(
    world: &mut World,
    entity: Entity,
    component_type: &str,
    value: &dyn bevy::reflect::Reflect,
) -> bool {
    let type_registry = world.resource::<AppTypeRegistry>().clone();
    let registry = type_registry.read();
    let query = component_type.to_lowercase();
    let Some(registration) = registry.iter().find(|reg| {
        let path = reg.type_info().type_path();
        let short = path.rsplit("::").next().unwrap_or(path);
        short.to_lowercase() == query
    }) else {
        return false;
    };
    let Some(reflect_component) = registration.data::<ReflectComponent>() else {
        return false;
    };
    let Ok(mut entity_mut) = world.get_entity_mut(entity) else {
        return false;
    };
    reflect_component.insert(&mut entity_mut, value.as_partial_reflect(), &registry);
    true
}

/// Recursively navigate a reflected struct or tuple-struct by field path and set
/// the final value. Numeric path parts (e.g. "0") index into tuple structs.
pub fn set_field_by_path(
    reflect: &mut dyn bevy::reflect::PartialReflect,
    path: &[&str],
    value: &PropertyValue,
) -> bool {
    if path.is_empty() {
        return false;
    }

    let field_name = path[0];
    let remaining = &path[1..];

    match reflect.reflect_mut() {
        bevy::reflect::ReflectMut::Struct(s) => {
            let Some(field) = s.field_mut(field_name) else {
                return false;
            };
            if remaining.is_empty() {
                apply_value_to_reflect(field, value)
            } else {
                set_field_by_path(field, remaining, value)
            }
        }
        bevy::reflect::ReflectMut::TupleStruct(ts) => {
            let Ok(idx) = field_name.parse::<usize>() else {
                return false;
            };
            let Some(field) = ts.field_mut(idx) else {
                return false;
            };
            if remaining.is_empty() {
                apply_value_to_reflect(field, value)
            } else {
                set_field_by_path(field, remaining, value)
            }
        }
        _ => false,
    }
}

/// Apply a [`PropertyValue`] onto a reflected field, coercing numeric types.
pub fn apply_value_to_reflect(
    field: &mut dyn bevy::reflect::PartialReflect,
    value: &PropertyValue,
) -> bool {
    match value {
        PropertyValue::Float(v) => {
            if let Some(current) = field.try_downcast_mut::<f32>() {
                *current = *v;
                return true;
            }
            if let Some(current) = field.try_downcast_mut::<f64>() {
                *current = *v as f64;
                return true;
            }
            // Integer fields accept floats by rounding. Property-animation
            // tracks only carry `TrackValue::Float` — an int field read as a
            // key widens to float (`TrackValue::from_property_value`), so the
            // sampled value must narrow back here or animating any integer
            // field (e.g. `SpriteSheet.frame`) would silently write nothing.
            if let Some(current) = field.try_downcast_mut::<i32>() {
                *current = v.round() as i32;
                return true;
            }
            if let Some(current) = field.try_downcast_mut::<i64>() {
                *current = v.round() as i64;
                return true;
            }
            if let Some(current) = field.try_downcast_mut::<u32>() {
                *current = v.round().max(0.0) as u32;
                return true;
            }
            if let Some(current) = field.try_downcast_mut::<usize>() {
                *current = v.round().max(0.0) as usize;
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

// ============================================================================
// Component / field enumeration (for inspectors, scripting, the anim picker)
// ============================================================================

/// Read ALL fields of a reflected component, returning a flat map. Nested
/// structs are flattened with dot notation (e.g. "color.x"); types that read as
/// a single [`PropertyValue`] (Vec3, Color, …) stop there rather than recursing.
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
                let Some(field) = s.field_at(i) else { continue };
                if let Some(val) = read_value_from_reflect(field) {
                    out.insert(full_name, val);
                } else {
                    collect_struct_fields(field, &full_name, out);
                }
            }
        }
        _ => {
            if let Some(val) = read_value_from_reflect(reflect) {
                if !prefix.is_empty() {
                    out.insert(prefix.to_string(), val);
                }
            }
        }
    }
}

/// Get the short names of all reflected components actually present on an entity.
pub fn get_entity_component_names(world: &World, entity: Entity) -> Vec<String> {
    let type_registry = world.resource::<AppTypeRegistry>().clone();
    let registry = type_registry.read();
    let mut names = Vec::new();

    let entity_ref = world.entity(entity);
    let archetype = entity_ref.archetype();

    for &component_id in archetype.components() {
        let Some(info) = world.components().get_info(component_id) else {
            continue;
        };
        let type_id = match info.type_id() {
            Some(id) => id,
            None => continue,
        };
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

// ============================================================================
// Animatable-field discovery (property-animation "Add Property" picker)
// ============================================================================

/// The animatable value kind of a field — used to filter/display in the picker.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnimFieldKind {
    Float,
    Vec3,
    Quat,
    Color,
    Bool,
}

/// One animatable field offered by the property-animation picker.
#[derive(Debug, Clone)]
pub struct AnimatableField {
    /// Reflected component short-name (e.g. "transform", "directional_light").
    pub component: String,
    /// Dotted reflection field path (e.g. "translation", "illuminance").
    pub field: String,
    pub kind: AnimFieldKind,
    /// Human-friendly label for display (e.g. "Translation").
    pub label: String,
}

/// Enumerate the animatable fields of every reflected component on `entity`.
///
/// Transform is special-cased to its three transform channels (the generic
/// reflection path can't surface `Quat` rotation as one field). Other components
/// are enumerated via reflection and filtered to interpolatable value kinds.
pub fn list_animatable_fields(world: &World, entity: Entity) -> Vec<AnimatableField> {
    let mut out = Vec::new();
    for component in get_entity_component_names(world, entity) {
        let lc = component.to_lowercase();
        if lc == "transform" {
            for (field, kind) in [
                ("translation", AnimFieldKind::Vec3),
                // Rotation animates as Euler degrees (Vec3) so a 0→360 key pair
                // produces a real spin (quaternion slerp would take the short path).
                ("rotation", AnimFieldKind::Vec3),
                ("scale", AnimFieldKind::Vec3),
            ] {
                out.push(AnimatableField {
                    component: component.clone(),
                    field: field.to_string(),
                    kind,
                    label: prettify_field(field),
                });
            }
            continue;
        }
        let Some(fields) = get_all_component_fields(world, entity, &component) else {
            continue;
        };
        let mut entries: Vec<(String, AnimFieldKind)> = fields
            .into_iter()
            .filter_map(|(field, value)| anim_field_kind(&value).map(|k| (field, k)))
            .collect();
        entries.sort_by(|a, b| a.0.cmp(&b.0));
        for (field, kind) in entries {
            let label = prettify_field(&field);
            out.push(AnimatableField {
                component: component.clone(),
                field,
                kind,
                label,
            });
        }
    }
    out
}

fn anim_field_kind(value: &PropertyValue) -> Option<AnimFieldKind> {
    match value {
        PropertyValue::Float(_) | PropertyValue::Int(_) => Some(AnimFieldKind::Float),
        PropertyValue::Vec3(_) => Some(AnimFieldKind::Vec3),
        PropertyValue::Color(_) => Some(AnimFieldKind::Color),
        PropertyValue::Bool(_) => Some(AnimFieldKind::Bool),
        PropertyValue::String(_) => None,
    }
}

/// Title-case a dotted field path for display ("base_color" -> "Base Color").
fn prettify_field(field: &str) -> String {
    field
        .replace(['_', '.'], " ")
        .split_whitespace()
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}
