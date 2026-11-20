//! Editor-only half of `renzora_tonemapping`: the Tonemapping row of the Camera
//! section, and the Tonemapping (off-camera) and Deband Dither inspectors.
//!
//! `renzora_tonemapping` compiles lean (no `editor` feature, no egui-phosphor).
//! This crate holds the inspectors (renzora editor contract + Phosphor icon),
//! registered `renzora::add!(TonemappingEditorPlugin, Editor)` and linked only by
//! the editor bundle.

use bevy::core_pipeline::tonemapping::{DebandDither, Tonemapping};
use bevy::prelude::*;
use renzora::{AppEditorExt, FieldDef, FieldType, FieldValue, InspectorEntry};
use renzora_tonemapping::{DebandDitherSettings, TonemappingSettings};

fn tonemapping_to_mode(t: &Tonemapping) -> u32 {
    match t {
        // Both read as "no curve" in the inspector; see `mode_to_tonemapping`.
        Tonemapping::None | Tonemapping::Linear => 0,
        Tonemapping::Reinhard => 1,
        Tonemapping::ReinhardLuminance => 2,
        Tonemapping::AcesFitted => 3,
        Tonemapping::AgX => 4,
        Tonemapping::SomewhatBoringDisplayTransform => 5,
        Tonemapping::TonyMcMapface => 6,
        Tonemapping::BlenderFilmic => 7,
        // 0.19 added KhronosPbrNeutral.
        Tonemapping::KhronosPbrNeutral => 8,
    }
}

/// The curves, index for index with `TonemappingSettings::mode`.
const MODE_LABELS: [&str; 8] = [
    "None",
    "Reinhard",
    "Reinhard Luminance",
    "ACES Fitted",
    "AgX",
    "Somewhat Boring",
    "Tony McMapface",
    "Blender Filmic",
];

/// Whether `entity` is a camera whose Camera section draws the Tonemapping row.
/// Not a 2D camera: the camera seeder strips tonemapping from those, so a value
/// set there would not last a frame.
fn draws_on(world: &World, entity: Entity) -> bool {
    world.get::<Camera>(entity).is_some() && world.get::<Camera2d>(entity).is_none()
}

/// A fresh `TonemappingSettings` for `entity`, starting from the curve the
/// camera already renders with so adding one changes nothing on screen.
fn settings_for(world: &World, entity: Entity) -> TonemappingSettings {
    let mode = world
        .get::<Tonemapping>(entity)
        .map(tonemapping_to_mode)
        .unwrap_or(6);
    // `ev100` is vestigial: exposure moved to the Camera section's EV100.
    // Keep a sane default on the back-compat field.
    TonemappingSettings { mode, ev100: 9.7, enabled: true }
}

/// The Tonemapping row of the Camera section. The curve is the camera's own
/// picture formation, so it is drawn with the camera rather than as a section
/// of its own; the data stays in `TonemappingSettings`, so routing and saved
/// scenes are unchanged. "None" is the off setting (no curve), the same thing
/// the old enable switch did, so there is no separate switch.
fn camera_row() -> FieldDef {
    FieldDef {
        name: "Tonemapping",
        field_type: FieldType::Enum { options: &MODE_LABELS },
        get_fn: |w, e| {
            if !draws_on(w, e) {
                return None;
            }
            let mode = match w.get::<TonemappingSettings>(e) {
                Some(s) if s.enabled => s.mode as usize,
                Some(_) => 0,
                None => settings_for(w, e).mode as usize,
            };
            let label = MODE_LABELS.get(mode).copied().unwrap_or(MODE_LABELS[6]);
            Some(FieldValue::Enum(label.to_string()))
        },
        set_fn: |w, e, v| {
            let FieldValue::Enum(label) = v else { return };
            let Some(mode) = MODE_LABELS.iter().position(|l| *l == label) else { return };
            if w.get::<TonemappingSettings>(e).is_none() {
                let fresh = settings_for(w, e);
                w.entity_mut(e).insert(fresh);
            }
            if let Some(mut s) = w.get_mut::<TonemappingSettings>(e) {
                s.mode = mode as u32;
                s.enabled = true;
            }
        },
    }
}

/// The component's own section, for the one case the Camera row does not
/// cover: `TonemappingSettings` on an entity that is not a 3D camera (older
/// scenes carried it on the World Environment). Not offered in Add Component;
/// on a camera, the Camera section's row is the way to set it.
fn inspector_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "tonemapping",
        display_name: "Tonemapping",
        icon: "sun",
        category: "camera",
        has_fn: |world, entity| {
            world.get::<TonemappingSettings>(entity).is_some() && !draws_on(world, entity)
        },
        add_fn: None,
        remove_fn: Some(|world, entity| {
            world
                .entity_mut(entity)
                .remove::<TonemappingSettings>()
                .insert(Tonemapping::default());
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
        ],
    }
}

fn deband_dither_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "deband_dither",
        display_name: "Deband Dither",
        icon: "gradient",
        category: "camera",
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
        app.extend_inspector("camera", vec![camera_row()]);
        app.register_inspector(inspector_entry());
        app.register_inspector(deband_dither_entry());
    }
}

renzora::add!(TonemappingEditorPlugin, Editor);
