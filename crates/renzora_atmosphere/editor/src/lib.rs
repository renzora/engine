//! Editor-only half of `renzora_atmosphere` — the Atmosphere inspector.
//!
//! `renzora_atmosphere` compiles lean (no `editor` feature, no egui-phosphor).
//! This crate holds the inspector (renzora editor contract + Phosphor icon),
//! registered `renzora::add!(AtmosphereEditorPlugin, Editor)` and linked only
//! by the editor bundle.

use bevy::light::Atmosphere; // 0.19: moved to bevy::light
use bevy::pbr::AtmosphereSettings;
use bevy::prelude::*;
use renzora::{AppEditorExt, InspectorEntry};
use renzora_atmosphere::AtmosphereComponentSettings;

fn inspector_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "atmosphere",
        display_name: "Atmosphere",
        icon: "cloud-sun",
        category: "rendering",
        has_fn: |world, entity| world.get::<AtmosphereComponentSettings>(entity).is_some(),
        add_fn: Some(|world, entity| {
            world
                .entity_mut(entity)
                .insert(AtmosphereComponentSettings::default());
        }),
        remove_fn: Some(|world, entity| {
            // Removing the source settings is what tears the sky down:
            // `sync_atmosphere` reconciles "no enabled source → strip the planet
            // `Atmosphere`". `Atmosphere`/`AtmosphereSettings` live on the planet
            // and cameras (not here), but removing them is a harmless no-op on
            // this entity and keeps the intent explicit.
            world
                .entity_mut(entity)
                .remove::<(AtmosphereComponentSettings, Atmosphere, AtmosphereSettings)>();
        }),
        is_enabled_fn: Some(|world, entity| {
            world
                .get::<AtmosphereComponentSettings>(entity)
                .map(|s| s.enabled)
                .unwrap_or(false)
        }),
        set_enabled_fn: Some(|world, entity, val| {
            if let Some(mut s) = world.get_mut::<AtmosphereComponentSettings>(entity) {
                s.enabled = val;
            }
        }),
        fields: vec![
            renzora::FieldDef {
                name: "Rendering",
                field_type: renzora::FieldType::Enum {
                    options: &["Lookup Texture", "Raymarched"],
                },
                get_fn: |w, e| {
                    w.get::<AtmosphereComponentSettings>(e).map(|s| {
                        renzora::FieldValue::Enum(
                            if s.mode == 1 { "Raymarched" } else { "Lookup Texture" }.to_string(),
                        )
                    })
                },
                set_fn: |w, e, v| {
                    if let (renzora::FieldValue::Enum(label), Some(mut s)) =
                        (v, w.get_mut::<AtmosphereComponentSettings>(e))
                    {
                        s.mode = if label == "Raymarched" { 1 } else { 0 };
                    }
                },
            },
            renzora::float_field!("Bottom Radius", AtmosphereComponentSettings, bottom_radius, 1000.0, 0.0, 100_000_000.0),
            renzora::float_field!("Top Radius", AtmosphereComponentSettings, top_radius, 1000.0, 0.0, 100_000_000.0),
            renzora::float_field!("Ground Albedo", AtmosphereComponentSettings, ground_albedo, 0.01, 0.0, 1.0),
            renzora::float_field!("Units to m", AtmosphereComponentSettings, scene_units_to_m, 0.1, 0.0001, 10000.0),
        ],
    }
}

/// Editor-scope companion to `renzora_atmosphere::AtmospherePlugin`.
#[derive(Default)]
pub struct AtmosphereEditorPlugin;

impl Plugin for AtmosphereEditorPlugin {
    fn build(&self, app: &mut App) {
        info!("[editor] AtmosphereEditorPlugin");
        app.register_inspector(inspector_entry());
    }
}

renzora::add!(AtmosphereEditorPlugin, Editor);
