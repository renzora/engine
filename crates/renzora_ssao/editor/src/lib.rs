//! Editor-only half of `renzora_ssao` — the SSAO inspector.
//!
//! `renzora_ssao` compiles lean (no `editor` feature, no egui-phosphor).
//! This crate holds the inspector (renzora editor contract + Phosphor icon),
//! registered `renzora::add!(SsaoEditorPlugin, Editor)` and linked only by the
//! editor bundle.

use bevy::prelude::*;
use renzora::{AppEditorExt, InspectorEntry, WorldEnvironment};

fn ssao_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "world_env_ssao",
        display_name: "SSAO",
        icon: "circle-half",
        category: "rendering",
        has_fn: |world, entity| world.get::<WorldEnvironment>(entity).is_some(),
        // Intrinsic to the WorldEnvironment — not added or removed on its own.
        add_fn: None,
        remove_fn: None,
        is_enabled_fn: Some(|world, entity| {
            world
                .get::<WorldEnvironment>(entity)
                .map(|e| e.ssao.enabled)
                .unwrap_or(false)
        }),
        set_enabled_fn: Some(|world, entity, val| {
            if let Some(mut e) = world.get_mut::<WorldEnvironment>(entity) {
                e.ssao.enabled = val;
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
