//! Data-driven inspector registry — components register fields declaratively.

use bevy::prelude::*;

/// A value that can be read from or written to a component field.
#[derive(Debug, Clone)]
pub enum FieldValue {
    Float(f32),
    Vec3([f32; 3]),
    Bool(bool),
    Color([f32; 3]),
    /// RGBA color (straight / unmultiplied) — editable alpha.
    ColorRgba([f32; 4]),
    String(String),
    ReadOnly(String),
    /// Asset path (project-relative).
    Asset(Option<String>),
    /// One of a fixed set of string variants (rendered as a dropdown). The
    /// allowed set is on the paired [`FieldType::Enum`]. Used for things like
    /// `FlexDirection` ("row"/"column"/...), `PositionType`, etc., which
    /// don't fit a numeric / boolean / color widget but have a small,
    /// well-known option list.
    Enum(String),
}

impl FieldValue {
    /// A type-appropriate zero / empty default, used by the inspector's
    /// "reset to default" action when a field doesn't provide its own default.
    pub fn type_default(&self) -> FieldValue {
        match self {
            FieldValue::Float(_) => FieldValue::Float(0.0),
            FieldValue::Vec3(_) => FieldValue::Vec3([0.0; 3]),
            FieldValue::Bool(_) => FieldValue::Bool(false),
            FieldValue::Color(_) => FieldValue::Color([1.0; 3]),
            FieldValue::ColorRgba(_) => FieldValue::ColorRgba([1.0; 4]),
            FieldValue::String(_) => FieldValue::String(String::new()),
            FieldValue::ReadOnly(s) => FieldValue::ReadOnly(s.clone()),
            FieldValue::Asset(_) => FieldValue::Asset(None),
            // "Reset to default" on an enum lands at the empty string — the
            // FieldDef's own `get_fn` will rewrite to the first option on the
            // next read.
            FieldValue::Enum(_) => FieldValue::Enum(String::new()),
        }
    }
}

/// Metadata about a field's type, used to select the correct widget.
#[derive(Debug, Clone)]
pub enum FieldType {
    Float {
        speed: f32,
        min: f32,
        max: f32,
    },
    Vec3 {
        speed: f32,
    },
    Bool,
    Color,
    /// RGBA color with an editable alpha channel.
    ColorRgba,
    String,
    ReadOnly,
    /// Asset path field — accepts drag-drop from asset browser.
    /// `extensions` filters which file types are accepted (e.g. `&["png", "jpg"]`).
    /// Empty slice = accept all.
    Asset {
        extensions: Vec<std::string::String>,
    },
    /// Dropdown choice from a fixed list of variant labels (e.g.
    /// `["row", "column", "row_reverse", "column_reverse"]`). The `set_fn`
    /// receives a `FieldValue::Enum(String)` carrying the selected label.
    Enum {
        options: &'static [&'static str],
    },
    /// A full-width action button. The `FieldDef::name` is the button label and
    /// `icon` is a leading Phosphor glyph. There's no value to read — `get_fn`
    /// should return `None`; clicking invokes `set_fn` with
    /// `FieldValue::Bool(true)` as a "pressed" signal.
    Button {
        icon: &'static str,
    },
}

/// A single inspectable field on a component.
pub struct FieldDef {
    pub name: &'static str,
    pub field_type: FieldType,
    pub get_fn: fn(&World, Entity) -> Option<FieldValue>,
    pub set_fn: fn(&mut World, Entity, FieldValue),
}

/// Registration entry for one component type.
///
pub struct InspectorEntry {
    pub type_id: &'static str,
    pub display_name: &'static str,
    pub icon: &'static str,
    pub category: &'static str,
    pub has_fn: fn(&World, Entity) -> bool,
    /// Optional function to add this component to an entity (for "Add Component" overlay).
    /// If `None`, the component won't appear in the Add Component overlay.
    pub add_fn: Option<fn(&mut World, Entity)>,
    /// Optional function to remove this component from an entity (trash button).
    /// If `None`, the component section won't show toggle/remove controls.
    pub remove_fn: Option<fn(&mut World, Entity)>,
    /// Check if the component is enabled (for toggle switch display).
    pub is_enabled_fn: Option<fn(&World, Entity) -> bool>,
    /// Set the component's enabled state (called on toggle switch click).
    pub set_enabled_fn: Option<fn(&mut World, Entity, bool)>,
    pub fields: Vec<FieldDef>,
}

/// Registry holding all inspector entries, keyed by component type_id.
#[derive(Resource, Default)]
pub struct InspectorRegistry {
    entries: Vec<InspectorEntry>,
}

