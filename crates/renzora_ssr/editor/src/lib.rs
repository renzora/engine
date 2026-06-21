//! Editor-only half of `renzora_ssr` — the SSR inspector.
//!
//! `renzora_ssr` compiles lean (no `editor` feature, no egui-phosphor).
//! This crate holds the inspector (renzora editor contract + Phosphor icon),
//! registered `renzora::add!(SsrEditorPlugin, Editor)` and linked only by the
//! editor bundle.

use bevy::pbr::ScreenSpaceReflections;
use bevy::prelude::*;
use renzora::{AppEditorExt, InspectorEntry};
use renzora_ssr::SsrSettings;

fn ssr_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "ssr",
        display_name: "SSR",
        icon: "swap",
        category: "rendering",
        has_fn: |world, entity| world.get::<SsrSettings>(entity).is_some(),
        add_fn: Some(|world, entity| {
            world.entity_mut(entity).insert(SsrSettings::default());
        }),
        remove_fn: Some(|world, entity| {
            world
                .entity_mut(entity)
                .remove::<(SsrSettings, ScreenSpaceReflections)>();
        }),
        is_enabled_fn: Some(|world, entity| {
            world
                .get::<SsrSettings>(entity)
                .map(|s| s.enabled)
                .unwrap_or(false)
        }),
        set_enabled_fn: Some(|world, entity, val| {
            if let Some(mut s) = world.get_mut::<SsrSettings>(entity) {
                s.enabled = val;
            }
        }),
        fields: vec![
            renzora::float_field!("Thickness", SsrSettings, thickness, 0.01, 0.0, 5.0),
            renzora::int_field!("Linear Steps", SsrSettings, linear_steps, u32, 1.0, 1.0, 64.0),
            renzora::int_field!("Bisection Steps", SsrSettings, bisection_steps, u32, 1.0, 0.0, 16.0),
            renzora::bool_field!("Secant Refine", SsrSettings, use_secant),
        ],
    }
}

/// Editor-scope companion to `renzora_ssr::SsrPlugin`.
#[derive(Default)]
pub struct SsrEditorPlugin;

impl Plugin for SsrEditorPlugin {
    fn build(&self, app: &mut App) {
        info!("[editor] SsrEditorPlugin");
        app.register_inspector(ssr_entry());
    }
}

renzora::add!(SsrEditorPlugin, Editor);
