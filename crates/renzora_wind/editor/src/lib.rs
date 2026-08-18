//! Editor-only half of `renzora_wind` — two inspector sections.
//!
//! * **Wind** — a section of the one `WorldEnvironment`, alongside Fog and
//!   SSAO. Not separately addable: like fog, it is intrinsic to the
//!   environment, so there is no add/remove, only an enable toggle.
//! * **Wind Sway** — a per-mesh component, addable to anything with geometry.
//!   This is the knob for "same wind, different plant".

use bevy::prelude::*;
use renzora::{
    AppEditorExt, FieldDef, FieldType, FieldValue, InspectorEntry, WindSway, WorldEnvironment,
};

/// Read/write one `f32` on the `WorldEnvironment`'s wind section.
macro_rules! wind_field {
    ($label:expr, $field:ident, $speed:expr, $min:expr, $max:expr) => {
        FieldDef {
            name: $label,
            field_type: FieldType::Float {
                speed: $speed,
                min: $min,
                max: $max,
            },
            get_fn: |world, entity| {
                world
                    .get::<WorldEnvironment>(entity)
                    .map(|e| FieldValue::Float(e.wind.$field))
            },
            set_fn: |world, entity, value| {
                if let (Some(mut e), FieldValue::Float(v)) =
                    (world.get_mut::<WorldEnvironment>(entity), value)
                {
                    e.wind.$field = v;
                }
            },
        }
    };
}

/// Read/write one `f32` on a [`WindSway`].
macro_rules! sway_field {
    ($label:expr, $field:ident, $speed:expr, $min:expr, $max:expr) => {
        FieldDef {
            name: $label,
            field_type: FieldType::Float {
                speed: $speed,
                min: $min,
                max: $max,
            },
            get_fn: |world, entity| {
                world
                    .get::<WindSway>(entity)
                    .map(|s| FieldValue::Float(s.$field))
            },
            set_fn: |world, entity, value| {
                if let (Some(mut s), FieldValue::Float(v)) =
                    (world.get_mut::<WindSway>(entity), value)
                {
                    s.$field = v;
                }
            },
        }
    };
}

fn wind_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "world_env_wind",
        display_name: "Wind",
        icon: "wind",
        category: "rendering",
        has_fn: |world, entity| world.get::<WorldEnvironment>(entity).is_some(),
        // Intrinsic to the WorldEnvironment — not added or removed on its own.
        add_fn: None,
        remove_fn: None,
        is_enabled_fn: Some(|world, entity| {
            world
                .get::<WorldEnvironment>(entity)
                .map(|e| e.wind.enabled)
                .unwrap_or(false)
        }),
        set_enabled_fn: Some(|world, entity, val| {
            if let Some(mut e) = world.get_mut::<WorldEnvironment>(entity) {
                e.wind.enabled = val;
            }
        }),
        fields: vec![
            // 0-360 rather than -180..180: this is a compass bearing, and the
            // wrap-around belongs at north where a reader expects it.
            wind_field!("Direction", direction, 1.0, 0.0, 360.0),
            // 30 m/s is roughly a storm. Past that foliage stops reading as
            // wind-blown and starts reading as broken.
            wind_field!("Speed (m/s)", speed, 0.1, 0.0, 30.0),
            wind_field!("Gust Strength", gust_strength, 0.01, 0.0, 1.0),
            wind_field!("Gusts / sec", gust_frequency, 0.01, 0.0, 2.0),
            wind_field!("Turbulence", turbulence, 0.01, 0.0, 1.0),
        ],
    }
}

fn sway_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "wind_sway",
        display_name: "Wind Sway",
        icon: "plant",
        category: "rendering",
        has_fn: |world, entity| world.get::<WindSway>(entity).is_some(),
        add_fn: Some(|world, entity| {
            world.entity_mut(entity).insert(WindSway::default());
        }),
        remove_fn: Some(|world, entity| {
            world.entity_mut(entity).remove::<WindSway>();
        }),
        is_enabled_fn: Some(|world, entity| {
            world.get::<WindSway>(entity).map(|s| s.enabled).unwrap_or(false)
        }),
        set_enabled_fn: Some(|world, entity, val| {
            if let Some(mut s) = world.get_mut::<WindSway>(entity) {
                s.enabled = val;
            }
        }),
        fields: vec![
            sway_field!("Response", response, 0.01, 0.0, 3.0),
            sway_field!("Flutter", flutter, 0.01, 0.0, 3.0),
            sway_field!("Amplitude (m)", amplitude, 0.01, 0.0, 3.0),
            // Only consulted for meshes with no authored UV_1 weights — an
            // imported bush rather than a generated tree.
            sway_field!("Pivot Height (m)", pivot_height, 0.05, 0.05, 50.0),
        ],
    }
}

/// Editor-scope companion to `renzora_wind::WindPlugin`.
#[derive(Default)]
pub struct WindEditorPlugin;

impl Plugin for WindEditorPlugin {
    fn build(&self, app: &mut App) {
        info!("[editor] WindEditorPlugin");
        app.register_inspector(wind_entry());
        app.register_inspector(sway_entry());
    }
}

renzora::add!(WindEditorPlugin, Editor);
