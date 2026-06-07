//! Editor-only half of `renzora_bloom_effect` — the Bloom inspector.
//!
//! `renzora_bloom_effect` compiles lean (no `editor` feature, no egui-phosphor).
//! This crate holds the inspector (renzora editor contract + Phosphor icon),
//! registered `renzora::add!(BloomEditorPlugin, Editor)` and linked only by the
//! editor bundle.

use bevy::post_process::bloom::Bloom;
use bevy::prelude::*;
use egui_phosphor::regular;
use renzora::{AppEditorExt, FieldDef, FieldType, FieldValue, InspectorEntry};
use renzora_bloom_effect::BloomSettings;

fn inspector_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "bloom",
        display_name: "Bloom",
        icon: regular::SPARKLE,
        category: "rendering",
        has_fn: |world, entity| world.get::<BloomSettings>(entity).is_some(),
        add_fn: Some(|world, entity| {
            world.entity_mut(entity).insert(BloomSettings::default());
        }),
        remove_fn: Some(|world, entity| {
            world.entity_mut(entity).remove::<(BloomSettings, Bloom)>();
        }),
        is_enabled_fn: Some(|world, entity| {
            world
                .get::<BloomSettings>(entity)
                .map(|s| s.enabled)
                .unwrap_or(false)
        }),
        set_enabled_fn: Some(|world, entity, val| {
            if let Some(mut s) = world.get_mut::<BloomSettings>(entity) {
                s.enabled = val;
            }
        }),
        fields: vec![
            FieldDef {
                name: "Intensity",
                field_type: FieldType::Float {
                    speed: 0.01,
                    min: 0.0,
                    max: 1.0,
                },
                get_fn: |world, entity| {
                    world
                        .get::<BloomSettings>(entity)
                        .map(|s| FieldValue::Float(s.intensity))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Float(v) = val {
                        if let Some(mut s) = world.get_mut::<BloomSettings>(entity) {
                            s.intensity = v;
                        }
                    }
                },
            },
            FieldDef {
                name: "Low Freq Boost",
                field_type: FieldType::Float {
                    speed: 0.01,
                    min: 0.0,
                    max: 1.0,
                },
                get_fn: |world, entity| {
                    world
                        .get::<BloomSettings>(entity)
                        .map(|s| FieldValue::Float(s.low_frequency_boost))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Float(v) = val {
                        if let Some(mut s) = world.get_mut::<BloomSettings>(entity) {
                            s.low_frequency_boost = v;
                        }
                    }
                },
            },
            FieldDef {
                name: "High Pass Freq",
                field_type: FieldType::Float {
                    speed: 0.01,
                    min: 0.0,
                    max: 1.0,
                },
                get_fn: |world, entity| {
                    world
                        .get::<BloomSettings>(entity)
                        .map(|s| FieldValue::Float(s.high_pass_frequency))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Float(v) = val {
                        if let Some(mut s) = world.get_mut::<BloomSettings>(entity) {
                            s.high_pass_frequency = v;
                        }
                    }
                },
            },
            FieldDef {
                name: "Threshold",
                field_type: FieldType::Float {
                    speed: 0.01,
                    min: 0.0,
                    max: 5.0,
                },
                get_fn: |world, entity| {
                    world
                        .get::<BloomSettings>(entity)
                        .map(|s| FieldValue::Float(s.threshold))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Float(v) = val {
                        if let Some(mut s) = world.get_mut::<BloomSettings>(entity) {
                            s.threshold = v;
                        }
                    }
                },
            },
            FieldDef {
                name: "Threshold Softness",
                field_type: FieldType::Float {
                    speed: 0.01,
                    min: 0.0,
                    max: 1.0,
                },
                get_fn: |world, entity| {
                    world
                        .get::<BloomSettings>(entity)
                        .map(|s| FieldValue::Float(s.threshold_softness))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Float(v) = val {
                        if let Some(mut s) = world.get_mut::<BloomSettings>(entity) {
                            s.threshold_softness = v;
                        }
                    }
                },
            },
        ],
    }
}

/// Editor-scope companion to `renzora_bloom_effect::BloomEffectPlugin`.
#[derive(Default)]
pub struct BloomEditorPlugin;

impl Plugin for BloomEditorPlugin {
    fn build(&self, app: &mut App) {
        info!("[editor] BloomEditorPlugin");
        app.register_inspector(inspector_entry());
    }
}

renzora::add!(BloomEditorPlugin, Editor);
