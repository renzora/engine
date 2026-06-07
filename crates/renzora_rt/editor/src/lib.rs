//! Editor-only half of `renzora_rt` — the RT Lighting (SSGI) inspector.
//!
//! `renzora_rt` compiles lean (no `editor` feature, no egui-phosphor). This
//! crate holds the inspector (renzora editor contract + Phosphor icon),
//! registered `renzora::add!(RtEditorPlugin, Editor)` and linked only by the
//! editor bundle.

use bevy::prelude::*;
use renzora::{AppEditorExt, FieldDef, FieldType, FieldValue, InspectorEntry};
use renzora_rt::{RtDebugMode, RtLighting};

fn inspector_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "rt_lighting",
        display_name: "RT Lighting (SSGI)",
        icon: "lightning",
        category: "lighting",
        has_fn: |world, entity| world.get::<RtLighting>(entity).is_some(),
        add_fn: Some(|world, entity| {
            world.entity_mut(entity).insert(RtLighting::default());
        }),
        remove_fn: Some(|world, entity| {
            world.entity_mut(entity).remove::<RtLighting>();
        }),
        is_enabled_fn: Some(|world, entity| {
            world.get::<RtLighting>(entity).map(|s| s.enabled).unwrap_or(false)
        }),
        set_enabled_fn: Some(|world, entity, val| {
            if let Some(mut s) = world.get_mut::<RtLighting>(entity) {
                s.enabled = val;
            }
        }),
        fields: vec![
            renzora::float_field!("Intensity", RtLighting, intensity, 0.05, 0.0, 5.0),
            FieldDef {
                name: "Debug",
                field_type: FieldType::Enum {
                    options: &["Composite", "Indirect Only"],
                },
                get_fn: |w, e| {
                    w.get::<RtLighting>(e).map(|s| {
                        FieldValue::Enum(
                            match s.debug {
                                RtDebugMode::Composite => "Composite",
                                RtDebugMode::IndirectOnly => "Indirect Only",
                            }
                            .to_string(),
                        )
                    })
                },
                set_fn: |w, e, v| {
                    if let (FieldValue::Enum(label), Some(mut s)) = (v, w.get_mut::<RtLighting>(e)) {
                        s.debug = match label.as_str() {
                            "Indirect Only" => RtDebugMode::IndirectOnly,
                            _ => RtDebugMode::Composite,
                        };
                    }
                },
            },
        ],
    }
}

/// Editor-scope companion to `renzora_rt::RtPlugin`.
#[derive(Default)]
pub struct RtEditorPlugin;

impl Plugin for RtEditorPlugin {
    fn build(&self, app: &mut App) {
        info!("[editor] RtEditorPlugin");
        app.register_inspector(inspector_entry());
    }
}

renzora::add!(RtEditorPlugin, Editor);
