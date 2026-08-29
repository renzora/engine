//! The Clouds inspector.
//!
//! This was a separate `Editor`-scope crate so the lean runtime half carried no
//! editor contract. As one native plugin it is a module again: `Runtime` scope
//! loads in the editor *and* a copy-based export, and a game pays a single `Vec`
//! push into a registry it never reads.
//!
//! It already imported the contract types from `renzora` rather than from
//! `renzora_editor_framework`, so nothing here had to change but the path to
//! `CloudsData` — which now lives in the contract crate too.

use bevy::prelude::*;
use renzora::{AppEditorExt, CloudsData, FieldDef, FieldType, FieldValue, InspectorEntry};

/// One scalar slider. The volumetric model has enough knobs that spelling every
/// getter and setter out by hand would triple the length of this file for no
/// added clarity.
macro_rules! float_field {
    ($name:literal, $field:ident, $speed:expr, $min:expr, $max:expr) => {
        FieldDef {
            name: $name,
            field_type: FieldType::Float {
                speed: $speed,
                min: $min,
                max: $max,
            },
            get_fn: |world, entity| {
                world
                    .get::<CloudsData>(entity)
                    .map(|d| FieldValue::Float(d.$field))
            },
            set_fn: |world, entity, val| {
                if let FieldValue::Float(v) = val {
                    if let Some(mut d) = world.get_mut::<CloudsData>(entity) {
                        d.$field = v;
                    }
                }
            },
        }
    };
}

/// Whole-number slider. `FieldType::Int` is required rather than a `Float` with
/// a rounding setter: the widget snaps its own model to integers, and without it
/// the fractional drag model and the rounded re-read fight mid-drag.
macro_rules! int_field {
    ($name:literal, $field:ident, $min:expr, $max:expr) => {
        FieldDef {
            name: $name,
            field_type: FieldType::Int {
                min: $min,
                max: $max,
            },
            get_fn: |world, entity| {
                world
                    .get::<CloudsData>(entity)
                    .map(|d| FieldValue::Float(d.$field as f32))
            },
            set_fn: |world, entity, val| {
                if let FieldValue::Float(v) = val {
                    if let Some(mut d) = world.get_mut::<CloudsData>(entity) {
                        d.$field = v.round().max(0.0) as u32;
                    }
                }
            },
        }
    };
}

macro_rules! bool_field {
    ($name:literal, $field:ident) => {
        FieldDef {
            name: $name,
            field_type: FieldType::Bool,
            get_fn: |world, entity| {
                world
                    .get::<CloudsData>(entity)
                    .map(|d| FieldValue::Bool(d.$field))
            },
            set_fn: |world, entity, val| {
                if let FieldValue::Bool(v) = val {
                    if let Some(mut d) = world.get_mut::<CloudsData>(entity) {
                        d.$field = v;
                    }
                }
            },
        }
    };
}

macro_rules! color_field {
    ($name:literal, $field:ident) => {
        FieldDef {
            name: $name,
            field_type: FieldType::Color,
            get_fn: |world, entity| {
                world
                    .get::<CloudsData>(entity)
                    .map(|d| FieldValue::Color([d.$field.0, d.$field.1, d.$field.2]))
            },
            set_fn: |world, entity, val| {
                if let FieldValue::Color([r, g, b]) = val {
                    if let Some(mut d) = world.get_mut::<CloudsData>(entity) {
                        d.$field = (r, g, b);
                    }
                }
            },
        }
    };
}

fn inspector_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "clouds",
        display_name: "Clouds",
        icon: "cloud-sun",
        category: "rendering",
        has_fn: |world, entity| world.get::<CloudsData>(entity).is_some(),
        add_fn: Some(|world, entity| {
            world.entity_mut(entity).insert(CloudsData::default());
        }),
        remove_fn: Some(|world, entity| {
            world.entity_mut(entity).remove::<CloudsData>();
        }),
        is_enabled_fn: Some(|world, entity| {
            world
                .get::<CloudsData>(entity)
                .map(|d| d.enabled)
                .unwrap_or(false)
        }),
        set_enabled_fn: Some(|world, entity, val| {
            if let Some(mut d) = world.get_mut::<CloudsData>(entity) {
                d.enabled = val;
            }
        }),
        fields: vec![
            // Shape
            float_field!("Coverage", coverage, 0.01, 0.0, 1.0),
            float_field!("Density", density, 0.01, 0.0, 1.0),
            float_field!("Scale", scale, 0.05, 0.1, 20.0),
            float_field!("Detail Scale", detail_scale, 0.5, 1.0, 200.0),
            float_field!("Detail Strength", detail_strength, 0.01, 0.0, 1.0),
            float_field!("Edge Softness", edge_softness, 0.005, 0.001, 1.0),
            float_field!("Base Softness", base_softness, 0.01, 0.001, 1.0),
            // Deck geometry
            float_field!("Bottom Height", bottom_height, 10.0, 0.0, 20000.0),
            float_field!("Top Height", top_height, 10.0, 1.0, 30000.0),
            float_field!("Planet Radius", planet_radius, 10000.0, 1000.0, 20000000.0),
            // Wind
            float_field!("Wind Speed", speed, 0.5, 0.0, 400.0),
            float_field!("Wind Direction", wind_direction, 1.0, 0.0, 360.0),
            float_field!("Morph Speed", morph_speed, 0.5, 0.0, 400.0),
            // Lighting
            color_field!("Color", color),
            float_field!("Brightness", brightness, 0.05, 0.0, 10.0),
            color_field!("Ambient Color", ambient_color),
            color_field!("Shadow Color", shadow_color),
            float_field!("Ambient", ambient_brightness, 0.01, 0.0, 5.0),
            float_field!("Absorption", absorption, 0.01, 0.0, 5.0),
            float_field!("Forward Scattering", forward_scattering, 0.01, 0.0, 0.99),
            float_field!("Backward Scattering", backward_scattering, 0.01, -0.99, 0.0),
            float_field!("Scattering Blend", scattering_blend, 0.01, 0.0, 1.0),
            float_field!("Powder Effect", powder_strength, 0.01, 0.0, 1.0),
            // March
            int_field!("Raymarch Steps", raymarch_steps, 4.0, 128.0),
            int_field!("Shadow Steps", shadow_steps, 0.0, 32.0),
            // Atmosphere
            bool_field!("Atmosphere Lighting", atmosphere_lighting),
            color_field!("Horizon Color", horizon_color),
            float_field!("Atmosphere", atmosphere_strength, 0.01, 0.0, 1.0),
        ],
    }
}

/// Registered from the one plugin's `build`, unconditionally — a native plugin
/// is compiled with no cargo features, so a `cfg(feature = "editor")` gate would
/// be permanently false and the section would vanish with nothing logged.
pub(crate) fn register(app: &mut App) {
    app.register_inspector(inspector_entry());
}
