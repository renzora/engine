//! Editor-only half of `renzora_volumetric_fog` — the Volumetric Fog inspector.
//!
//! `renzora_volumetric_fog` compiles lean (no `editor` feature, no
//! egui-phosphor). This crate holds the inspector (renzora editor contract +
//! Phosphor icon), registered `renzora::add!(VolumetricFogEditorPlugin, Editor)`
//! and linked only by the editor bundle.

use bevy::light::VolumetricFog;
use bevy::prelude::*;
use egui_phosphor::regular;
use renzora::{AppEditorExt, InspectorEntry};
use renzora_volumetric_fog::VolumetricFogSettings;

fn inspector_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "volumetric_fog",
        display_name: "Volumetric Fog",
        icon: regular::CLOUD_FOG,
        category: "environment",
        has_fn: |world, entity| world.get::<VolumetricFogSettings>(entity).is_some(),
        add_fn: Some(|world, entity| {
            world
                .entity_mut(entity)
                .insert(VolumetricFogSettings::default());
        }),
        remove_fn: Some(|world, entity| {
            world
                .entity_mut(entity)
                .remove::<(VolumetricFogSettings, VolumetricFog)>();
        }),
        is_enabled_fn: Some(|world, entity| {
            world
                .get::<VolumetricFogSettings>(entity)
                .map(|s| s.enabled)
                .unwrap_or(false)
        }),
        set_enabled_fn: Some(|world, entity, val| {
            if let Some(mut s) = world.get_mut::<VolumetricFogSettings>(entity) {
                s.enabled = val;
            }
        }),
        fields: vec![
            renzora::tuple_color_field!("Color", VolumetricFogSettings, ambient_color),
            renzora::float_field!("Ambient Intensity", VolumetricFogSettings, ambient_intensity, 0.01, 0.0, 4.0),
            renzora::int_field!("Step Count", VolumetricFogSettings, step_count, u32, 1.0, 8.0, 256.0),
            renzora::float_field!("Jitter", VolumetricFogSettings, jitter, 0.01, 0.0, 1.0),
        ],
    }
}

/// Editor-scope companion to `renzora_volumetric_fog::VolumetricFogPlugin`.
#[derive(Default)]
pub struct VolumetricFogEditorPlugin;

impl Plugin for VolumetricFogEditorPlugin {
    fn build(&self, app: &mut App) {
        info!("[editor] VolumetricFogEditorPlugin");
        app.register_inspector(inspector_entry());
    }
}

renzora::add!(VolumetricFogEditorPlugin, Editor);
