//! Editor-only half of `renzora_lens_distortion` — the inspector.
//!
//! `renzora_lens_distortion` compiles lean (no `editor` feature). This crate
//! holds the inspector, registered `renzora::add!(.., Editor)` and linked only
//! by the editor bundle.

use bevy::post_process::effect_stack::LensDistortion;
use bevy::prelude::*;
use renzora::{AppEditorExt, InspectorEntry};
use renzora_lens_distortion::LensDistortionSettings;

fn lens_distortion_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "lens_distortion",
        display_name: "Lens Distortion",
        icon: "circle",
        category: "effects",
        has_fn: |world, entity| world.get::<LensDistortionSettings>(entity).is_some(),
        add_fn: Some(|world, entity| {
            world
                .entity_mut(entity)
                .insert(LensDistortionSettings::default());
        }),
        remove_fn: Some(|world, entity| {
            world
                .entity_mut(entity)
                .remove::<(LensDistortionSettings, LensDistortion)>();
        }),
        is_enabled_fn: Some(|world, entity| {
            world
                .get::<LensDistortionSettings>(entity)
                .map(|s| s.enabled)
                .unwrap_or(false)
        }),
        set_enabled_fn: Some(|world, entity, val| {
            if let Some(mut s) = world.get_mut::<LensDistortionSettings>(entity) {
                s.enabled = val;
            }
        }),
        fields: vec![
            renzora::float_field!("Intensity", LensDistortionSettings, intensity, 0.01, -1.0, 1.0),
            renzora::float_field!("Scale", LensDistortionSettings, scale, 0.01, 0.5, 2.0),
        ],
    }
}

#[derive(Default)]
pub struct LensDistortionEditorPlugin;

impl Plugin for LensDistortionEditorPlugin {
    fn build(&self, app: &mut App) {
        info!("[editor] LensDistortionEditorPlugin");
        app.register_inspector(lens_distortion_entry());
    }
}

renzora::add!(LensDistortionEditorPlugin, Editor);
