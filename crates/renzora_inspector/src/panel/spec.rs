//! What a section and a field are, described once under the exclusive borrow and
//! then handed to the builders.
//!
//! Nothing here touches the UI. [`collect`](super::collect) fills these in from
//! the registry, [`section`](super::section) and [`fields`](super::fields) turn
//! them into widgets.

use bevy::ecs::component::ComponentId;
use bevy::prelude::*;

use renzora_editor_framework::{FieldValue, NativeInspectorDrawer};
use renzora_ember::reactive::Rx;

use super::{GetFn, Mutate, Pred, SetEnabled, SetFn};

#[derive(Clone, Copy)]
pub(crate) enum FieldKind {
    Float { speed: f32, min: f32, max: f32 },
    /// Whole-number drag field: the widget's model snaps to integers
    /// (`DragSnap`), matching a `set_fn` that rounds into an int component
    /// field — see `FieldType::Int`.
    Int { min: f32, max: f32 },
    Vec3 { speed: f32 },
    Bool,
    Color,
    ColorRgba,
    Text,
    Asset,
    Enum { options: &'static [&'static str] },
    /// Dynamic dropdown; options + selected index live in [`FieldInit::DynEnum`]
    /// (so this stays `Copy`). Value is the selected index (`FieldValue::Float`).
    DynamicEnum,
    Button { icon: &'static str },
    ReadOnly,
}

#[derive(Clone)]
pub(crate) enum FieldInit {
    Float(f32),
    Vec3([f32; 3]),
    Bool(bool),
    Text(String),
    /// Dynamic-dropdown options (computed from the world) + the selected index.
    DynEnum(Vec<String>, usize),
}

#[derive(Clone)]
pub(crate) struct FieldSpec {
    pub(crate) name: &'static str,
    pub(crate) kind: FieldKind,
    pub(crate) get_fn: GetFn,
    pub(crate) set_fn: SetFn,
    pub(crate) init: FieldInit,
    /// Accepted extensions for `Asset` fields (empty = accept any). Unused for
    /// other kinds.
    pub(crate) extensions: Vec<String>,
    /// `AssetCreatable` fields only: the "+" button's create-in-place action.
    pub(crate) create_fn: Option<Mutate>,
    /// The component this field reads, when it could be resolved from the
    /// section's type path.
    ///
    /// `get_fn` is a contract-crate `fn(&World, Entity)` and cannot take an
    /// `&Rx`, so the binding around it would otherwise have to give up on
    /// tracking entirely — which is what pinned most of the inspector dirty.
    /// Naming the component lets the binding *declare* the dependency instead:
    /// of 248 `get_fn` definitions in the workspace, 247 are literally
    /// `|w, e| w.get::<C>(e).map(..)` and the one exception ignores both
    /// arguments, so `(entity, component)` is what they read.
    ///
    /// `None` falls back to untracked — unchanged behaviour.
    pub(crate) cid: Option<ComponentId>,
}

/// Call a contract `fn(&World, Entity)` while still declaring what it reads.
///
/// The two halves are deliberately in one place: `manually_tracked` is the only
/// hatch where being wrong causes staleness rather than wasted work, so the
/// `track_component_id` that justifies it sits immediately above the call.
pub(crate) fn tracked_read<T>(
    rx: &Rx,
    entity: Entity,
    cid: Option<ComponentId>,
    f: impl FnOnce(&World) -> T,
) -> T {
    match cid {
        Some(cid) => {
            rx.track_component_id(entity, cid);
            f(rx.manually_tracked())
        }
        // Unknown component: stay conservative, exactly as before.
        None => f(rx.untracked()),
    }
}

/// Resolve a reflected type path (`"bevy_transform::components::Transform"`) to
/// the `ComponentId` the world knows it by, so a binding can depend on it.
///
/// `None` for anything not registered or not a component — the caller then
/// falls back to untracked, which is always safe.
pub(crate) fn component_id_for(world: &World, type_path: &str) -> Option<ComponentId> {
    let registry = world.get_resource::<AppTypeRegistry>()?.clone();
    let type_id = {
        let r = registry.read();
        r.get_with_type_path(type_path)?.type_id()
    };
    world.components().get_id(type_id)
}

