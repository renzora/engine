//! Inspector rows generated from `bevy_reflect`, with no per-component code.
//!
//! The hand-written path (`InspectorRegistry` + `FieldDef` + the `*_field!`
//! macros) describes each component's fields a second time, by hand, in the
//! crate that owns the component — ~420 call sites across the workspace. Every
//! one of those types is *already* registered with Bevy's type registry
//! (`#[derive(Reflect)]` + `register_type`), which carries the same information:
//! field names, field types, enum variants. This module reads that registration
//! and produces the same rows the renderer already knows how to draw.
//!
//! The point is which way the dependency runs. A `FieldDef` forces the owning
//! crate to name `FieldType`/`FieldValue`, so those types have to live somewhere
//! both the component and the inspector can see — that is the entire reason the
//! inspector contract sits in the `renzora` hub that 134 crates depend on.
//! Reflection is late-bound: nothing here names a single engine type, so a
//! component crate needs no editor dependency at all, and the inspector needs no
//! knowledge of the component.
//!
//! ## What this deliberately does not do
//!
//! Reflection gives structure, not presentation. It cannot know that a field is
//! a 0..1 slider, that a `Vec3` is a colour rather than a position, or that two
//! fields belong under one heading. Bevy's answer is `#[reflect(@..)]` custom
//! attributes and `TypeData` — of which this workspace currently uses **zero**.
//! Until those are added, generated rows use wide ranges and name-based
//! heuristics, and anything unrecognised degrades to a read-only row rather than
//! being dropped, so a generated section is always a complete picture of the
//! component even when it is not yet a pretty one.

use std::collections::HashSet;
use std::sync::{Mutex, OnceLock};

use bevy::ecs::component::ComponentId;
use bevy::prelude::*;
use bevy::reflect::enums::{DynamicEnum, DynamicVariant, VariantInfo};
// `GetPath` is what turns a dotted string into a reflected field. Note its impl
// is `T: Reflect`, not `PartialReflect` — paths must be walked from the `&dyn
// Reflect` the component hands back, never from an `as_partial_reflect()` view.
use bevy::reflect::{GetPath, ReflectRef, TypeInfo};

use renzora::{FieldType, FieldValue};

/// One generated row: what to draw, and the reflect path to read/write it.
pub struct ReflectField {
    /// Prettified label (`rayleigh_scattering` → `Rayleigh Scattering`).
    pub label: &'static str,
    /// Dotted path within the component, as understood by `GetPath`
    /// (`sun.intensity`). Leaked to `'static` via [`intern`] because the
    /// renderer stores it in a `Component` and Bevy components are `'static`.
    pub path: &'static str,
    pub field_type: FieldType,
    pub value: FieldValue,
}

/// One generated component section.
pub struct ReflectSection {
    /// Full Rust type path — the key used to look the component back up on
    /// write. Already `'static`: it comes from `TypeInfo`, which the registry
    /// owns for the lifetime of the app.
    pub type_path: &'static str,
    /// Short type name, used as the section title.
    pub short_name: &'static str,
    /// The component carries a `bool` field named `enabled`, so the section can
    /// show the same on/off switch a hand-written entry wires by hand through
    /// `is_enabled_fn`/`set_enabled_fn`. A convention rather than a declaration —
    /// see [`ENABLED_FIELD`].
    pub has_enabled: bool,
    pub fields: Vec<ReflectField>,
}

/// The field name that means "this effect is on". Every hand-written
/// `is_enabled_fn` in the workspace reads exactly this, so promoting it to a
/// convention costs nothing and removes another reason for a component crate to
/// name an editor type.
const ENABLED_FIELD: &str = "enabled";

/// How much of the inspector the generated path should take over. Lets a
/// generated section be compared against the hand-written one for the same
/// component without rebuilding the editor.
#[derive(Resource, Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum ReflectInspectorMode {
    /// Hand-written sections only.
    ///
    /// **The default, deliberately.** `FillGaps` looked additive and safe, and
    /// is not: "has no hand-written entry" turns out to correlate strongly with
    /// "is not authored state". Generating sections for those produced rows that
    /// cannot be edited — `renzora_lumen` re-`try_insert`s `RtLighting` from its
    /// routing system every time settings change, so deleting the component or
    /// flipping its toggle silently reverts within a frame.
    ///
    /// Reflection cannot distinguish an authored component from a derived one.
    /// Until that is declared somewhere, generating by default does more harm
    /// than the missing rows did.
    #[default]
    Off,
    /// Generate sections for reflected components with no hand-written entry.
    /// Useful for surveying what exists; expect inert rows on derived
    /// components (see [`DERIVED_COMPONENTS`], which covers Bevy's but cannot
    /// know about every plugin's).
    FillGaps,
    /// Generate a section for every reflected component, alongside the
    /// hand-written ones. The A/B mode — this is what answers "how much of the
    /// 6,257 lines does reflection actually replace".
    All,
}

