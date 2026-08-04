//! Reflection helpers for reading component fields via Bevy's reflection system.
//!
//! These live in renzora so both scripting and blueprint crates can use them
//! without creating a dependency between each other.
//!
//! ## What is Bevy's and what is ours
//!
//! Path navigation is **Bevy's** — [`GetPath::reflect_path`] parses a dotted
//! string and walks structs, tuple structs, lists and maps, treating a numeric
//! segment as a tuple index exactly the way the hand-written navigator this
//! module used to carry did. That navigator (three mutually recursive functions,
//! ~100 lines) was deleted in favour of it.
//!
//! What remains here is genuinely ours and has no Bevy equivalent:
//!   * [`resolve_field_alias`] — the friendly names scripts use (`Text.content`
//!     for `Text.0`).
//!   * [`read_value_from_reflect`] / [`apply_value_to_reflect`] — conversion to
//!     and from [`PropertyValue`], the single wire type scripts and animation
//!     tracks speak. The `apply` half is *lossy on purpose*: it rounds a float
//!     into an integer field, which `PartialReflect::try_apply` will not do
//!     (it requires an exact type match), and which property animation depends
//!     on — every track carries floats.
//!   * The case-insensitive short-name component lookup scripts rely on.

use bevy::prelude::*;
use bevy::reflect::{GetPath, ReflectRef, TypeRegistration, TypeRegistry};

use crate::PropertyValue;

/// Find a registered component by its **short, case-insensitive** type name
/// (`"transform"` matches `bevy_transform::components::Transform`).
///
/// Scripts and the undo stack both address components this way — a user types
/// `get("Transform.translation.x")`, not a full Rust path. Deduplicated here
/// because five call sites carried a copy of this loop.
fn find_registration<'a>(
    registry: &'a TypeRegistry,
    component_type: &str,
) -> Option<&'a TypeRegistration> {
    let query = component_type.to_lowercase();
    registry.iter().find(|reg| {
        let path = reg.type_info().type_path();
        let short = path.rsplit("::").next().unwrap_or(path);
        short.to_lowercase() == query
    })
}

/// The reflected component on `entity`, by short type name.
fn reflect_on<'a>(
    world: &'a World,
    entity: Entity,
    registry: &TypeRegistry,
    component_type: &str,
) -> Option<&'a dyn bevy::reflect::Reflect> {
    let reflect_component = find_registration(registry, component_type)?.data::<ReflectComponent>()?;
    reflect_component.reflect(world.get_entity(entity).ok()?)
}

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
    let reflected = reflect_on(world, entity, &registry, component_type)?;
    let resolved_path = resolve_field_alias(component_type, field_path);
    read_value_from_reflect(reflected.reflect_path(resolved_path.as_str()).ok()?)
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
    let reflected = reflect_on(world, entity, &registry, component_type)?;
    let field = reflected.reflect_path(field_path).ok()?;
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
    let Some(reflect_component) =
        find_registration(&registry, component_type).and_then(|r| r.data::<ReflectComponent>())
    else {
        return false;
    };
    let Some(reflected) = reflect_on(world, entity, &registry, component_type) else {
        return false;
    };
    // Mutate a clone and `apply` the whole component rather than poking the live
    // one: `apply` goes through `Mut`, so change detection fires — which the
    // inspector's two-way bindings and every `Changed<T>` system depend on.
    let Ok(mut cloned) = reflected.reflect_clone() else {
        return false;
    };

    let resolved_path = resolve_field_alias(component_type, field_path);
    let Ok(target) = cloned.reflect_path_mut(resolved_path.as_str()) else {
        return false;
    };
    if !apply_value_to_reflect(target, value) {
        return false;
    }
    let Ok(mut entity_mut) = world.get_entity_mut(entity) else {
        return false;
    };
    reflect_component.apply(&mut entity_mut, cloned.as_partial_reflect());
    true
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
    reflect_on(world, entity, &registry, component_type)?
        .reflect_clone()
        .ok()
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
    let Some(reflect_component) =
        find_registration(&registry, component_type).and_then(|r| r.data::<ReflectComponent>())
    else {
        return false;
    };
    let Ok(mut entity_mut) = world.get_entity_mut(entity) else {
        return false;
    };
    reflect_component.insert(&mut entity_mut, value.as_partial_reflect(), &registry);
    true
}