pub(crate) struct SectionSpec {
    pub(crate) title: &'static str,
    /// The component this section is for, when resolvable — lets the enable
    /// toggle declare its dependency instead of pinning itself dirty.
    pub(crate) cid: Option<ComponentId>,
    pub(crate) icon: &'static str, // phosphor icon name (resolved via icon_glyph)
    pub(crate) type_id: &'static str,
    pub(crate) custom: bool,
    /// Native (bevy_ui) drawer, if the component registered one. Takes priority
    /// over declarative fields.
    pub(crate) native_drawer: Option<NativeInspectorDrawer>,
    pub(crate) remove_fn: Option<Mutate>,
    pub(crate) enable: Option<(Pred, SetEnabled)>,
    pub(crate) enabled_now: bool,
    /// Category-derived header background + accent (icon tint).
    pub(crate) header_bg: (u8, u8, u8),
    pub(crate) accent: (u8, u8, u8),
    /// Whether this section starts expanded (per the expand-default policy /
    /// expand-all override, computed in [`collect_sections`](super::collect::collect_sections)).
    pub(crate) open: bool,
    pub(crate) fields: Vec<FieldSpec>,
}

/// Extract an `(r, g, b)` triple from a theme color (no egui types in scope).
pub(crate) fn c32(col: renzora_theme::ThemeColor) -> (u8, u8, u8) {
    let [r, g, b, _] = col.to_array();
    (r, g, b)
}

/// Replicates `renzora_ui::category_colors`: maps a component category to its
/// themed (accent, header_bg). So lights get an amber header, environment a
/// blue-grey one, etc. — not all the same.
pub(crate) fn category_rgb(
    theme: &renzora_theme::Theme,
    category: &str,
) -> ((u8, u8, u8), (u8, u8, u8)) {
    let s = match category {
        "environment" => &theme.categories.environment,
        "light" | "lighting" => &theme.categories.lighting,
        "camera" => &theme.categories.camera,
        "script" | "scripting" => &theme.categories.scripting,
        "physics" => &theme.categories.physics,
        "plugin" => &theme.categories.plugin,
        "nodes2d" | "nodes_2d" => &theme.categories.nodes_2d,
        "ui" => &theme.categories.ui,
        "rendering" => &theme.categories.rendering,
        "effects" | "particles" => &theme.categories.effects,
        _ => &theme.categories.transform,
    };
    (c32(s.accent), c32(s.header_bg))
}

/// Display order weight for a section: pinned components come first in a fixed
/// order; everything else shares the same (higher) weight and so keeps its
/// registry order under the stable sort in
/// [`collect_sections`](super::collect::collect_sections).
pub(crate) fn section_priority(title: &str) -> u8 {
    match title {
        "Transform" => 0,
        "Scripts" => 1,
        "Material" => 2,
        _ => 3,
    }
}

/// Lowercase, collapsing each run of non-alphanumerics to one `_`, for deriving a
/// stable localization-key segment from a human label
/// ("Wind Direction" → `wind_direction`). The reflection-driven component and
/// field labels have no literal in source to translate, so we key off this.
fn loc_slug(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut prev_us = false;
    for c in s.trim().chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
            prev_us = false;
        } else if !prev_us {
            out.push('_');
            prev_us = true;
        }
    }
    out.trim_matches('_').to_string()
}

/// Localized component header name, falling back to the English `display_name`.
/// Keyed `comp.<slug>.name` (e.g. "Clouds" → `comp.clouds.name`).
pub(crate) fn comp_name_loc(display_name: &str) -> String {
    renzora::lang::t_or(&format!("comp.{}.name", loc_slug(display_name)), display_name)
}

/// Localized field label, falling back to the English `name`. Keyed in a SHARED
/// `field.<slug>` namespace (e.g. "Wind Direction" → `field.wind_direction`) so a
/// field name common to many components is translated once, not per component.
pub(crate) fn field_label_loc(name: &str) -> String {
    renzora::lang::t_or(&format!("field.{}", loc_slug(name)), name)
}

pub(crate) fn format_value(v: Option<&FieldValue>) -> String {
    match v {
        Some(FieldValue::Float(f)) => format!("{f:.3}"),
        Some(FieldValue::Vec3(a)) => format!("{:.3}, {:.3}, {:.3}", a[0], a[1], a[2]),
        Some(FieldValue::Bool(b)) => b.to_string(),
        Some(FieldValue::Color(col)) => format!(
            "#{:02X}{:02X}{:02X}",
            (col[0] * 255.0) as u8,
            (col[1] * 255.0) as u8,
            (col[2] * 255.0) as u8
        ),
        Some(FieldValue::ColorRgba(col)) => format!(
            "#{:02X}{:02X}{:02X}{:02X}",
            (col[0] * 255.0) as u8,
            (col[1] * 255.0) as u8,
            (col[2] * 255.0) as u8,
            (col[3] * 255.0) as u8
        ),
        Some(FieldValue::String(s)) | Some(FieldValue::ReadOnly(s)) | Some(FieldValue::Enum(s)) => {
            s.clone()
        }
        Some(FieldValue::Asset(a)) => a.clone().unwrap_or_else(|| "—".into()),
        None => "—".into(),
    }
}