impl InspectorRegistry {
    /// Register an inspector entry for a component.
    ///
    /// Ordering: `name` is always first, `transform` second, `material_ref`
    /// third; everything else is appended in registration order.
    pub fn register(&mut self, entry: InspectorEntry) {
        match entry.type_id {
            "name" => self.entries.insert(0, entry),
            "transform" => {
                // Insert after any existing "name" entry.
                let pos = self
                    .entries
                    .iter()
                    .position(|e| e.type_id != "name")
                    .unwrap_or(self.entries.len());
                self.entries.insert(pos, entry);
            }
            "material_ref" => {
                // Insert after name + transform, before everything else.
                let pos = self
                    .entries
                    .iter()
                    .position(|e| e.type_id != "name" && e.type_id != "transform")
                    .unwrap_or(self.entries.len());
                self.entries.insert(pos, entry);
            }
            _ => self.entries.push(entry),
        }
    }

    /// Iterate over all registered entries.
    pub fn iter(&self) -> impl Iterator<Item = &InspectorEntry> {
        self.entries.iter()
    }
}

/// Declare a native-renderable `f32` [`FieldDef`] for a component field without
/// hand-writing the get/set fn-pointers. The common case for effect/settings
/// inspectors whose UI is declarative `fields` (rendered by the bevy_ui
/// inspector).
///
/// ```ignore
/// fields: vec![
///     renzora_editor_framework::float_field!("Speed", MySettings, speed, 0.1, 0.0, 10.0),
/// ],
/// ```
#[macro_export]
macro_rules! float_field {
    ($name:expr, $comp:ty, $field:ident, $speed:expr, $min:expr, $max:expr $(,)?) => {
        $crate::FieldDef {
            name: $name,
            field_type: $crate::FieldType::Float {
                speed: $speed,
                min: $min,
                max: $max,
            },
            get_fn: |w, e| w.get::<$comp>(e).map(|comp| $crate::FieldValue::Float(comp.$field)),
            set_fn: |w, e, v| {
                if let ($crate::FieldValue::Float(f), Some(mut comp)) = (v, w.get_mut::<$comp>(e)) {
                    comp.$field = f;
                }
            },
        }
    };
}

/// Like [`float_field!`] for a `bool` component field.
#[macro_export]
macro_rules! bool_field {
    ($name:expr, $comp:ty, $field:ident $(,)?) => {
        $crate::FieldDef {
            name: $name,
            field_type: $crate::FieldType::Bool,
            get_fn: |w, e| w.get::<$comp>(e).map(|comp| $crate::FieldValue::Bool(comp.$field)),
            set_fn: |w, e, v| {
                if let ($crate::FieldValue::Bool(b), Some(mut comp)) = (v, w.get_mut::<$comp>(e)) {
                    comp.$field = b;
                }
            },
        }
    };
}

/// Like [`bool_field!`] for a `String` component field (single-line text input).
#[macro_export]
macro_rules! string_field {
    ($name:expr, $comp:ty, $field:ident $(,)?) => {
        $crate::FieldDef {
            name: $name,
            field_type: $crate::FieldType::String,
            get_fn: |w, e| {
                w.get::<$comp>(e).map(|comp| $crate::FieldValue::String(comp.$field.clone()))
            },
            set_fn: |w, e, v| {
                if let ($crate::FieldValue::String(s), Some(mut comp)) = (v, w.get_mut::<$comp>(e)) {
                    comp.$field = s;
                }
            },
        }
    };
}

/// Like [`bool_field!`] for a `bevy::prelude::Color` component field, rendered as
/// an RGBA color editor with editable alpha (straight / unmultiplied sRGBA — the
/// same space as the egui `color_edit_button_rgba_unmultiplied`).
#[macro_export]
macro_rules! color_rgba_field {
    ($name:expr, $comp:ty, $field:ident $(,)?) => {
        $crate::FieldDef {
            name: $name,
            field_type: $crate::FieldType::ColorRgba,
            get_fn: |w, e| {
                w.get::<$comp>(e)
                    .map(|comp| $crate::FieldValue::ColorRgba(comp.$field.to_srgba().to_f32_array()))
            },
            set_fn: |w, e, v| {
                if let ($crate::FieldValue::ColorRgba(a), Some(mut comp)) = (v, w.get_mut::<$comp>(e)) {
                    comp.$field = bevy::prelude::Color::srgba(a[0], a[1], a[2], a[3]);
                }
            },
        }
    };
}

