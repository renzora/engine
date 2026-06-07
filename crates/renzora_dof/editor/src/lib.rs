//! Editor-only half of `renzora_dof` — the Depth of Field inspector.
//!
//! `renzora_dof` compiles lean (no `editor` feature, no egui-phosphor).
//! This crate holds the inspector (renzora editor contract + Phosphor icon),
//! registered `renzora::add!(DepthOfFieldEditorPlugin, Editor)` and linked only
//! by the editor bundle.

use bevy::post_process::dof::DepthOfField;
use bevy::prelude::*;
use egui_phosphor::regular;
use renzora::{AppEditorExt, InspectorEntry};
use renzora_dof::DepthOfFieldSettings;

fn inspector_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "depth_of_field",
        display_name: "Depth of Field",
        icon: regular::CAMERA,
        category: "rendering",
        has_fn: |world, entity| world.get::<DepthOfFieldSettings>(entity).is_some(),
        add_fn: Some(|world, entity| {
            world
                .entity_mut(entity)
                .insert(DepthOfFieldSettings::default());
        }),
        remove_fn: Some(|world, entity| {
            world
                .entity_mut(entity)
                .remove::<(DepthOfFieldSettings, DepthOfField)>();
        }),
        is_enabled_fn: Some(|world, entity| {
            world
                .get::<DepthOfFieldSettings>(entity)
                .map(|s| s.enabled)
                .unwrap_or(false)
        }),
        set_enabled_fn: Some(|world, entity, val| {
            if let Some(mut s) = world.get_mut::<DepthOfFieldSettings>(entity) {
                s.enabled = val;
            }
        }),
        // Declarative fields render natively (bevy_ui).
        fields: vec![
            renzora::enum_u32_field!("Mode", DepthOfFieldSettings, mode, ["Gaussian", "Bokeh"]),
            renzora::float_field!("Focal Distance", DepthOfFieldSettings, focal_distance, 0.1, 0.1, 1000.0),
            renzora::float_field!("Aperture", DepthOfFieldSettings, aperture_f_stops, 0.1, 0.1, 64.0),
            renzora::float_field!("Max CoC", DepthOfFieldSettings, max_circle_of_confusion_diameter, 1.0, 1.0, 256.0),
        ],
    }
}

/// Editor-scope companion to `renzora_dof::DepthOfFieldPlugin`.
#[derive(Default)]
pub struct DepthOfFieldEditorPlugin;

impl Plugin for DepthOfFieldEditorPlugin {
    fn build(&self, app: &mut App) {
        info!("[editor] DepthOfFieldEditorPlugin");
        app.register_inspector(inspector_entry());
    }
}

renzora::add!(DepthOfFieldEditorPlugin, Editor);