/// Apply a [`PropertyValue`] onto a reflected field, coercing numeric types.
fn apply_value_to_reflect(
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
    let reflected = reflect_on(world, entity, &registry, component_type)?;

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

// ============================================================================
// Tests
// ============================================================================
//
// These pin the path-navigation behaviour that used to be hand-written here and
// is now delegated to Bevy's `GetPath`. They are the reason that swap is safe:
// the scripting `get`/`set` API addresses fields by string, so a change in path
// semantics would break user scripts silently rather than at compile time.
//
// NOTE: run with `renzora test` — `cargo test` does not link natively on
// Windows (CLAUDE.md §2).
#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Component, Reflect, Default)]
    #[reflect(Component)]
    struct Nested {
        value: f32,
        count: u32,
    }

    #[derive(Component, Reflect, Default)]
    #[reflect(Component)]
    struct Sample {
        flag: bool,
        label: String,
        nested: Nested,
        offset: Vec3,
    }

    /// Stands in for Bevy's tuple-struct components (`Text(String)`), which
    /// reflection only exposes as field "0".
    #[derive(Component, Reflect, Default)]
    #[reflect(Component)]
    struct Tup(String);

    fn setup() -> (World, Entity) {
        let mut world = World::new();
        let registry = AppTypeRegistry::default();
        {
            let mut w = registry.write();
            w.register::<Sample>();
            w.register::<Nested>();
            w.register::<Tup>();
        }
        world.insert_resource(registry);
        let entity = world
            .spawn((
                Sample {
                    flag: true,
                    label: "hello".into(),
                    nested: Nested { value: 1.5, count: 7 },
                    offset: Vec3::new(1.0, 2.0, 3.0),
                },
                Tup("tuple".into()),
            ))
            .id();
        (world, entity)
    }

    #[test]
    fn reads_top_level_and_nested_fields() {
        let (world, e) = setup();
        assert!(matches!(
            get_reflected_field(&world, e, "Sample", "flag"),
            Some(PropertyValue::Bool(true))
        ));
        assert!(matches!(
            get_reflected_field(&world, e, "Sample", "nested.value"),
            Some(PropertyValue::Float(v)) if (v - 1.5).abs() < f32::EPSILON
        ));
    }

    /// The component name is matched case-insensitively on the SHORT type name —
    /// scripts write `get("sample.flag")`, not a full Rust path.
    #[test]
    fn component_lookup_is_case_insensitive() {
        let (world, e) = setup();
        assert!(get_reflected_field(&world, e, "sample", "flag").is_some());
        assert!(get_reflected_field(&world, e, "SAMPLE", "flag").is_some());
    }

    /// A numeric path segment indexes a tuple struct. This is the case the
    /// hand-written navigator special-cased and `GetPath` handles natively.
    #[test]
    fn reads_tuple_struct_by_index() {
        let (world, e) = setup();
        assert!(matches!(
            get_reflected_field(&world, e, "Tup", "0"),
            Some(PropertyValue::String(s)) if s == "tuple"
        ));
    }

    /// Vec3 stops at the value rather than recursing into x/y/z…
    #[test]
    fn vec3_reads_as_one_value_but_components_are_addressable() {
        let (world, e) = setup();
        assert!(matches!(
            get_reflected_field(&world, e, "Sample", "offset"),
            Some(PropertyValue::Vec3([1.0, 2.0, 3.0]))
        ));
        // …and is still reachable field-by-field through the same path syntax.
        assert!(matches!(
            get_reflected_field(&world, e, "Sample", "offset.y"),
            Some(PropertyValue::Float(v)) if (v - 2.0).abs() < f32::EPSILON
        ));
    }

    #[test]
    fn writes_round_trip() {
        let (mut world, e) = setup();
        assert!(set_reflected_field(
            &mut world,
            e,
            "Sample",
            "nested.value",
            &PropertyValue::Float(9.25)
        ));
        assert!(matches!(
            get_reflected_field(&world, e, "Sample", "nested.value"),
            Some(PropertyValue::Float(v)) if (v - 9.25).abs() < f32::EPSILON
        ));
    }

    /// The lossy half of [`apply_value_to_reflect`]: property-animation tracks
    /// carry only floats, so a float written into an integer field must round
    /// rather than fail. `PartialReflect::try_apply` would reject this outright,
    /// which is why that function still exists.
    #[test]
    fn float_writes_round_into_integer_fields() {
        let (mut world, e) = setup();
        assert!(set_reflected_field(
            &mut world,
            e,
            "Sample",
            "nested.count",
            &PropertyValue::Float(3.7)
        ));
        assert!(matches!(
            get_reflected_field(&world, e, "Sample", "nested.count"),
            Some(PropertyValue::Int(4))
        ));
    }

    #[test]
    fn missing_component_or_field_is_none_not_panic() {
        let (mut world, e) = setup();
        assert!(get_reflected_field(&world, e, "NoSuchComponent", "flag").is_none());
        assert!(get_reflected_field(&world, e, "Sample", "no_such_field").is_none());
        assert!(!set_reflected_field(
            &mut world,
            e,
            "Sample",
            "no_such_field",
            &PropertyValue::Float(1.0)
        ));
    }

    /// A write whose value kind cannot fit the field must fail cleanly and leave
    /// the component untouched.
    #[test]
    fn mismatched_value_kind_fails_without_writing() {
        let (mut world, e) = setup();
        assert!(!set_reflected_field(
            &mut world,
            e,
            "Sample",
            "label",
            &PropertyValue::Float(1.0)
        ));
        assert!(matches!(
            get_reflected_field(&world, e, "Sample", "label"),
            Some(PropertyValue::String(s)) if s == "hello"
        ));
    }

    #[test]
    fn captured_component_can_be_reinserted() {
        let (mut world, e) = setup();
        let captured = capture_component(&world, e, "Sample").expect("captured");
        world.entity_mut(e).remove::<Sample>();
        assert!(get_reflected_field(&world, e, "Sample", "flag").is_none());
        assert!(insert_component_reflected(&mut world, e, "Sample", captured.as_ref()));
        assert!(matches!(
            get_reflected_field(&world, e, "Sample", "flag"),
            Some(PropertyValue::Bool(true))
        ));
    }
}