// ── string interning ─────────────────────────────────────────────────────
//
// Labels and nested paths are built at runtime (`format!("{parent}.{child}")`)
// but the renderer stores them in Bevy components, which must be `'static`.
// The set of (type, field) pairs is fixed once the app has built, so interning
// leaks a bounded amount — one allocation per distinct string, never per frame.
// A plain leak per call would grow without bound while scrubbing the hierarchy.

static INTERNED: OnceLock<Mutex<HashSet<&'static str>>> = OnceLock::new();

fn intern(s: &str) -> &'static str {
    let set = INTERNED.get_or_init(|| Mutex::new(HashSet::new()));
    let mut set = match set.lock() {
        Ok(g) => g,
        // A poisoned intern table is not worth killing the editor over; leaking
        // one extra copy is strictly better than a panic mid-frame.
        Err(_) => return Box::leak(s.to_string().into_boxed_str()),
    };
    if let Some(existing) = set.get(s) {
        return existing;
    }
    let leaked: &'static str = Box::leak(s.to_string().into_boxed_str());
    set.insert(leaked);
    leaked
}

/// `rayleigh_scattering` → `Rayleigh Scattering`.
fn prettify(name: &str) -> String {
    let mut out = String::with_capacity(name.len() + 4);
    for (i, word) in name.split('_').enumerate() {
        if word.is_empty() {
            continue;
        }
        if i > 0 {
            out.push(' ');
        }
        let mut chars = word.chars();
        if let Some(first) = chars.next() {
            out.extend(first.to_uppercase());
            out.push_str(chars.as_str());
        }
    }
    out
}

fn short_name(type_path: &str) -> &str {
    // Strip generics before taking the last segment, so `Handle<Image>` doesn't
    // become `Image>`.
    let base = type_path.split('<').next().unwrap_or(type_path);
    base.rsplit("::").next().unwrap_or(base)
}

// ── value mapping ────────────────────────────────────────────────────────

/// A very wide numeric range. The hand-written path supplies real clamps per
/// field; reflection has no way to know them until the workspace starts using
/// `#[reflect(@..)]` attributes, and a wrong clamp silently destroys data on
/// edit, so an unbounded drag is the safe default.
const WIDE: f32 = 1.0e9;

/// Whether a field name suggests a colour, used to pick a colour widget for a
/// bare `Vec3`/`Vec4`. A heuristic on purpose — the correct fix is a reflect
/// attribute on the field, and this is what it looks like without one.
fn looks_like_color(label: &str) -> bool {
    let l = label.to_ascii_lowercase();
    l.contains("color") || l.contains("colour") || l.contains("tint") || l.contains("albedo")
}

