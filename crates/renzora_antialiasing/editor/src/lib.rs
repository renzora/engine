//! Editor-only half of `renzora_antialiasing` — the FXAA / SMAA / TAA / CAS
//! inspectors.
//!
//! `renzora_antialiasing` compiles lean (no `editor` feature, no egui-phosphor).
//! This crate holds the inspectors (renzora editor contract + Phosphor icons),
//! registered `renzora::add!(AntiAliasingEditorPlugin, Editor)` and linked only
//! by the editor bundle.

use bevy::anti_alias::contrast_adaptive_sharpening::ContrastAdaptiveSharpening;
use bevy::anti_alias::fxaa::Fxaa;
use bevy::anti_alias::smaa::Smaa;
use bevy::anti_alias::taa::TemporalAntiAliasing;
use bevy::prelude::*;
use renzora::{AppEditorExt, InspectorEntry};
use renzora_antialiasing::{CasSettings, FxaaSettings, SmaaSettings, TaaSettings};

const SENSITIVITY_LABELS: [&str; 5] = ["Low", "Medium", "High", "Ultra", "Extreme"];

const SMAA_LABELS: [&str; 4] = ["Low", "Medium", "High", "Ultra"];

fn fxaa_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "fxaa",
        display_name: "FXAA",
        icon: "grid-four",
        category: "rendering",
        has_fn: |world, entity| world.get::<FxaaSettings>(entity).is_some(),
        add_fn: Some(|world, entity| {
            world.entity_mut(entity).insert(FxaaSettings::default());
        }),
        remove_fn: Some(|world, entity| {
            world.entity_mut(entity).remove::<(FxaaSettings, Fxaa)>();
        }),
        is_enabled_fn: Some(|world, entity| {
            world
                .get::<FxaaSettings>(entity)
                .map(|s| s.enabled)
                .unwrap_or(false)
        }),
        set_enabled_fn: Some(|world, entity, val| {
            if let Some(mut s) = world.get_mut::<FxaaSettings>(entity) {
                s.enabled = val;
            }
        }),
        fields: vec![
            renzora::FieldDef {
                name: "Edge Threshold",
                field_type: renzora::FieldType::Enum { options: &SENSITIVITY_LABELS },
                get_fn: |w, e| {
                    w.get::<FxaaSettings>(e).map(|s| {
                        renzora::FieldValue::Enum(
                            SENSITIVITY_LABELS.get(s.edge_threshold as usize).copied().unwrap_or("Low").to_string(),
                        )
                    })
                },
                set_fn: |w, e, v| {
                    if let (renzora::FieldValue::Enum(label), Some(mut s)) = (v, w.get_mut::<FxaaSettings>(e)) {
                        if let Some(i) = SENSITIVITY_LABELS.iter().position(|l| *l == label) {
                            s.edge_threshold = i as u32;
                        }
                    }
                },
            },
            renzora::FieldDef {
                name: "Edge Thr. Min",
                field_type: renzora::FieldType::Enum { options: &SENSITIVITY_LABELS },
                get_fn: |w, e| {
                    w.get::<FxaaSettings>(e).map(|s| {
                        renzora::FieldValue::Enum(
                            SENSITIVITY_LABELS.get(s.edge_threshold_min as usize).copied().unwrap_or("Low").to_string(),
                        )
                    })
                },
                set_fn: |w, e, v| {
                    if let (renzora::FieldValue::Enum(label), Some(mut s)) = (v, w.get_mut::<FxaaSettings>(e)) {
                        if let Some(i) = SENSITIVITY_LABELS.iter().position(|l| *l == label) {
                            s.edge_threshold_min = i as u32;
                        }
                    }
                },
            },
        ],
    }
}

