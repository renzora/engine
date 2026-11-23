//! Editor-only half of `renzora_ssr` — the SSR inspector.
//!
//! `renzora_ssr` compiles lean (no `editor` feature, no egui-phosphor).
//! This crate holds the inspector (renzora editor contract + Phosphor icon),
//! registered `renzora::add!(SsrEditorPlugin, Editor)` and linked only by the
//! editor bundle.

use bevy::pbr::ScreenSpaceReflections;
use bevy::prelude::*;
use renzora_editor_api::{AppEditorExt, InspectorEntry};
use renzora_ssr::SsrSettings;

fn ssr_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "ssr",
        display_name: "SSR",
        icon: "swap",
        category: "post_process",
        has_fn: |world, entity| world.get::<SsrSettings>(entity).is_some(),
        add_fn: Some(|world, entity| {
            world.entity_mut(entity).insert(SsrSettings::default());
        }),
        remove_fn: Some(|world, entity| {
            world
                .entity_mut(entity)
                .remove::<(SsrSettings, ScreenSpaceReflections)>();
        }),
        // While Solari lights the camera SSR is stood down (see
        // `renzora_ssr::sync_ssr`), so the switch reads off and does nothing.
        // The stored setting is left alone and returns when Solari goes off.
        is_enabled_fn: Some(|world, entity| {
            !renzora_ssr::stood_down_by_solari(world, entity)
                && world.get::<SsrSettings>(entity).is_some_and(|s| s.enabled)
        }),
        set_enabled_fn: Some(|world, entity, val| {
            if renzora_ssr::stood_down_by_solari(world, entity) {
                return;
            }
            if let Some(mut s) = world.get_mut::<SsrSettings>(entity) {
                s.enabled = val;
            }
        }),
        fields: vec![
            renzora_editor_api::FieldDef {
                name: "Solari",
                field_type: renzora_editor_api::FieldType::ReadOnly,
                get_fn: |world, entity| {
                    renzora_ssr::stood_down_by_solari(world, entity).then(|| {
                        renzora_editor_api::FieldValue::ReadOnly(
                            "Off while Solari ray-traced GI is on: it traces its own reflections."
                                .to_string(),
                        )
                    })
                },
                set_fn: |_, _, _| {},
            },
            renzora_editor_api::float_field!("Thickness", SsrSettings, thickness, 0.01, 0.0, 5.0),
            renzora_editor_api::int_field!("Linear Steps", SsrSettings, linear_steps, u32, 1.0, 1.0, 64.0),
            renzora_editor_api::int_field!("Bisection Steps", SsrSettings, bisection_steps, u32, 1.0, 0.0, 16.0),
            renzora_editor_api::bool_field!("Secant Refine", SsrSettings, use_secant),
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