/// Map one reflected leaf to a widget + value. `None` means "not a leaf" — the
/// caller then recurses into it as a struct.
fn leaf_to_field(
    value: &dyn bevy::reflect::PartialReflect,
    label: &str,
    range: Option<(f32, f32)>,
) -> Option<(FieldType, FieldValue)> {
    // A declared range also fixes the drag speed: 200 pixels sweeps the whole
    // range, which is what makes a 0..1 field feel different from a 0..1000 one.
    // Without an attribute both get the same unbounded 0.01 crawl.
    let (min, max) = range.unwrap_or((-WIDE, WIDE));
    let speed = range
        .map(|(lo, hi)| ((hi - lo) / 200.0).max(0.0001))
        .unwrap_or(0.01);
    if let Some(v) = value.try_downcast_ref::<f32>() {
        return Some((FieldType::Float { speed, min, max }, FieldValue::Float(*v)));
    }
    if let Some(v) = value.try_downcast_ref::<f64>() {
        return Some((
            FieldType::Float { speed, min, max },
            FieldValue::Float(*v as f32),
        ));
    }
    // Integers travel as `FieldValue::Float` — the registry has one numeric wire
    // type — but `FieldType::Int` makes the widget snap its model, which is
    // required or the fractional drag and the rounded re-read fight each other.
    if let Some(v) = value.try_downcast_ref::<i32>() {
        return Some((FieldType::Int { min, max }, FieldValue::Float(*v as f32)));
    }
    if let Some(v) = value.try_downcast_ref::<i64>() {
        return Some((FieldType::Int { min, max }, FieldValue::Float(*v as f32)));
    }
    if let Some(v) = value.try_downcast_ref::<u32>() {
        return Some((FieldType::Int { min: min.max(0.0), max }, FieldValue::Float(*v as f32)));
    }
    if let Some(v) = value.try_downcast_ref::<u64>() {
        return Some((FieldType::Int { min: min.max(0.0), max }, FieldValue::Float(*v as f32)));
    }
    if let Some(v) = value.try_downcast_ref::<usize>() {
        return Some((FieldType::Int { min: min.max(0.0), max }, FieldValue::Float(*v as f32)));
    }
    if let Some(v) = value.try_downcast_ref::<bool>() {
        return Some((FieldType::Bool, FieldValue::Bool(*v)));
    }
    if let Some(v) = value.try_downcast_ref::<String>() {
        return Some((FieldType::String, FieldValue::String(v.clone())));
    }
    if let Some(v) = value.try_downcast_ref::<Color>() {
        let c = v.to_srgba();
        return Some((
            FieldType::ColorRgba,
            FieldValue::ColorRgba([c.red, c.green, c.blue, c.alpha]),
        ));
    }
    if let Some(v) = value.try_downcast_ref::<Vec3>() {
        return if looks_like_color(label) {
            Some((FieldType::Color, FieldValue::Color([v.x, v.y, v.z])))
        } else {
            Some((FieldType::Vec3 { speed: 0.01 }, FieldValue::Vec3([v.x, v.y, v.z])))
        };
    }
    if let Some(v) = value.try_downcast_ref::<Vec4>() {
        return if looks_like_color(label) {
            Some((FieldType::ColorRgba, FieldValue::ColorRgba([v.x, v.y, v.z, v.w])))
        } else {
            // No 4-component numeric widget exists; showing it read-only beats
            // silently dropping the field from the section.
            Some((
                FieldType::ReadOnly,
                FieldValue::ReadOnly(format!("({:.3}, {:.3}, {:.3}, {:.3})", v.x, v.y, v.z, v.w)),
            ))
        };
    }
    if let Some(v) = value.try_downcast_ref::<Vec2>() {
        return Some((
            FieldType::ReadOnly,
            FieldValue::ReadOnly(format!("({:.3}, {:.3})", v.x, v.y)),
        ));
    }
    None
}

/// Unit-only enums render as a dropdown. `variant_names()` borrows from the
/// registry's `TypeInfo`, which lives as long as the app, so the options slice
/// is genuinely `'static` — no interning needed.
fn enum_field(
    value: &dyn bevy::reflect::PartialReflect,
    registry: &bevy::reflect::TypeRegistry,
) -> Option<(FieldType, FieldValue)> {
    let ReflectRef::Enum(e) = value.reflect_ref() else {
        return None;
    };
    let type_info = value.get_represented_type_info()?;
    let TypeInfo::Enum(info) = type_info else {
        return None;
    };
    let _ = registry;
    // Data-carrying variants would need a nested editor; a dropdown that
    // silently discarded the payload on switch would be a data-loss bug, so
    // those fall through to the read-only path in the caller.
    if info.iter().any(|v| !matches!(v, VariantInfo::Unit(_))) {
        return None;
    }
    let options: &'static [&'static str] = info.variant_names();
    Some((
        FieldType::Enum { options },
        FieldValue::Enum(e.variant_name().to_string()),
    ))
}

// ── section generation ───────────────────────────────────────────────────

/// Components that are *derived*, not authored: the engine recomputes them every
/// frame from something else, so a row for them is at best inert and at worst
/// looks like an edit that silently reverts.
///
/// This list is the honest cost of the reflection approach. Reflection knows a
/// component's shape but not its *provenance* — there is no `Reflect` signal for
/// "this is an output, not an input" — so somebody has to say so. Note how short
/// it is next to the ~590 hand-written field descriptions it replaces: the
/// registry's irreplaceable value was curation, not field data.
const DERIVED_COMPONENTS: &[&str] = &[
    // Transform propagation outputs.
    "GlobalTransform",
    "TransformTreeChanged",
    // Visibility propagation outputs (`Visibility` is the authored input).
    "InheritedVisibility",
    "ViewVisibility",
    "VisibilityClass",
    "VisibleEntities",
    // Render-world plumbing.
    "RenderEntity",
    "MainEntity",
    "SyncToRenderWorld",
    "NoFrustumCulling",
    // Shadow-cascade outputs, recomputed per light per camera.
    "Cascades",
    "CascadesFrusta",
    "CascadesVisibleEntities",
    // Culling volumes recomputed from mesh + transform.
    "Aabb",
    "Frustum",
];

