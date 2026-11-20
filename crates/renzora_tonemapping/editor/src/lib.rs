//! Editor-only half of `renzora_tonemapping`: the Tonemapping row of the Camera
//! section, and the Tonemapping (off-camera) and Deband Dither inspectors.
//!
//! `renzora_tonemapping` compiles lean (no `editor` feature, no egui-phosphor).
//! This crate holds the inspectors (renzora editor contract + Phosphor icon),
//! registered `renzora::add!(TonemappingEditorPlugin, Editor)` and linked only by
//! the editor bundle.

use bevy::core_pipeline::tonemapping::{DebandDither, Tonemapping};
use bevy::prelude::*;
use bevy::render::view::ColorGrading;
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

/// One slider over a nested `ColorGrading` field, such as `global.exposure` or
/// `shadows.lift`. The shared `float_field!` takes a single field name, and
/// grading's values sit two levels down.
macro_rules! grading_field {
    ($name:literal, $($path:ident).+, $speed:expr, $min:expr, $max:expr) => {
        FieldDef {
            name: $name,
            field_type: FieldType::Float { speed: $speed, min: $min, max: $max },
            get_fn: |w, e| w.get::<ColorGrading>(e).map(|g| FieldValue::Float(g.$($path).+)),
            set_fn: |w, e, v| {
                if let (FieldValue::Float(v), Some(mut g)) = (v, w.get_mut::<ColorGrading>(e)) {
                    g.$($path).+ = v;
                }
            },
        }
    };
}

/// The five controls each tonal range has, folded into a group of its own.
macro_rules! grading_range {
    ($label:literal, $range:ident) => {
        FieldDef::group(
            $label,
            vec![
                grading_field!("Saturation", $range.saturation, 0.01, 0.0, 3.0),
                grading_field!("Contrast", $range.contrast, 0.01, 0.0, 3.0),
                grading_field!("Gamma", $range.gamma, 0.01, 0.1, 3.0),
                grading_field!("Gain", $range.gain, 0.01, 0.0, 3.0),
                grading_field!("Lift", $range.lift, 0.005, -1.0, 1.0),
            ],
        )
    };
}

/// The camera's colour grading: Bevy's `ColorGrading`, which every 3D camera
/// carries, applied with the tone curve. It had no section, so the only way to
/// grade a scene was a script, and what a script set showed only in Play (see
/// `sync_color_grading`, which now carries it to the viewport too).
///
/// No add or remove: Bevy requires the component on every 3D camera, so it is
/// always there, and each row's reset puts its neutral value back.
fn color_grading_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "color_grading",
        display_name: "Color Grading",
        icon: "palette",
        category: "camera",
        has_fn: |world, entity| {
            draws_on(world, entity) && world.get::<ColorGrading>(entity).is_some()
        },
        add_fn: None,
        remove_fn: None,
        is_enabled_fn: None,
        set_enabled_fn: None,
        fields: vec![
            // Stops, on top of the camera's exposure: what a colourist reaches
            // for first, then white balance and overall saturation.
            grading_field!("Exposure", global.exposure, 0.05, -5.0, 5.0),
            // Offsets to the CIE 1931 chromaticity, where a little goes a long
            // way: 0.02 is a warm afternoon, 0.5 turned a street solid red. A
            // range of ±1 left all the useful values in the slider's first few
            // pixels.
            grading_field!("Temperature", global.temperature, 0.002, -0.2, 0.2),
            grading_field!("Tint", global.tint, 0.002, -0.2, 0.2),
            grading_field!("Saturation", global.post_saturation, 0.01, 0.0, 3.0),
            grading_field!("Hue", global.hue, 0.01, -std::f32::consts::PI, std::f32::consts::PI),
            grading_range!("Shadows", shadows),
            grading_range!("Midtones", midtones),
            grading_range!("Highlights", highlights),
        ],
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
        app.register_inspector(color_grading_entry());
    }
}

renzora::add!(TonemappingEditorPlugin, Editor);
