//! The Auto Exposure inspector.
//!
//! Was a separate `Editor`-scope crate so the lean runtime half carried no
//! editor contract; as one `Runtime` native plugin it is a module again, loading
//! in the editor and in a copy-based export alike.

use bevy::post_process::auto_exposure::AutoExposure;
use bevy::prelude::*;
use renzora::{AppEditorExt, AutoExposureSettings, InspectorEntry};

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
            renzora::float_field!("Keep Night Dark", AutoExposureSettings, keep_dark_strength, 0.05, 0.0, 1.0),
            renzora::float_field!("Keep-Dark Pivot (EV)", AutoExposureSettings, keep_dark_pivot_ev, 0.1, -8.0, 16.0),
        ],
    }
}

/// Registered unconditionally from the one plugin's `build` — a native plugin
/// compiles with no cargo features, so a `cfg(feature = "editor")` gate would be
/// permanently false and this section would vanish with nothing logged.
pub(crate) fn register(app: &mut App) {
    app.register_inspector(inspector_entry());
}