fn is_derived(short: &str) -> bool {
    DERIVED_COMPONENTS.contains(&short)
}

/// Maximum struct nesting to flatten. Two levels covers the common shape
/// (`Settings { sun: SunConfig { intensity } }`) without turning a deeply
/// nested component into hundreds of rows.
const MAX_DEPTH: usize = 2;

fn walk(
    value: &dyn bevy::reflect::PartialReflect,
    prefix: &str,
    depth: usize,
    registry: &bevy::reflect::TypeRegistry,
    out: &mut Vec<ReflectField>,
) {
    // One entry per member to draw: the path segment that addresses it, the
    // label it contributes, the value, and any declared range. Collected first
    // so named structs and newtypes share the single row-building loop below —
    // the two differ only in how a member is named.
    type Member<'a> = (String, String, &'a dyn bevy::reflect::PartialReflect, Option<(f32, f32)>);
    let mut members: Vec<Member<'_>> = Vec::new();
    match value.reflect_ref() {
        ReflectRef::Struct(s) => {
            // The *value* view (`ReflectRef`) carries field names and data; the
            // *schema* view (`TypeInfo`) additionally carries `#[reflect(@..)]`
            // custom attributes. Both are needed: the value to read, the schema
            // to know how to present it.
            let struct_info = match value.get_represented_type_info() {
                Some(TypeInfo::Struct(info)) => Some(info),
                _ => None,
            };
            for i in 0..s.field_len() {
                let Some(name) = s.name_at(i) else { continue };
                let Some(field) = s.field_at(i) else { continue };
                // `#[reflect(@0.0f32..=5.0f32)]` — the clamp, declared on the
                // component in Bevy's own vocabulary. This is the whole point:
                // the owning crate says what the range is without naming a
                // single Renzora type.
                let range = struct_info
                    .and_then(|info| info.field_at(i))
                    .and_then(|f| f.get_attribute::<core::ops::RangeInclusive<f32>>())
                    .map(|r| (*r.start(), *r.end()));
                members.push((name.to_string(), prettify(name), field, range));
            }
        }
        // A newtype (`struct Score(u32)`) is as common a shape as a named
        // struct — especially for resources — and reflection addresses its
        // members by index, which `GetPath` parses from a bare `0` segment.
        // Skipping these was why a resource browser showed nothing at all for a
        // large share of what it listed.
        ReflectRef::TupleStruct(t) => {
            let tuple_info = match value.get_represented_type_info() {
                Some(TypeInfo::TupleStruct(info)) => Some(info),
                _ => None,
            };
            for i in 0..t.field_len() {
                let Some(field) = t.field(i) else { continue };
                let range = tuple_info
                    .and_then(|info| info.field_at(i))
                    .and_then(|f| f.get_attribute::<core::ops::RangeInclusive<f32>>())
                    .map(|r| (*r.start(), *r.end()));
                // A single unnamed member has no name of its own to show, so it
                // takes the containing field's (or "Value" at the root).
                let label = if t.field_len() == 1 {
                    String::new()
                } else {
                    i.to_string()
                };
                members.push((i.to_string(), label, field, range));
            }
        }
        _ => return,
    }

    for (segment, own_label, field, range) in members {
        let path = if prefix.is_empty() {
            segment.clone()
        } else {
            format!("{prefix}.{segment}")
        };
        // Nested labels carry their parent so two `intensity` rows under
        // different sub-structs stay distinguishable.
        let parent = prettify(prefix.rsplit('.').next().unwrap_or(prefix));
        let label = match (prefix.is_empty(), own_label.is_empty()) {
            (true, true) => "Value".to_string(),
            (true, false) => own_label,
            (false, true) => parent,
            (false, false) => format!("{parent} {own_label}"),
        };

        if let Some((field_type, val)) = leaf_to_field(field, &label, range) {
            out.push(ReflectField {
                label: intern(&label),
                path: intern(&path),
                field_type,
                value: val,
            });
            continue;
        }
        if let Some((field_type, val)) = enum_field(field, registry) {
            out.push(ReflectField {
                label: intern(&label),
                path: intern(&path),
                field_type,
                value: val,
            });
            continue;
        }
        if depth < MAX_DEPTH
            && matches!(
                field.reflect_ref(),
                ReflectRef::Struct(_) | ReflectRef::TupleStruct(_)
            )
        {
            walk(field, &path, depth + 1, registry, out);
            continue;
        }
        // Unrecognised: show what it is rather than hiding it, so a generated
        // section is always an honest inventory of the component.
        let type_name = field
            .get_represented_type_info()
            .map(|t| short_name(t.type_path()).to_string())
            .unwrap_or_else(|| "?".to_string());
        out.push(ReflectField {
            label: intern(&label),
            path: intern(&path),
            field_type: FieldType::ReadOnly,
            value: FieldValue::ReadOnly(format!("<{type_name}>")),
        });
    }
}

