//! Editor-only half of `renzora_tonemapping` — the Tonemapping + Deband Dither
//! inspectors.
//!
//! `renzora_tonemapping` compiles lean (no `editor` feature, no egui-phosphor).
//! This crate holds the inspectors (renzora editor contract + Phosphor icon),
//! registered `renzora::add!(TonemappingEditorPlugin, Editor)` and linked only by
//! the editor bundle.

use bevy::camera::Exposure;
use bevy::core_pipeline::tonemapping::{DebandDither, Tonemapping};
use bevy::prelude::*;
use egui_phosphor::regular;
use renzora::{AppEditorExt, InspectorEntry};
use renzora_tonemapping::{DebandDitherSettings, TonemappingSettings};

fn tonemapping_to_mode(t: &Tonemapping) -> u32 {
    match t {
        Tonemapping::None => 0,
        Tonemapping::Reinhard => 1,
        Tonemapping::ReinhardLuminance => 2,
        Tonemapping::AcesFitted => 3,
        Tonemapping::AgX => 4,
        Tonemapping::SomewhatBoringDisplayTransform => 5,
        Tonemapping::TonyMcMapface => 6,
        Tonemapping::BlenderFilmic => 7,
    }
}

fn inspector_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "tonemapping",
        display_name: "Tonemapping",
        icon: regular::SUN,
        category: "rendering",
        has_fn: |world, entity| world.get::<TonemappingSettings>(entity).is_some(),
        add_fn: Some(|world, entity| {
            let mode = world
                .get::<Tonemapping>(entity)
                .map(tonemapping_to_mode)
                .unwrap_or(6);
            let ev100 = world
                .get::<Exposure>(entity)
                .map(|e| e.ev100)
                .unwrap_or(9.7);
            world.entity_mut(entity).insert(TonemappingSettings {
                mode,
                ev100,
                enabled: true,
            });
        }),
        remove_fn: Some(|world, entity| {
            world
                .entity_mut(entity)
                .remove::<TonemappingSettings>()
                .insert((Tonemapping::default(), Exposure::default()));
        }),
        is_enabled_fn: Some(|world, entity| {
            world
                .get::<TonemappingSettings>(entity)
                .map(|s| s.enabled)
                .unwrap_or(false)
        }),
        set_enabled_fn: Some(|world, entity, val| {
            if let Some(mut s) = world.get_mut::<TonemappingSettings>(entity) {
                s.enabled = val;
            }
        }),
        fields: vec![
            renzora::enum_u32_field!(
                "Mode",
                TonemappingSettings,
                mode,
                [
                    "None",
                    "Reinhard",
                    "Reinhard Luminance",
                    "ACES Fitted",
                    "AgX",
                    "Somewhat Boring",
                    "Tony McMapface",
                    "Blender Filmic"
                ]
            ),
            renzora::float_field!("EV100", TonemappingSettings, ev100, 0.1, -16.0, 16.0),
        ],
    }
}

fn deband_dither_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "deband_dither",
        display_name: "Deband Dither",
        icon: regular::GRADIENT,
        category: "rendering",
        has_fn: |world, entity| world.get::<DebandDitherSettings>(entity).is_some(),
        add_fn: Some(|world, entity| {
            world
                .entity_mut(entity)
                .insert(DebandDitherSettings::default());
        }),
        remove_fn: Some(|world, entity| {
            world.entity_mut(entity).remove::<DebandDitherSettings>();
            world.entity_mut(entity).insert(DebandDither::Disabled);
        }),
        is_enabled_fn: Some(|world, entity| {
            world
                .get::<DebandDitherSettings>(entity)
                .map(|s| s.enabled)
                .unwrap_or(false)
        }),
        set_enabled_fn: Some(|world, entity, val| {
            if let Some(mut s) = world.get_mut::<DebandDitherSettings>(entity) {
                s.enabled = val;
            }
        }),
        fields: vec![],
    }
}

/// Editor-scope companion to `renzora_tonemapping::TonemappingPlugin`.
#[derive(Default)]
pub struct TonemappingEditorPlugin;

impl Plugin for TonemappingEditorPlugin {
    fn build(&self, app: &mut App) {
        info!("[editor] TonemappingEditorPlugin");
        app.register_inspector(inspector_entry());
        app.register_inspector(deband_dither_entry());
    }
}

renzora::add!(TonemappingEditorPlugin, Editor);