fn smaa_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "smaa",
        display_name: "SMAA",
        icon: "grid-four",
        category: "rendering",
        has_fn: |world, entity| world.get::<SmaaSettings>(entity).is_some(),
        add_fn: Some(|world, entity| {
            world.entity_mut(entity).insert(SmaaSettings::default());
        }),
        remove_fn: Some(|world, entity| {
            world.entity_mut(entity).remove::<(SmaaSettings, Smaa)>();
        }),
        is_enabled_fn: Some(|world, entity| {
            world
                .get::<SmaaSettings>(entity)
                .map(|s| s.enabled)
                .unwrap_or(false)
        }),
        set_enabled_fn: Some(|world, entity, val| {
            if let Some(mut s) = world.get_mut::<SmaaSettings>(entity) {
                s.enabled = val;
            }
        }),
        fields: vec![renzora::FieldDef {
            name: "Preset",
            field_type: renzora::FieldType::Enum { options: &SMAA_LABELS },
            get_fn: |w, e| {
                w.get::<SmaaSettings>(e).map(|s| {
                    renzora::FieldValue::Enum(
                        SMAA_LABELS.get(s.preset as usize).copied().unwrap_or("Low").to_string(),
                    )
                })
            },
            set_fn: |w, e, v| {
                if let (renzora::FieldValue::Enum(label), Some(mut s)) = (v, w.get_mut::<SmaaSettings>(e)) {
                    if let Some(i) = SMAA_LABELS.iter().position(|l| *l == label) {
                        s.preset = i as u32;
                    }
                }
            },
        }],
    }
}

fn taa_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "taa",
        display_name: "TAA",
        icon: "grid-four",
        category: "rendering",
        has_fn: |world, entity| world.get::<TaaSettings>(entity).is_some(),
        add_fn: Some(|world, entity| {
            world.entity_mut(entity).insert(TaaSettings::default());
        }),
        remove_fn: Some(|world, entity| {
            world
                .entity_mut(entity)
                .remove::<(TaaSettings, TemporalAntiAliasing)>();
        }),
        is_enabled_fn: Some(|world, entity| {
            world
                .get::<TaaSettings>(entity)
                .map(|s| s.enabled)
                .unwrap_or(false)
        }),
        set_enabled_fn: Some(|world, entity, val| {
            if let Some(mut s) = world.get_mut::<TaaSettings>(entity) {
                s.enabled = val;
            }
        }),
        fields: vec![renzora::bool_field!("Reset", TaaSettings, reset)],
    }
}

fn cas_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "cas",
        display_name: "Sharpening (CAS)",
        icon: "diamonds-four",
        category: "rendering",
        has_fn: |world, entity| world.get::<CasSettings>(entity).is_some(),
        add_fn: Some(|world, entity| {
            world.entity_mut(entity).insert(CasSettings::default());
        }),
        remove_fn: Some(|world, entity| {
            world
                .entity_mut(entity)
                .remove::<(CasSettings, ContrastAdaptiveSharpening)>();
        }),
        is_enabled_fn: Some(|world, entity| {
            world
                .get::<CasSettings>(entity)
                .map(|s| s.enabled)
                .unwrap_or(false)
        }),
        set_enabled_fn: Some(|world, entity, val| {
            if let Some(mut s) = world.get_mut::<CasSettings>(entity) {
                s.enabled = val;
            }
        }),
        fields: vec![
            renzora::float_field!("Strength", CasSettings, sharpening_strength, 0.01, 0.0, 1.0),
            renzora::bool_field!("Denoise", CasSettings, denoise),
        ],
    }
}

/// Editor-scope companion to `renzora_antialiasing::AntiAliasingPlugin`.
#[derive(Default)]
pub struct AntiAliasingEditorPlugin;

impl Plugin for AntiAliasingEditorPlugin {
    fn build(&self, app: &mut App) {
        info!("[editor] AntiAliasingEditorPlugin");
        app.register_inspector(fxaa_entry());
        app.register_inspector(smaa_entry());
        app.register_inspector(taa_entry());
        app.register_inspector(cas_entry());
    }
}

renzora::add!(AntiAliasingEditorPlugin, Editor);
