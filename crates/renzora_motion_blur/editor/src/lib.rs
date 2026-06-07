//! Editor-only half of `renzora_motion_blur` — the Motion Blur inspector.
//!
//! `renzora_motion_blur` compiles lean (no `editor` feature, no egui-phosphor).
//! This crate holds the inspector (renzora editor contract + Phosphor icon),
//! registered `renzora::add!(MotionBlurEditorPlugin, Editor)` and linked only by
//! the editor bundle.

use bevy::post_process::motion_blur::MotionBlur;
use bevy::prelude::*;
use renzora::{AppEditorExt, FieldDef, FieldType, FieldValue, InspectorEntry};
use renzora_motion_blur::MotionBlurSettings;

fn inspector_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "motion_blur",
        display_name: "Motion Blur",
        icon: "wind",
        category: "rendering",
        has_fn: |world, entity| world.get::<MotionBlurSettings>(entity).is_some(),
        add_fn: Some(|world, entity| {
            world
                .entity_mut(entity)
                .insert(MotionBlurSettings::default());
        }),
        remove_fn: Some(|world, entity| {
            world
                .entity_mut(entity)
                .remove::<(MotionBlurSettings, MotionBlur)>();
        }),
        is_enabled_fn: Some(|world, entity| {
            world
                .get::<MotionBlurSettings>(entity)
                .map(|s| s.enabled)
                .unwrap_or(false)
        }),
        set_enabled_fn: Some(|world, entity, val| {
            if let Some(mut s) = world.get_mut::<MotionBlurSettings>(entity) {
                s.enabled = val;
            }
        }),
        fields: vec![
            FieldDef {
                name: "Shutter Angle",
                field_type: FieldType::Float {
                    speed: 0.01,
                    min: 0.0,
                    max: 2.0,
                },
                get_fn: |world, entity| {
                    world
                        .get::<MotionBlurSettings>(entity)
                        .map(|s| FieldValue::Float(s.shutter_angle))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Float(v) = val {
                        if let Some(mut s) = world.get_mut::<MotionBlurSettings>(entity) {
                            s.shutter_angle = v;
                        }
                    }
                },
            },
            FieldDef {
                name: "Samples",
                field_type: FieldType::Float {
                    speed: 1.0,
                    min: 0.0,
                    max: 16.0,
                },
                get_fn: |world, entity| {
                    world
                        .get::<MotionBlurSettings>(entity)
                        .map(|s| FieldValue::Float(s.samples))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Float(v) = val {
                        if let Some(mut s) = world.get_mut::<MotionBlurSettings>(entity) {
                            s.samples = v;
                        }
                    }
                },
            },
        ],
    }
}

/// Editor-scope companion to `renzora_motion_blur::MotionBlurPlugin`.
#[derive(Default)]
pub struct MotionBlurEditorPlugin;

impl Plugin for MotionBlurEditorPlugin {
    fn build(&self, app: &mut App) {
        info!("[editor] MotionBlurEditorPlugin");
        app.register_inspector(inspector_entry());
    }
}

renzora::add!(MotionBlurEditorPlugin, Editor);
