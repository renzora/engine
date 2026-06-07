//! Renzora Auto Exposure — settings wrapper for Bevy's
//! `bevy_post_process::auto_exposure::AutoExposure`.
//!
//! Bevy's AE is histogram-based with percentile filtering: it builds a
//! 64-bin luminance histogram each frame, ignores the darkest N% and
//! brightest M% of pixels, and animates the camera's exposure so the
//! remaining "metered" pixels average to middle gray. This handles
//! dark/sparse scenes properly (the old log-average implementation
//! would max out exposure when the scene was mostly empty, blowing the
//! frame to white).
//!
//! This crate just authors user-facing settings on a `WorldEnvironment`
//! source entity and routes them onto every camera via `EffectRouting`.
//! The compute shader, smoothing, percentile filter, and exponential
//! anti-jitter all live in Bevy.
//!
//! Note: Bevy's AE doesn't add itself via `DefaultPlugins` — its
//! `AutoExposurePlugin` is opt-in. We add it from our `build` hook so
//! enabling AutoExposureSettings on any entity Just Works.

use bevy::camera::Exposure;
use bevy::post_process::auto_exposure::{AutoExposure, AutoExposurePlugin as BevyAePlugin};
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

#[cfg(feature = "editor")]
use {
    egui_phosphor::regular,
    renzora::{AppEditorExt, InspectorEntry},
};

#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component, Serialize, Deserialize)]
pub struct AutoExposureSettings {
    /// How fast the camera adapts to brighter scenes, in F-stops/second.
    /// Bevy's default is 3.0 (eye adapts to bright quickly).
    pub speed_brighten: f32,
    /// How fast the camera adapts to darker scenes, in F-stops/second.
    /// Bevy's default is 1.0 (eye adapts to dark slowly).
    pub speed_darken: f32,
    /// Minimum EV the metering can drive towards. Bevy default: -8.
    pub range_min: f32,
    /// Maximum EV the metering can drive towards. Bevy default: +8.
    pub range_max: f32,
    /// Lower percentile cutoff (0..1). Pixels darker than this fraction
    /// of the histogram are excluded from metering. 0.10 = ignore the
    /// darkest 10%. This is what stops a dark/empty scene from pulling
    /// the average toward zero and blowing the frame to white.
    pub filter_low: f32,
    /// Upper percentile cutoff. 0.90 = ignore brightest 10% (specular
    /// highlights, sun disk, etc.).
    pub filter_high: f32,
    /// Anti-jitter band in F-stops. Small frame-to-frame changes within
    /// this band animate exponentially (slow, smooth); larger changes
    /// use the linear `speed_*` rates. 1.5 = Bevy default.
    pub exponential_transition_distance: f32,
    pub enabled: bool,
}

impl Default for AutoExposureSettings {
    fn default() -> Self {
        // Mirrors Bevy's `AutoExposure::default()` field-for-field —
        // these are the values the Bevy team picked after testing
        // against real scenes.
        Self {
            speed_brighten: 3.0,
            speed_darken: 1.0,
            range_min: -8.0,
            range_max: 8.0,
            filter_low: 0.10,
            filter_high: 0.90,
            exponential_transition_distance: 1.5,
            enabled: true,
        }
    }
}

/// Route AutoExposureSettings from a `WorldEnvironment` source onto
/// every active camera. When disabled, the Bevy `AutoExposure`
/// component is removed and the camera reverts to its static
/// `Exposure.ev100` (typically driven by `TonemappingSettings`).
fn sync_auto_exposure(
    mut commands: Commands,
    sources: Query<(Entity, Ref<AutoExposureSettings>)>,
    routing: Res<renzora::EffectRouting>,
) {
    let routing_changed = routing.is_changed();
    for (target, source_list) in routing.iter() {
        let mut found = false;
        for &src in source_list {
            if let Ok((_, settings)) = sources.get(src) {
                if !routing_changed && !settings.is_changed() {
                    found = true;
                    break;
                }
                if settings.enabled {
                    commands.entity(*target).insert(AutoExposure {
                        range: settings.range_min..=settings.range_max,
                        filter: settings.filter_low..=settings.filter_high,
                        speed_brighten: settings.speed_brighten,
                        speed_darken: settings.speed_darken,
                        exponential_transition_distance: settings.exponential_transition_distance,
                        ..default()
                    });
                } else {
                    commands.entity(*target).remove::<AutoExposure>();
                }
                found = true;
                break;
            }
        }
        if !found && routing_changed {
            if let Ok(mut ec) = commands.get_entity(*target) {
                ec.remove::<AutoExposure>();
            }
        }
    }
}

fn cleanup_auto_exposure(
    mut commands: Commands,
    mut removed: RemovedComponents<AutoExposureSettings>,
    routing: Res<renzora::EffectRouting>,
) {
    if removed.read().next().is_some() {
        for (target, _) in routing.iter() {
            if let Ok(mut ec) = commands.get_entity(*target) {
                ec.remove::<AutoExposure>();
            }
        }
    }
}

/// Mirror the camera's `Exposure.ev100` into `CameraExposureState` so
/// scripting (`camera_ev` Lua/Rhai global) and HUDs can display it.
///
/// Caveat: with Bevy's AE active, the metering result lives in a GPU
/// state buffer, not in `Exposure.ev100` — that component carries the
/// pre-AE baseline (typically what `TonemappingSettings` set). So this
/// reading is the *manual* EV, not the live AE-adjusted value. Fixing
/// that would require a GPU→CPU readback of Bevy's internal state,
/// which is non-trivial; the manual EV is good enough for HUD use.
fn mirror_camera_ev(
    cameras: Query<&Exposure, With<Camera3d>>,
    mut state: ResMut<renzora::core::CameraExposureState>,
) {
    if let Some(exposure) = cameras.iter().next() {
        if (state.ev100 - exposure.ev100).abs() > 1e-4 {
            state.ev100 = exposure.ev100;
        }
    }
}

#[cfg(feature = "editor")]
fn inspector_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "auto_exposure",
        display_name: "Auto Exposure",
        icon: regular::SUN,
        category: "rendering",
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

#[derive(Default)]
pub struct AutoExposurePlugin;

impl Plugin for AutoExposurePlugin {
    fn build(&self, app: &mut App) {
        info!("[runtime] AutoExposurePlugin");
        // Bevy's AutoExposurePlugin is opt-in (not part of DefaultPlugins).
        // Adding it here means `AutoExposure` components are actually
        // honored by the render graph.
        if !app.is_plugin_added::<BevyAePlugin>() {
            app.add_plugins(BevyAePlugin);
        }
        app.init_resource::<renzora::core::CameraExposureState>();
        app.register_type::<AutoExposureSettings>();
        app.add_systems(
            Update,
            (sync_auto_exposure, cleanup_auto_exposure, mirror_camera_ev),
        );
        #[cfg(feature = "editor")]
        app.register_inspector(auto_exposure_entry());
    }
}

#[cfg(feature = "editor")]
fn auto_exposure_entry() -> InspectorEntry {
    inspector_entry()
}

renzora::add!(AutoExposurePlugin);
