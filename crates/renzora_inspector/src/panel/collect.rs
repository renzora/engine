//! Reading the registry — and reflection — into [`SectionSpec`]s.
//!
//! Two sources, deliberately. The hand-written `InspectorRegistry` is what a
//! crate registers to get a curated section; `append_reflected_sections` fills
//! the gaps from `bevy_reflect` so a component crate can carry no editor
//! dependency at all and still be inspectable.

use bevy::prelude::*;

use renzora_editor_framework::{
    EditorSettings, FieldType, FieldValue, InspectorRegistry, NativeInspectorRegistry,
};
use renzora_ember::reactive::Rx;
use renzora_theme::ThemeManager;

use super::spec::{
    category_rgb, component_id_for, format_value, section_priority, FieldInit, FieldKind,
    FieldSpec, SectionSpec,
};
use super::{policy_open, InspectorSectionsOpen, InspectorState, Mutate, Pred, SetEnabled};

pub(crate) fn collect_sections(world: &Rx, entity: Option<Entity>) -> Vec<SectionSpec> {
    let Some(entity) = entity else {
        return Vec::new();
    };
    let Some(reg) = world.get_resource::<InspectorRegistry>() else {
        return Vec::new();
    };
    let theme = world.get_resource::<ThemeManager>();
    let native_reg = world.get_resource::<NativeInspectorRegistry>();
    let filter = world
        .get_resource::<InspectorState>()
        .map(|s| s.filter.clone())
        .unwrap_or_default();

    // Initial expand state per section, from the user's `inspector_expand_default`
    // policy (Essentials keeps only Name/Transform/Scripts open). After build the
    // expand/collapse-all button drives sections live (see `expand_all_click`).
    let expand_policy = world
        .get_resource::<EditorSettings>()
        .map(|s| s.inspector_expand_default)
        .unwrap_or_default();
    // Remembered per-type collapse state wins over the policy; the policy is
    // re-asserted (and this map cleared) by `apply_expand_policy_change` whenever
    // the setting itself changes, so it can never become unreachable.
    let remembered = world.get_resource::<InspectorSectionsOpen>();
    let section_open = |type_id: &'static str| -> bool {
        if let Some(&open) = remembered.and_then(|m| m.0.get(type_id)) {
            return open;
        }
        policy_open(expand_policy, type_id)
    };

    let mut out = Vec::new();
    for entry in reg.iter() {
        if !(entry.has_fn)(world.untracked(), entity) {
            continue;
        }
        // Component-name filter (case-insensitive substring on the display name).
        if !filter.is_empty() && !entry.display_name.to_lowercase().contains(&filter) {
            continue;
        }
        let (accent, header_bg) = theme
            .map(|tm| category_rgb(&tm.active_theme, entry.category))
            .unwrap_or(((120, 140, 200), (44, 44, 54)));
        let enable = match (entry.is_enabled_fn, entry.set_enabled_fn) {
            (Some(g), Some(s)) => Some((std::sync::Arc::new(g) as Pred, std::sync::Arc::new(s) as SetEnabled)),
            _ => None,
        };
        let enabled_now = enable.as_ref().map(|(g, _)| g(world.untracked(), entity)).unwrap_or(true);
        // Priority: a registered native bevy_ui drawer > declarative `fields` >
        // placeholder note (component has neither a native drawer nor any fields).
        let native_drawer = native_reg.and_then(|r| r.get(entry.type_id));
        if native_drawer.is_some() {
            out.push(SectionSpec {
                title: entry.display_name,
                cid: component_id_for(world.untracked(), entry.type_id),
                icon: entry.icon,
                type_id: entry.type_id,
                custom: false,
                native_drawer,
                remove_fn: entry.remove_fn.map(|f| std::sync::Arc::new(f) as Mutate),
                enable: enable.clone(),
                enabled_now,
                header_bg,
                accent,
                open: section_open(entry.type_id),
                fields: Vec::new(),
            });
            continue;
        }
        if entry.fields.is_empty() {
            out.push(SectionSpec {
                title: entry.display_name,
                cid: component_id_for(world.untracked(), entry.type_id),
                icon: entry.icon,
                type_id: entry.type_id,
                custom: true,
                native_drawer: None,
                remove_fn: entry.remove_fn.map(|f| std::sync::Arc::new(f) as Mutate),
                enable: enable.clone(),
                enabled_now,
                header_bg,
                accent,
                open: section_open(entry.type_id),
                fields: Vec::new(),
            });
            continue;
        }
        let mut fields = Vec::new();
        for f in &entry.fields {
            let val = (f.get_fn)(world.untracked(), entity);
            // A `None` read means "row not applicable right now" — the section's
            // component is toggled off, or the field only applies to some
            // states (e.g. occluder Width/Height on a polygon shape). Hide the
            // row rather than falling through to a junk ReadOnly. Buttons are
            // the exception: they have no value to read by design.
            if val.is_none() && !matches!(f.field_type, FieldType::Button { .. }) {
                continue;
            }
            let (kind, init) = match (&f.field_type, &val) {
                (FieldType::Float { speed, min, max }, Some(FieldValue::Float(v))) => (
                    FieldKind::Float {
                        speed: *speed,
                        min: *min,
                        max: *max,
                    },
                    FieldInit::Float(*v),
                ),
                (FieldType::Int { min, max }, Some(FieldValue::Float(v))) => (
                    FieldKind::Int { min: *min, max: *max },
                    FieldInit::Float(*v),
                ),
                (FieldType::Vec3 { speed }, Some(FieldValue::Vec3(a))) => {
                    (FieldKind::Vec3 { speed: *speed }, FieldInit::Vec3(*a))
                }
                (FieldType::Bool, Some(FieldValue::Bool(b))) => {
                    (FieldKind::Bool, FieldInit::Bool(*b))
                }
                (FieldType::Color, Some(FieldValue::Color(_))) => {
                    // color_field seeds itself from the live value; no init needed.
                    (FieldKind::Color, FieldInit::Text(String::new()))
                }
                (FieldType::ColorRgba, Some(FieldValue::ColorRgba(_))) => {
                    (FieldKind::ColorRgba, FieldInit::Text(String::new()))
                }
                (FieldType::String, Some(FieldValue::String(s))) => {
                    (FieldKind::Text, FieldInit::Text(s.clone()))
                }
                (FieldType::Enum { options }, Some(FieldValue::Enum(s))) => {
                    (FieldKind::Enum { options }, FieldInit::Text(s.clone()))
                }
                // Options are computed from the world here (mapping has `world`);
                // stored in the init so `FieldKind` stays `Copy`.
                (FieldType::DynamicEnum { options }, Some(FieldValue::Float(v))) => (
                    FieldKind::DynamicEnum,
                    FieldInit::DynEnum(options(world.untracked(), entity), v.round().max(0.0) as usize),
                ),
                (FieldType::Asset { .. }, Some(FieldValue::Asset(_)))
                | (FieldType::AssetCreatable { .. }, Some(FieldValue::Asset(_))) => {
                    (FieldKind::Asset, FieldInit::Text(String::new()))
                }
                // Buttons have no value to read — match regardless of `val`.
                (FieldType::Button { icon }, _) => {
                    (FieldKind::Button { icon }, FieldInit::Text(String::new()))
                }
                _ => (FieldKind::ReadOnly, FieldInit::Text(format_value(val.as_ref()))),
            };
            let extensions = match &f.field_type {
                FieldType::Asset { extensions }
                | FieldType::AssetCreatable { extensions, .. } => extensions.clone(),
                _ => Vec::new(),
            };
            let create_fn = match &f.field_type {
                FieldType::AssetCreatable { create_fn, .. } => Some(std::sync::Arc::new(*create_fn) as Mutate),
                _ => None,
            };
            fields.push(FieldSpec {
                name: f.name,
                kind,
                // A hand-written `FieldDef` still supplies plain fn pointers;
                // they coerce into the boxed accessor here, at the one seam.
                get_fn: std::sync::Arc::new(f.get_fn),
                set_fn: std::sync::Arc::new(f.set_fn),
                init,
                extensions,
                create_fn: create_fn.clone(),
                cid: component_id_for(world.untracked(), entry.type_id),
            });
        }
        out.push(SectionSpec {
            title: entry.display_name,
            cid: component_id_for(world.untracked(), entry.type_id),
            icon: entry.icon,
            type_id: entry.type_id,
            custom: false,
            native_drawer: None,
            remove_fn: entry.remove_fn.map(|f| std::sync::Arc::new(f) as Mutate),
            enable: enable.clone(),
            enabled_now,
            header_bg,
            accent,
            open: section_open(entry.type_id),
            fields,
        });
    }

    append_reflected_sections(world, entity, reg, &mut out);

    // Pin the most-edited components to the top in a fixed order — Name,
    // Transform, then Scripts, then Material — so they're always right where you
    // expect regardless of plugin registration order. A stable sort keeps every
    // other component in its original registry order behind them.
    out.sort_by_key(|s| section_priority(s.title));
    out
}