/// Generate a section for every reflected component on `entity` that `skip`
/// does not claim. `skip` receives the lowercased short type name, which is what
/// the hand-written registry keys on.
pub fn reflect_sections(
    world: &World,
    entity: Entity,
    skip: &dyn Fn(&str) -> bool,
) -> Vec<ReflectSection> {
    let Some(app_registry) = world.get_resource::<AppTypeRegistry>() else {
        return Vec::new();
    };
    let app_registry = app_registry.clone();
    let registry = app_registry.read();

    let Ok(entity_ref) = world.get_entity(entity) else {
        return Vec::new();
    };

    let mut out = Vec::new();
    for &component_id in entity_ref.archetype().components() {
        let Some(info) = world.components().get_info(component_id) else {
            continue;
        };
        let Some(type_id) = info.type_id() else { continue };
        let Some(registration) = registry.get(type_id) else {
            continue;
        };
        let Some(reflect_component) = registration.data::<ReflectComponent>() else {
            continue;
        };
        let type_path = registration.type_info().type_path();
        let short = short_name(type_path);
        if is_derived(short) || skip(&short.to_ascii_lowercase()) {
            continue;
        }
        let Some(reflected) = reflect_component.reflect(entity_ref) else {
            continue;
        };

        let mut fields = Vec::new();
        walk(reflected.as_partial_reflect(), "", 0, &registry, &mut fields);
        // Drop sections with nothing to edit. A marker component (no fields) or
        // one whose every field is opaque to reflection yields a section that can
        // only be expanded to reveal nothing — noise that buries the real ones.
        // The hand-written registry never had this problem because a human chose
        // what to register; the generated path has to earn its place per section.
        if !fields.iter().any(|f| !matches!(f.field_type, FieldType::ReadOnly)) {
            continue;
        }
        // Promote `enabled` from a checkbox row to the section's own switch, the
        // way the hand-written entries do — and remove the row, or it appears
        // twice.
        let has_enabled = fields
            .iter()
            .position(|f| f.path == ENABLED_FIELD && matches!(f.field_type, FieldType::Bool))
            .map(|i| {
                fields.remove(i);
                true
            })
            .unwrap_or(false);
        out.push(ReflectSection {
            type_path,
            short_name: intern(short),
            has_enabled,
            fields,
        });
    }

    out.sort_by_key(|s| s.short_name);
    out
}

// ── read / write by path ─────────────────────────────────────────────────

/// Re-read one generated field. The renderer refreshes rows every frame, so
/// this is the generated counterpart of a `FieldDef::get_fn`.
pub fn read_field(
    world: &World,
    entity: Entity,
    type_path: &str,
    path: &str,
    read_only: bool,
) -> Option<FieldValue> {
    let app_registry = world.get_resource::<AppTypeRegistry>()?.clone();
    let registry = app_registry.read();
    let registration = registry.get_with_type_path(type_path)?;
    let reflect_component = registration.data::<ReflectComponent>()?;
    let entity_ref = world.get_entity(entity).ok()?;
    let reflected = reflect_component.reflect(entity_ref)?;

    let target = reflected.reflect_path(path).ok()?;
    if let Some((_, v)) = leaf_to_field(target, path, None) {
        return Some(v);
    }
    if let Some((_, v)) = enum_field(target, &registry) {
        return Some(v);
    }
    // Keep the row alive rather than returning `None`, which the renderer reads
    // as "row no longer applicable" and hides — an informational row would
    // silently vanish from the section on the first refresh.
    if read_only {
        let shown = target
            .get_represented_type_info()
            .map(|t| format!("<{}>", short_name(t.type_path())))
            .unwrap_or_else(|| "<?>".to_string());
        return Some(FieldValue::ReadOnly(shown));
    }
    None
}

