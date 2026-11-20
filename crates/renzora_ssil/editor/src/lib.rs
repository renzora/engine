//! Editor-only half of `renzora_ssil`: the SSIL inspector section.
//!
//! `renzora_ssil` compiles lean, with no editor contract. This crate holds the
//! inspector, registered `renzora::add!(SsilEditorPlugin, Editor)` and linked
//! only by the editor bundle.

use bevy::prelude::*;
use renzora::{AppEditorExt, InspectorEntry};
use renzora_ssil::{Ssil, SsilSettings};

fn ssil_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "ssil",
        display_name: "SSIL",
        icon: "sun-dim",
        category: "post_process",
        has_fn: |world, entity| world.get::<SsilSettings>(entity).is_some(),
        add_fn: Some(|world, entity| {
            world.entity_mut(entity).insert(SsilSettings::default());
        }),
        remove_fn: Some(|world, entity| {
            world.entity_mut(entity).remove::<(SsilSettings, Ssil)>();
        }),
        // While Solari lights the camera SSIL is stood down (see
        // `renzora_ssil::sync_ssil`), so the switch reads off and does nothing.
        // The stored setting is left alone and returns when Solari goes off.
        is_enabled_fn: Some(|world, entity| {
            !renzora_ssil::stood_down_by_solari(world, entity)
                && world.get::<SsilSettings>(entity).is_some_and(|s| s.enabled)
        }),
        set_enabled_fn: Some(|world, entity, val| {
            if renzora_ssil::stood_down_by_solari(world, entity) {
                return;
            }
            if let Some(mut s) = world.get_mut::<SsilSettings>(entity) {
                s.enabled = val;
            }
        }),
        fields: vec![
            renzora::FieldDef {
                name: "Solari",
                field_type: renzora::FieldType::ReadOnly,
                get_fn: |world, entity| {
                    renzora_ssil::stood_down_by_solari(world, entity).then(|| {
                        renzora::FieldValue::ReadOnly(
                            "Off while Solari ray-traced GI is on: it traces its own bounce light."
                                .to_string(),
                        )
                    })
                },
                set_fn: |_, _, _| {},
            },
            renzora::float_field!("Intensity", SsilSettings, intensity, 0.01, 0.0, 8.0),
            renzora::float_field!("Occlusion", SsilSettings, occlusion, 0.01, 0.0, 2.0),
            renzora::float_field!("Radius", SsilSettings, radius, 0.05, 0.1, 50.0),
            // The strengths and reach stay on the card; how the gather is
            // tuned folds away, since most scenes never touch it.
            renzora::FieldDef::group(
                "Quality",
                vec![
                    renzora::int_field!(
                        "Samples",
                        SsilSettings,
                        samples,
                        u32,
                        1.0,
                        *renzora_ssil::SAMPLES.start() as f32,
                        *renzora_ssil::SAMPLES.end() as f32
                    ),
                    renzora::float_field!("Sharpness", SsilSettings, sharpness, 0.01, 0.0, 1.0),
                    renzora::float_field!("Normal Rejection", SsilSettings, normal_rejection, 0.01, 0.0, 1.0),
                ],
            ),
            renzora::FieldDef::group(
                "Distance Fade",
                vec![
                    renzora::float_field!("Fade Start", SsilSettings, fade_start, 1.0, 0.0, 2000.0),
                    renzora::float_field!("Fade End", SsilSettings, fade_end, 1.0, 0.0, 2000.0),
                ],
            ),
        ],
    }
}

/// Editor-scope companion to `renzora_ssil::SsilPlugin`.
#[derive(Default)]
pub struct SsilEditorPlugin;

impl Plugin for SsilEditorPlugin {
    fn build(&self, app: &mut App) {
        info!("[editor] SsilEditorPlugin");
        app.register_inspector(ssil_entry());
    }
}

renzora::add!(SsilEditorPlugin, Editor);