/// Append sections generated from `bevy_reflect` for components the hand-written
/// [`InspectorRegistry`] does not cover (or, in `All` mode, for every reflected
/// component, so a generated section can be compared side by side against the
/// hand-written one for the same type).
///
/// This is the whole point of [`crate::reflect_source`]: the rows below are
/// produced without any component naming an inspector type, which is what lets a
/// component crate carry no editor dependency at all.
fn append_reflected_sections(
    world: &Rx,
    entity: Entity,
    reg: &InspectorRegistry,
    out: &mut Vec<SectionSpec>,
) {
    let mode = world
        .get_resource::<crate::reflect_source::ReflectInspectorMode>()
        .copied()
        .unwrap_or_default();
    if mode == crate::reflect_source::ReflectInspectorMode::Off {
        return;
    }

    // The hand-written registry keys on a slug (`"transform"`), reflection on a
    // Rust type name (`Transform`). Match on both the slug and the display name
    // with separators removed, which is as close as the two vocabularies get —
    // an over-match only means a component keeps its hand-written section, which
    // is the safe direction.
    let mut covered: std::collections::HashSet<String> = std::collections::HashSet::new();
    if mode == crate::reflect_source::ReflectInspectorMode::FillGaps {
        for entry in reg.iter() {
            for key in [entry.type_id, entry.display_name] {
                let k = key.to_ascii_lowercase().replace([' ', '_', '-'], "");
                // Register the singular too: entries are named for the panel
                // ("Scripts") while the type is singular (`ScriptComponent`).
                covered.insert(k.trim_end_matches('s').to_string());
                covered.insert(k);
            }
        }
    }

    let generated = crate::reflect_source::reflect_sections(world.untracked(), entity, &|short| {
        // Reflected type names carry noise words the curated names never do —
        // `AtmosphereComponentSettings` is the `Atmosphere` entry, `CloudsData`
        // is `Clouds`. Strip those before comparing, or every settings component
        // gets a duplicate generated section next to its hand-written one.
        let bare = short.replace('_', "");
        let mut stem = bare.as_str();
        for suffix in ["componentsettings", "component", "settings", "config", "data"] {
            stem = stem.strip_suffix(suffix).unwrap_or(stem);
        }
        covered.contains(&bare)
            || covered.contains(stem)
            || covered.contains(stem.trim_end_matches('s'))
    });

    for section in generated {
        let type_path = section.type_path;
        // Generic equivalents of a hand-written entry's `remove_fn` and
        // `is_enabled_fn`/`set_enabled_fn`, both parameterised by the type path —
        // which is exactly why these had to stop being bare fn pointers.
        let remove_fn: Option<Mutate> = Some(std::sync::Arc::new(
            move |w: &mut World, e: Entity| {
                crate::reflect_source::remove_component(w, e, type_path);
            },
        ));
        let enable: Option<(Pred, SetEnabled)> = section.has_enabled.then(|| {
            let pred: Pred = std::sync::Arc::new(move |w: &World, e: Entity| {
                matches!(
                    crate::reflect_source::read_field(w, e, type_path, "enabled", false),
                    Some(FieldValue::Bool(true))
                )
            });
            let set: SetEnabled = std::sync::Arc::new(move |w: &mut World, e: Entity, v: bool| {
                crate::reflect_source::write_field(w, e, type_path, "enabled", FieldValue::Bool(v));
            });
            (pred, set)
        });
        let enabled_now = enable.as_ref().map(|(g, _)| g(world.untracked(), entity)).unwrap_or(true);
        let mut fields = Vec::new();
        for f in section.fields {
            let (kind, init) = match (&f.field_type, &f.value) {
                (FieldType::Float { speed, min, max }, FieldValue::Float(v)) => (
                    FieldKind::Float { speed: *speed, min: *min, max: *max },
                    FieldInit::Float(*v),
                ),
                (FieldType::Int { min, max }, FieldValue::Float(v)) => {
                    (FieldKind::Int { min: *min, max: *max }, FieldInit::Float(*v))
                }
                (FieldType::Vec3 { speed }, FieldValue::Vec3(a)) => {
                    (FieldKind::Vec3 { speed: *speed }, FieldInit::Vec3(*a))
                }
                (FieldType::Bool, FieldValue::Bool(b)) => (FieldKind::Bool, FieldInit::Bool(*b)),
                // The colour widgets seed themselves from the live value.
                (FieldType::Color, FieldValue::Color(_)) => {
                    (FieldKind::Color, FieldInit::Text(String::new()))
                }
                (FieldType::ColorRgba, FieldValue::ColorRgba(_)) => {
                    (FieldKind::ColorRgba, FieldInit::Text(String::new()))
                }
                (FieldType::String, FieldValue::String(s)) => {
                    (FieldKind::Text, FieldInit::Text(s.clone()))
                }
                (FieldType::Enum { options }, FieldValue::Enum(s)) => {
                    (FieldKind::Enum { options }, FieldInit::Text(s.clone()))
                }
                _ => (
                    FieldKind::ReadOnly,
                    FieldInit::Text(format_value(Some(&f.value))),
                ),
            };
            let read_only = matches!(kind, FieldKind::ReadOnly);
            let (path, get_path) = (f.path, f.path);
            fields.push(FieldSpec {
                name: f.label,
                kind,
                get_fn: std::sync::Arc::new(move |w: &World, e: Entity| {
                    crate::reflect_source::read_field(w, e, type_path, get_path, read_only)
                }),
                set_fn: std::sync::Arc::new(move |w: &mut World, e: Entity, v: FieldValue| {
                    crate::reflect_source::write_field(w, e, type_path, path, v);
                }),
                init,
                extensions: Vec::new(),
                create_fn: None,
                cid: component_id_for(world.untracked(), type_path),
            });
        }
        out.push(SectionSpec {
            title: section.short_name,
            cid: component_id_for(world.untracked(), type_path),
            icon: "cube",
            type_id: type_path,
            custom: false,
            native_drawer: None,
            remove_fn,
            enable,
            enabled_now,
            header_bg: (44, 44, 54),
            accent: (150, 130, 200),
            // Closed by default: in `All` mode every component gains a second
            // section, and opening them all would bury the hand-written ones.
            open: false,
            fields,
        });
    }
}
