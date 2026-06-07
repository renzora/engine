//! Editor-only half of `renzora_ssao` — the SSAO inspector.
//!
//! `renzora_ssao` compiles lean (no `editor` feature, no egui-phosphor).
//! This crate holds the inspector (renzora editor contract + Phosphor icon),
//! registered `renzora::add!(SsaoEditorPlugin, Editor)` and linked only by the
//! editor bundle.

use bevy::pbr::ScreenSpaceAmbientOcclusion;
use bevy::prelude::*;
use renzora::{AppEditorExt, InspectorEntry};
use renzora_ssao::SsaoSettings;

fn ssao_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "ssao",
        display_name: "SSAO",
        icon: "circle-half",
        category: "rendering",
        has_fn: |world, entity| world.get::<SsaoSettings>(entity).is_some(),
        add_fn: Some(|world, entity| {
            world.entity_mut(entity).insert(SsaoSettings::default());
        }),
        remove_fn: Some(|world, entity| {
            world
                .entity_mut(entity)
                .remove::<(SsaoSettings, ScreenSpaceAmbientOcclusion)>();
        }),
        is_enabled_fn: Some(|world, entity| {
            world
                .get::<SsaoSettings>(entity)
                .map(|s| s.enabled)
                .unwrap_or(false)
        }),
        set_enabled_fn: Some(|world, entity, val| {
            if let Some(mut s) = world.get_mut::<SsaoSettings>(entity) {
                s.enabled = val;
            }
        }),
        fields: vec![],
    }
}

/// Editor-scope companion to `renzora_ssao::SsaoPlugin`.
#[derive(Default)]
pub struct SsaoEditorPlugin;

impl Plugin for SsaoEditorPlugin {
    fn build(&self, app: &mut App) {
        info!("[editor] SsaoEditorPlugin");
        app.register_inspector(ssao_entry());
    }
}

renzora::add!(SsaoEditorPlugin, Editor);
