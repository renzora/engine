//! Editor-only half of `renzora_environment_map` — the Environment Map (IBL)
//! inspector.
//!
//! `renzora_environment_map` compiles lean (no `editor` feature, no
//! egui-phosphor). This crate holds the inspector (renzora editor contract +
//! Phosphor icon), registered `renzora::add!(EnvironmentMapEditorPlugin, Editor)`
//! and linked only by the editor bundle.

use bevy::prelude::*;
use egui_phosphor::regular;
use renzora::{AppEditorExt, InspectorEntry};
use renzora_environment_map::EnvironmentMapComponentSettings;

fn inspector_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "environment_map",
        display_name: "Environment Map",
        icon: regular::SUN_HORIZON,
        category: "rendering",
        has_fn: |world, entity| {
            world
                .get::<EnvironmentMapComponentSettings>(entity)
                .is_some()
        },
        add_fn: Some(|world, entity| {
            world
                .entity_mut(entity)
                .insert(EnvironmentMapComponentSettings::default());
        }),
        remove_fn: Some(|world, entity| {
            world
                .entity_mut(entity)
                .remove::<EnvironmentMapComponentSettings>();
        }),
        is_enabled_fn: Some(|world, entity| {
            world
                .get::<EnvironmentMapComponentSettings>(entity)
                .map(|s| s.enabled)
                .unwrap_or(false)
        }),
        set_enabled_fn: Some(|world, entity, val| {
            if let Some(mut s) = world.get_mut::<EnvironmentMapComponentSettings>(entity) {
                s.enabled = val;
            }
        }),
        fields: vec![renzora::float_field!(
            "Intensity",
            EnvironmentMapComponentSettings,
            intensity,
            0.01,
            0.0,
            10.0
        )],
    }
}

/// Editor-scope companion to `renzora_environment_map::EnvironmentMapPlugin`.
#[derive(Default)]
pub struct EnvironmentMapEditorPlugin;

impl Plugin for EnvironmentMapEditorPlugin {
    fn build(&self, app: &mut App) {
        info!("[editor] EnvironmentMapEditorPlugin");
        app.register_inspector(inspector_entry());
    }
}

renzora::add!(EnvironmentMapEditorPlugin, Editor);
