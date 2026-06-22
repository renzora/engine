//! Editor-only half of `renzora_auto_exposure` — the Auto Exposure inspector.
//!
//! `renzora_auto_exposure` compiles lean (no `editor` feature, no
//! egui-phosphor). This crate holds the inspector (renzora editor contract +
//! Phosphor icon), registered `renzora::add!(AutoExposureEditorPlugin, Editor)`
//! and linked only by the editor bundle.

use bevy::post_process::auto_exposure::AutoExposure;
use bevy::prelude::*;
use renzora::{AppEditorExt, InspectorEntry};
use renzora_auto_exposure::AutoExposureSettings;

fn inspector_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "auto_exposure",
        display_name: "Auto Exposure",
        icon: "sun",
        category: "camera",
        has_fn: |world, entity| world.get::<AutoExposureSettings>(entity).is_some(),
        add_fn: Some(|world, entity| {
            world
                .entity_mut(entity)
                .insert(AutoExposureSettings::default());
        }),
        remove_fn: Some(|world, entity| {
            world
                .entity_mut(entity)
                .remove::<(AutoExposureSettings, AutoExposure)>();
        }),
        is_enabled_fn: Some(|world, entity| {
            world
                .get::<AutoExposureSettings>(entity)
                .map(|s| s.enabled)
                .unwrap_or(false)
        }),
        set_enabled_fn: Some(|world, entity, val| {
            if let Some(mut s) = world.get_mut::<AutoExposureSettings>(entity) {
                s.enabled = val;
            }
        }),
        // Declarative fields render natively (bevy_ui).
        fields: vec![
            renzora::float_field!("Speed Brighten", AutoExposureSettings, speed_brighten, 0.1, 0.0, 10.0),
            renzora::float_field!("Speed Darken", AutoExposureSettings, speed_darken, 0.1, 0.0, 10.0),
            renzora::float_field!("Range Min (EV)", AutoExposureSettings, range_min, 0.1, -16.0, 8.0),
            renzora::float_field!("Range Max (EV)", AutoExposureSettings, range_max, 0.1, -8.0, 16.0),
            renzora::float_field!("Filter Low (%)", AutoExposureSettings, filter_low, 0.01, 0.0, 0.5),
            renzora::float_field!("Filter High (%)", AutoExposureSettings, filter_high, 0.01, 0.5, 1.0),
            renzora::float_field!("Anti-Jitter Band", AutoExposureSettings, exponential_transition_distance, 0.05, 0.0, 5.0),
        ],
    }
}

/// Editor-scope companion to `renzora_auto_exposure::AutoExposurePlugin`.
#[derive(Default)]
pub struct AutoExposureEditorPlugin;

impl Plugin for AutoExposureEditorPlugin {
    fn build(&self, app: &mut App) {
        info!("[editor] AutoExposureEditorPlugin");
        app.register_inspector(inspector_entry());
    }
}

renzora::add!(AutoExposureEditorPlugin, Editor);
