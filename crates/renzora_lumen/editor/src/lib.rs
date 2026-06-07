//! Editor-only half of `renzora_lumen` — the Lumen Global Illumination
//! inspector.
//!
//! `renzora_lumen` compiles lean (no `editor` feature, no egui-phosphor). This
//! crate holds the inspector (renzora editor contract + Phosphor icon),
//! registered `renzora::add!(LumenEditorPlugin, Editor)` and linked only by the
//! editor bundle.

use bevy::prelude::*;
use egui_phosphor::regular::LIGHTNING;
use renzora::{AppEditorExt, InspectorEntry};
use renzora_lumen::{LumenDebug, LumenLighting, LumenQuality};
use renzora_rt::RtLighting;

fn inspector_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "lumen_lighting",
        display_name: "Lumen Global Illumination",
        icon: LIGHTNING,
        category: "lighting",
        has_fn: |world, entity| world.get::<LumenLighting>(entity).is_some(),
        add_fn: Some(|world, entity| {
            world.entity_mut(entity).insert(LumenLighting::default());
            // Lumen owns the screen-space tier when present — strip any
            // hand-attached `RtLighting` so the two don't double-apply.
            world.entity_mut(entity).remove::<RtLighting>();
        }),
        remove_fn: Some(|world, entity| {
            world.entity_mut(entity).remove::<(LumenLighting, RtLighting)>();
        }),
        is_enabled_fn: Some(|world, entity| {
            world
                .get::<LumenLighting>(entity)
                .map(|s| s.quality != LumenQuality::Off)
                .unwrap_or(false)
        }),
        set_enabled_fn: Some(|world, entity, val| {
            if let Some(mut s) = world.get_mut::<LumenLighting>(entity) {
                s.quality = if val { LumenQuality::ScreenSpace } else { LumenQuality::Off };
            }
        }),
        fields: vec![
            renzora::FieldDef {
                name: "Quality",
                field_type: renzora::FieldType::Enum {
                    options: &[
                        "Off",
                        "Screen Space (SSGI)",
                        "SDF Low (Phase 5)",
                        "SDF High (Phase 5)",
                        "Hardware RT (Phase 10)",
                    ],
                },
                get_fn: |w, e| {
                    w.get::<LumenLighting>(e).map(|c| {
                        renzora::FieldValue::Enum(
                            match c.quality {
                                LumenQuality::Off => "Off",
                                LumenQuality::ScreenSpace => "Screen Space (SSGI)",
                                LumenQuality::SdfLow => "SDF Low (Phase 5)",
                                LumenQuality::SdfHigh => "SDF High (Phase 5)",
                                LumenQuality::Hwrt => "Hardware RT (Phase 10)",
                            }
                            .to_string(),
                        )
                    })
                },
                set_fn: |w, e, v| {
                    if let (renzora::FieldValue::Enum(s), Some(mut c)) =
                        (v, w.get_mut::<LumenLighting>(e))
                    {
                        c.quality = match s.as_str() {
                            "Off" => LumenQuality::Off,
                            "SDF Low (Phase 5)" => LumenQuality::SdfLow,
                            "SDF High (Phase 5)" => LumenQuality::SdfHigh,
                            "Hardware RT (Phase 10)" => LumenQuality::Hwrt,
                            _ => LumenQuality::ScreenSpace,
                        };
                    }
                },
            },
            renzora::float_field!("Intensity", LumenLighting, intensity, 0.05, 0.0, 5.0),
            renzora::FieldDef {
                name: "Debug",
                field_type: renzora::FieldType::Enum {
                    options: &["None", "Indirect Only", "Voxel Cache"],
                },
                get_fn: |w, e| {
                    w.get::<LumenLighting>(e).map(|c| {
                        renzora::FieldValue::Enum(
                            match c.debug {
                                LumenDebug::None => "None",
                                LumenDebug::IndirectOnly => "Indirect Only",
                                LumenDebug::VoxelCache => "Voxel Cache",
                            }
                            .to_string(),
                        )
                    })
                },
                set_fn: |w, e, v| {
                    if let (renzora::FieldValue::Enum(s), Some(mut c)) =
                        (v, w.get_mut::<LumenLighting>(e))
                    {
                        c.debug = match s.as_str() {
                            "Indirect Only" => LumenDebug::IndirectOnly,
                            "Voxel Cache" => LumenDebug::VoxelCache,
                            _ => LumenDebug::None,
                        };
                    }
                },
            },
        ],
    }
}

/// Editor-scope companion to `renzora_lumen::LumenPlugin`.
#[derive(Default)]
pub struct LumenEditorPlugin;

impl Plugin for LumenEditorPlugin {
    fn build(&self, app: &mut App) {
        info!("[editor] LumenEditorPlugin");
        app.register_inspector(inspector_entry());
    }
}

renzora::add!(LumenEditorPlugin, Editor);