/// A component the Add Component overlay can offer without anyone registering it.
pub struct AddableComponent {
    pub type_path: &'static str,
    pub short_name: &'static str,
}

/// Every component that can be added to an entity purely from its registration.
///
/// "Addable" is *inferred*, not declared: a type qualifies if it is a reflected
/// component (`ReflectComponent`) that knows how to construct itself
/// (`ReflectDefault`, i.e. the crate wrote `#[reflect(Default)]`). Both are Bevy
/// vocabulary, so a component crate opts in without importing anything from the
/// editor — which is the entire point.
///
/// Two exclusions keep the menu meaningful rather than exhaustive:
///   * [`is_derived`] — engine outputs nobody authors.
///   * anything from `bevy_*` — Bevy's own components that are genuinely worth
///     adding already have curated entries, so inferring them would only
///     duplicate those and bury them under internals.
pub fn addable_components(world: &World) -> Vec<AddableComponent> {
    let Some(app_registry) = world.get_resource::<AppTypeRegistry>() else {
        return Vec::new();
    };
    let app_registry = app_registry.clone();
    let registry = app_registry.read();

    let mut out: Vec<AddableComponent> = registry
        .iter()
        .filter(|reg| {
            reg.data::<ReflectComponent>().is_some()
                && reg.data::<bevy::reflect::std_traits::ReflectDefault>().is_some()
        })
        .filter_map(|reg| {
            let type_path = reg.type_info().type_path();
            if type_path.starts_with("bevy_") {
                return None;
            }
            let short = short_name(type_path);
            if is_derived(short) {
                return None;
            }
            Some(AddableComponent {
                type_path,
                short_name: intern(short),
            })
        })
        .collect();
    out.sort_by_key(|c| c.short_name);
    out
}

/// Insert a component by type path, built from its `ReflectDefault` — the
/// generic replacement for a hand-written `add_fn`.
pub fn add_component(world: &mut World, entity: Entity, type_path: &str) -> bool {
    let Some(app_registry) = world.get_resource::<AppTypeRegistry>() else {
        return false;
    };
    let app_registry = app_registry.clone();
    let registry = app_registry.read();
    let Some(registration) = registry.get_with_type_path(type_path) else {
        return false;
    };
    let Some(reflect_component) = registration.data::<ReflectComponent>().cloned() else {
        return false;
    };
    let Some(value) = registration
        .data::<bevy::reflect::std_traits::ReflectDefault>()
        .map(|d| d.default())
    else {
        return false;
    };
    // `insert` needs the registry to resolve the value's own type data, so hand
    // it the guard's registry rather than dropping it first.
    let Ok(mut entity_mut) = world.get_entity_mut(entity) else {
        return false;
    };
    reflect_component.insert(&mut entity_mut, value.as_partial_reflect(), &registry);
    true
}

/// Whether a component is currently on the entity — the generic `has_fn`.
pub fn has_component(world: &World, entity: Entity, type_path: &str) -> bool {
    let Some(app_registry) = world.get_resource::<AppTypeRegistry>() else {
        return false;
    };
    let app_registry = app_registry.clone();
    let registry = app_registry.read();
    let Some(reflect_component) = registry
        .get_with_type_path(type_path)
        .and_then(|r| r.data::<ReflectComponent>())
    else {
        return false;
    };
    world
        .get_entity(entity)
        .ok()
        .and_then(|e| reflect_component.reflect(e))
        .is_some()
}

/// Remove a component by type path — the generic replacement for a hand-written
/// `remove_fn`.
///
/// Note what this deliberately cannot do: a hand-written `remove_fn` sometimes
/// removes a *pair* (`renzora_vignette` drops both `VignetteSettings` and bevy's
/// `Vignette`). That pairing is domain knowledge, and the right home for it is
/// the owning crate's own cleanup system reacting to the settings component
/// going away — not the editor.
pub fn remove_component(world: &mut World, entity: Entity, type_path: &str) -> bool {
    let Some(app_registry) = world.get_resource::<AppTypeRegistry>() else {
        return false;
    };
    let app_registry = app_registry.clone();
    let registry = app_registry.read();
    let Some(reflect_component) = registry
        .get_with_type_path(type_path)
        .and_then(|r| r.data::<ReflectComponent>().cloned())
    else {
        return false;
    };
    drop(registry);
    let Ok(mut entity_mut) = world.get_entity_mut(entity) else {
        return false;
    };
    reflect_component.remove(&mut entity_mut);
    true
}