/// Declare an [`FieldDef`] for a `u32` index-style enum field rendered as a
/// dropdown of `labels` (index 0 = first label). For settings that store an
/// enum mode as a plain `u32` (atmosphere/dof/...).
///
/// ```ignore
/// renzora_editor_framework::enum_u32_field!("Mode", MySettings, mode, ["Gaussian", "Bokeh"]),
/// ```
#[macro_export]
macro_rules! enum_u32_field {
    ($name:expr, $comp:ty, $field:ident, [$($label:expr),+ $(,)?]) => {
        $crate::FieldDef {
            name: $name,
            field_type: $crate::FieldType::Enum { options: &[$($label),+] },
            get_fn: |w, e| {
                w.get::<$comp>(e).map(|comp| {
                    let opts = [$($label),+];
                    let i = (comp.$field as usize).min(opts.len().saturating_sub(1));
                    $crate::FieldValue::Enum(opts[i].to_string())
                })
            },
            set_fn: |w, e, v| {
                if let ($crate::FieldValue::Enum(label), Some(mut comp)) = (v, w.get_mut::<$comp>(e)) {
                    let opts = [$($label),+];
                    if let Some(i) = opts.iter().position(|l| *l == label) {
                        comp.$field = i as u32;
                    }
                }
            },
        }
    };
}

/// A native (bevy_ui) inspector body builder for a component, for inspectors that
/// need more than declarative `fields`. Reads state from `&mut World` (the component, `EmberFonts`,
/// theme) and builds + binds an arbitrary bevy_ui subtree, returning its root
/// entity; the inspector parents it under the component's section header.
///
/// Build with a local `CommandQueue` (so you can use ember widgets + `bind_2way`):
/// ```ignore
/// fn my_inspector(world: &mut World, entity: Entity) -> Entity {
///     let fonts = world.resource::<EmberFonts>().clone();
///     let mut q = bevy::ecs::world::CommandQueue::default();
///     let root;
///     { let mut c = bevy::prelude::Commands::new(&mut q, world); root = /* build */; }
///     q.apply(world);
///     root
/// }
/// ```
pub type NativeInspectorDrawer = fn(&mut World, Entity) -> Entity;

/// Maps a component `type_id` to its native (bevy_ui) inspector drawer. Lets a
/// plugin provide custom bevy_ui inspector UI beyond declarative `fields`.
/// Registered via `App::register_native_inspector_ui`.
#[derive(Resource, Default)]
pub struct NativeInspectorRegistry {
    drawers: std::collections::HashMap<&'static str, NativeInspectorDrawer>,
}

impl NativeInspectorRegistry {
    pub fn register(&mut self, type_id: &'static str, drawer: NativeInspectorDrawer) {
        self.drawers.insert(type_id, drawer);
    }

    pub fn get(&self, type_id: &str) -> Option<NativeInspectorDrawer> {
        self.drawers.get(type_id).copied()
    }
}

/// A `Color` [`FieldDef`] for a `Vec3`-stored RGB component field.
#[macro_export]
macro_rules! vec3_color_field {
    ($name:expr, $comp:ty, $field:ident $(,)?) => {
        $crate::FieldDef {
            name: $name,
            field_type: $crate::FieldType::Color,
            get_fn: |w, e| {
                w.get::<$comp>(e)
                    .map(|c| $crate::FieldValue::Color([c.$field.x, c.$field.y, c.$field.z]))
            },
            set_fn: |w, e, v| {
                if let ($crate::FieldValue::Color(rgb), Some(mut c)) = (v, w.get_mut::<$comp>(e)) {
                    c.$field.x = rgb[0];
                    c.$field.y = rgb[1];
                    c.$field.z = rgb[2];
                }
            },
        }
    };
}

/// A `Color` [`FieldDef`] for a `(f32, f32, f32)`-stored RGB component field.
#[macro_export]
macro_rules! tuple_color_field {
    ($name:expr, $comp:ty, $field:ident $(,)?) => {
        $crate::FieldDef {
            name: $name,
            field_type: $crate::FieldType::Color,
            get_fn: |w, e| {
                w.get::<$comp>(e)
                    .map(|c| $crate::FieldValue::Color([c.$field.0, c.$field.1, c.$field.2]))
            },
            set_fn: |w, e, v| {
                if let ($crate::FieldValue::Color(rgb), Some(mut c)) = (v, w.get_mut::<$comp>(e)) {
                    c.$field = (rgb[0], rgb[1], rgb[2]);
                }
            },
        }
    };
}

/// A [`FieldDef`] for an integer field (`$ty`, e.g. `u32`/`i32`) rendered as a
/// `Float` drag and cast back on write.
#[macro_export]
macro_rules! int_field {
    ($name:expr, $comp:ty, $field:ident, $ty:ty, $speed:expr, $min:expr, $max:expr $(,)?) => {
        $crate::FieldDef {
            name: $name,
            field_type: $crate::FieldType::Float {
                speed: $speed,
                min: $min,
                max: $max,
            },
            get_fn: |w, e| w.get::<$comp>(e).map(|c| $crate::FieldValue::Float(c.$field as f32)),
            set_fn: |w, e, v| {
                if let ($crate::FieldValue::Float(f), Some(mut c)) = (v, w.get_mut::<$comp>(e)) {
                    c.$field = f as $ty;
                }
            },
        }
    };
}
