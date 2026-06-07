//! Editor-only half of `renzora_oit` — the OIT Transparency inspector.
//!
//! `renzora_oit` compiles lean (no `editor` feature, no egui-phosphor). This
//! crate holds the inspector (renzora editor contract + Phosphor icon),
//! registered `renzora::add!(OitEditorPlugin, Editor)` and linked only by the
//! editor bundle.

use bevy::core_pipeline::oit::OrderIndependentTransparencySettings;
use bevy::prelude::*;
use egui_phosphor::regular;
use renzora::{AppEditorExt, InspectorEntry};
use renzora_oit::OitSettings;

fn oit_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "oit",
        display_name: "OIT Transparency",
        icon: regular::STACK,
        category: "rendering",
        has_fn: |world, entity| world.get::<OitSettings>(entity).is_some(),
        add_fn: Some(|world, entity| {
            world.entity_mut(entity).insert(OitSettings::default());
        }),
        remove_fn: Some(|world, entity| {
            world
                .entity_mut(entity)
                .remove::<(OitSettings, OrderIndependentTransparencySettings)>();
        }),
        is_enabled_fn: Some(|world, entity| {
            world
                .get::<OitSettings>(entity)
                .map(|s| s.enabled)
                .unwrap_or(false)
        }),
        set_enabled_fn: Some(|world, entity, val| {
            if let Some(mut s) = world.get_mut::<OitSettings>(entity) {
                s.enabled = val;
            }
        }),
        fields: vec![
            renzora::int_field!("Layers", OitSettings, layer_count, i32, 1.0, 1.0, 32.0),
            renzora::float_field!(
                "Alpha Threshold",
                OitSettings,
                alpha_threshold,
                0.01,
                0.0,
                1.0
            ),
        ],
    }
}

/// Editor-scope companion to `renzora_oit::OitPlugin`.
#[derive(Default)]
pub struct OitEditorPlugin;

impl Plugin for OitEditorPlugin {
    fn build(&self, app: &mut App) {
        info!("[editor] OitEditorPlugin");
        app.register_inspector(oit_entry());
    }
}

renzora::add!(OitEditorPlugin, Editor);