/// Write one generated field back through reflection.
///
/// Mirrors `renzora::core::reflection::set_reflected_field`: clone the
/// component, mutate the clone, then `apply` it. Applying a whole component
/// rather than poking the live one is what makes change detection fire, which
/// the renderer's two-way bindings and the undo recorder both rely on.
pub fn write_field(
    world: &mut World,
    entity: Entity,
    type_path: &str,
    path: &str,
    value: FieldValue,
) -> bool {
    let Some(app_registry) = world.get_resource::<AppTypeRegistry>() else {
        return false;
    };
    let app_registry = app_registry.clone();
    let registry = app_registry.read();
    let Some(registration) = registry.get_with_type_path(type_path) else {
        return false;
    };
    // Cloned, not borrowed: `ReflectComponent` is a bundle of fn pointers, and
    // holding a borrow of the registry would keep the `RwLock` read guard alive
    // across the `world.get_entity_mut` below — which needs `&mut World` while
    // the guard still borrows `world`.
    let Some(reflect_component) = registration.data::<ReflectComponent>().cloned() else {
        return false;
    };
    drop(registry);
    let Ok(entity_ref) = world.get_entity(entity) else {
        return false;
    };
    let Some(reflected) = reflect_component.reflect(entity_ref) else {
        return false;
    };
    let Ok(mut cloned) = reflected.reflect_clone() else {
        return false;
    };

    let applied = {
        let Ok(target) = cloned.reflect_path_mut(path) else {
            return false;
        };
        apply_value(target, value)
    };
    if !applied {
        return false;
    }

    let Ok(mut entity_mut) = world.get_entity_mut(entity) else {
        return false;
    };
    reflect_component.apply(&mut entity_mut, cloned.as_partial_reflect());
    true
}

/// Write a `FieldValue` into a reflected leaf, converting back to the leaf's
/// real Rust type. The widget wire types are lossy on purpose (every number is
/// an `f32`), so this is where the integer/float distinction is restored.
fn apply_value(target: &mut dyn bevy::reflect::PartialReflect, value: FieldValue) -> bool {
    fn set<T: bevy::reflect::PartialReflect>(
        target: &mut dyn bevy::reflect::PartialReflect,
        v: T,
    ) -> bool {
        if target.try_downcast_mut::<T>().is_some() {
            return target.try_apply(&v).is_ok();
        }
        false
    }

    match value {
        FieldValue::Float(f) => {
            // Try every numeric type the leaf might really be; only one matches.
            set(target, f)
                || set(target, f as f64)
                || set(target, f.round() as i32)
                || set(target, f.round() as i64)
                || set(target, f.max(0.0).round() as u32)
                || set(target, f.max(0.0).round() as u64)
                || set(target, f.max(0.0).round() as usize)
        }
        FieldValue::Bool(b) => set(target, b),
        FieldValue::String(s) => set(target, s),
        FieldValue::Vec3(a) => set(target, Vec3::new(a[0], a[1], a[2])),
        FieldValue::Color(rgb) => {
            set(target, Vec3::new(rgb[0], rgb[1], rgb[2]))
                || set(target, Color::srgb(rgb[0], rgb[1], rgb[2]))
        }
        FieldValue::ColorRgba(c) => {
            set(target, Color::srgba(c[0], c[1], c[2], c[3]))
                || set(target, Vec4::new(c[0], c[1], c[2], c[3]))
        }
        FieldValue::Enum(variant) => {
            // Unit variants only — `enum_field` refuses to build a dropdown for
            // data-carrying enums precisely so this cannot drop a payload.
            let dynamic = DynamicEnum::new(variant, DynamicVariant::Unit);
            target.try_apply(&dynamic).is_ok()
        }
        // Read-only rows have no write path, and `Asset` is not generated.
        FieldValue::ReadOnly(_) | FieldValue::Asset(_) => false,
    }
}

// ── resources ────────────────────────────────────────────────────────────
//
// Everything above is written for a component on an entity, and a resource
// reuses all of it unchanged. That is not a coincidence: Bevy 0.19 made
// `Resource: Component`, so a resource's value now lives as a component on a
// hidden entity that `World::resource_entities` maps its `ComponentId` to, and
// `#[reflect(Resource)]` registers `ReflectComponent` alongside the
// `ReflectResource` marker (which in 0.19 carries no functions of its own).
// Hand [`read_field`] / [`write_field`] the entity from that map and they read
// and write the resource — there is no resource-specific twin to keep in sync.

/// A reflected resource that currently exists in the world.
pub struct ResourceEntry {
    /// The entity holding this resource's value — the handle [`read_field`] and
    /// [`write_field`] take.
    pub entity: Entity,
    /// The resource's own `ComponentId`, for declaring a reactive dependency.
    pub cid: ComponentId,
    /// Full Rust type path, from the type registry.
    pub type_path: &'static str,
}

/// What [`world_resources`] found.
pub struct WorldResources {
    /// Every resource this build can name and read, unsorted and unfiltered.
    pub reflected: Vec<ResourceEntry>,
    /// How many resources exist that are **not** reflected, so a caller can say
    /// so rather than quietly presenting a partial list as the whole picture.
    ///
    /// Counted rather than listed because there is nothing to list: naming a
    /// component without going through the type registry means
    /// `ComponentInfo::name()`, which returns the literal string
    /// `"<Enable the debug feature to see the name>"` unless Bevy's `debug`
    /// feature is on — and this workspace does not enable it. Rows for these
    /// would be indistinguishable from each other and openable to nothing.
    pub unreflected: usize,
}

/// Every resource present in the world.
///
/// Cheap by design: it resolves each resource's type but never walks its
/// fields, so a caller can run it periodically to notice the set changing and
/// only pay for the rows it is actually about to draw.
pub fn world_resources(world: &World) -> WorldResources {
    let app_registry = world.get_resource::<AppTypeRegistry>().cloned();
    let registry = app_registry.as_ref().map(|r| r.read());

    let mut reflected = Vec::new();
    let mut unreflected = 0usize;
    for (cid, entity) in world.resource_entities().iter() {
        let Some(info) = world.components().get_info(cid) else {
            continue;
        };
        // Reflectability is `ReflectComponent`, not `ReflectResource`: the
        // latter is a bare marker in 0.19 and cannot read anything.
        let type_path = info
            .type_id()
            .and_then(|tid| registry.as_ref().and_then(|r| r.get(tid)))
            .filter(|reg| reg.data::<ReflectComponent>().is_some())
            .map(|reg| reg.type_info().type_path());
        match type_path {
            Some(type_path) => reflected.push(ResourceEntry {
                entity,
                cid,
                type_path,
            }),
            None => unreflected += 1,
        }
    }
    WorldResources {
        reflected,
        unreflected,
    }
}

/// The rows for one resource — the resource counterpart of a
/// [`ReflectSection`]'s fields.
///
/// A resource is often a newtype or a bare enum rather than a named struct, so
/// this also handles the case where the resource *itself* is the value: those
/// get one row addressed by the empty path, which `GetPath` resolves to the
/// root.
pub fn resource_fields(world: &World, entity: Entity, type_path: &str) -> Vec<ReflectField> {
    let Some(app_registry) = world.get_resource::<AppTypeRegistry>().cloned() else {
        return Vec::new();
    };
    let registry = app_registry.read();
    let Some(registration) = registry.get_with_type_path(type_path) else {
        return Vec::new();
    };
    let Some(reflect_component) = registration.data::<ReflectComponent>() else {
        return Vec::new();
    };
    let Ok(entity_ref) = world.get_entity(entity) else {
        return Vec::new();
    };
    let Some(reflected) = reflect_component.reflect(entity_ref) else {
        return Vec::new();
    };
    let value = reflected.as_partial_reflect();

    let mut fields = Vec::new();
    walk(value, "", 0, &registry, &mut fields);
    if !fields.is_empty() {
        return fields;
    }

    // Not a struct-shaped resource: `enum GameMode` and `struct Paused(bool)`
    // are both perfectly ordinary resources, and both walk to nothing.
    if let Some((field_type, val)) = leaf_to_field(value, "value", None)
        .or_else(|| enum_field(value, &registry))
    {
        fields.push(ReflectField {
            label: "Value",
            path: "",
            field_type,
            value: val,
        });
    }
    fields
}
