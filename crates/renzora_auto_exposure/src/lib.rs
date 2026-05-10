//! Renzora's own auto-exposure: CPU-side EV driver fed by the scene
//! luminance readback.
//!
//! Bevy's `bevy_post_process::auto_exposure` works fine but its
//! `AutoExposureCompensationCurve` is `RenderAssetUsages::RENDER_WORLD`
//! only, which means after first extract the main-world copy is dropped
//! and any subsequent re-extraction logs a loud `cannot be extracted`
//! error. Rather than fight that, we drive `Camera::Exposure.ev100`
//! ourselves on the CPU using the luminance readback we already have.
//!
//! Pipeline:
//!   1. `LuminanceReadbackPlugin` (in `luminance.rs`) dispatches a tiny
//!      compute reduce on the view target, async-maps the result, and
//!      writes the raw average log-luminance into `SceneLuminance`.
//!   2. `drive_auto_exposure` reads that, computes a target EV that
//!      compensates the average toward 18% middle gray, smooths
//!      towards it at the user's brighten/darken speeds, clamps to the
//!      configured range, and writes the result into the camera's
//!      `bevy::camera::Exposure` component (inserting one if needed).
//!   3. `CameraExposureState.ev100` is mirrored from the live value so
//!      scripting (`camera_ev` Lua global) can display it.

use bevy::camera::Exposure;
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

mod luminance;
pub use luminance::{LuminanceReadbackPlugin, SceneLuminance};

#[cfg(feature = "editor")]
use {
    bevy_egui::egui,
    egui_phosphor::regular,
    renzora_editor::{inline_property, AppEditorExt, EditorCommands, InspectorEntry},
    renzora_theme::Theme,
};

#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component, Serialize, Deserialize)]
pub struct AutoExposureSettings {
    pub speed_brighten: f32,
    pub speed_darken: f32,
    pub range_min: f32,
    pub range_max: f32,
    pub enabled: bool,
}

impl Default for AutoExposureSettings {
    fn default() -> Self {
        Self {
            speed_brighten: 2.0,
            speed_darken: 1.0,
            range_min: -2.0,
            range_max: 4.0,
            enabled: true,
        }
    }
}

/// Drives `Exposure.ev100` on every routed target camera based on the
/// scene luminance readback.
fn drive_auto_exposure(
    time: Res<Time>,
    lum: Res<SceneLuminance>,
    sources: Query<&AutoExposureSettings>,
    routing: Res<renzora::EffectRouting>,
    mut commands: Commands,
    mut exposures: Query<&mut Exposure>,
    mut state: ResMut<renzora::core::CameraExposureState>,
) {
    if !lum.valid {
        return;
    }

    // Target EV that maps the scene's average luminance to ~18% middle
    // gray under Bevy's `exposure() = exp2(-ev) / 1.2` formula. The
    // constant is `-log2(0.18 * 1.2)` ≈ 2.21.
    const MID_GRAY_BIAS: f32 = 2.21;
    let raw_target = lum.avg_log_lum + MID_GRAY_BIAS;
    let dt = time.delta_secs().min(0.1);

    for (target, source_list) in routing.iter() {
        let settings = source_list
            .iter()
            .find_map(|&src| sources.get(src).ok())
            .filter(|s| s.enabled);
        let Some(settings) = settings else { continue; };

        let target_ev = raw_target.clamp(settings.range_min, settings.range_max);

        let current_ev = exposures
            .get(*target)
            .map(|e| e.ev100)
            // First-frame fallback: skip the smoothing transient by
            // starting at the target.
            .unwrap_or(target_ev);

        let diff = target_ev - current_ev;
        let speed = if diff >= 0.0 {
            settings.speed_brighten
        } else {
            settings.speed_darken
        };
        let step = diff.clamp(-speed * dt, speed * dt);
        let new_ev = current_ev + step;

        if let Ok(mut exposure) = exposures.get_mut(*target) {
            if (exposure.ev100 - new_ev).abs() > 1e-4 {
                exposure.ev100 = new_ev;
            }
        } else {
            commands.entity(*target).insert(Exposure { ev100: new_ev });
        }
        state.ev100 = new_ev;
    }
}

/// Strips `Exposure` overrides from cameras whose source no longer has
/// `AutoExposureSettings` (component removed via inspector). Without
/// this the camera would stick at the last AE-driven EV instead of
/// falling back to its baseline.
fn cleanup_auto_exposure(
    mut commands: Commands,
    mut removed: RemovedComponents<AutoExposureSettings>,
    routing: Res<renzora::EffectRouting>,
    sources: Query<&AutoExposureSettings>,
) {
    if removed.read().next().is_none() {
        return;
    }
    for (target, source_list) in routing.iter() {
        let still_has = source_list
            .iter()
            .any(|&src| sources.get(src).is_ok());
        if !still_has {
            commands.entity(*target).remove::<Exposure>();
        }
    }
}

#[cfg(feature = "editor")]
fn auto_exposure_entry() -> InspectorEntry {
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
            world.entity_mut(entity).remove::<AutoExposureSettings>();
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
        fields: vec![],
        custom_ui_fn: Some(auto_exposure_custom_ui),
    }
}

#[cfg(feature = "editor")]
fn auto_exposure_custom_ui(
    ui: &mut egui::Ui,
    world: &World,
    entity: Entity,
    cmds: &EditorCommands,
    theme: &Theme,
) {
    let Some(settings) = world.get::<AutoExposureSettings>(entity) else {
        return;
    };
    let mut data = settings.clone();
    let mut row = 0;
    let mut changed = false;

    inline_property(ui, row, "Speed Brighten", theme, |ui| {
        let orig = data.speed_brighten;
        ui.add(egui::DragValue::new(&mut data.speed_brighten).speed(0.1).range(0.0..=10.0));
        if data.speed_brighten != orig {
            changed = true;
        }
    });
    row += 1;
    inline_property(ui, row, "Speed Darken", theme, |ui| {
        let orig = data.speed_darken;
        ui.add(egui::DragValue::new(&mut data.speed_darken).speed(0.1).range(0.0..=10.0));
        if data.speed_darken != orig {
            changed = true;
        }
    });
    row += 1;
    inline_property(ui, row, "Range Min (EV)", theme, |ui| {
        let orig = data.range_min;
        ui.add(egui::DragValue::new(&mut data.range_min).speed(0.1).range(-16.0..=8.0));
        if data.range_min != orig {
            changed = true;
        }
    });
    row += 1;
    inline_property(ui, row, "Range Max (EV)", theme, |ui| {
        let orig = data.range_max;
        ui.add(egui::DragValue::new(&mut data.range_max).speed(0.1).range(-8.0..=16.0));
        if data.range_max != orig {
            changed = true;
        }
    });

    if changed {
        cmds.push(move |world: &mut World| {
            if let Some(mut s) = world.get_mut::<AutoExposureSettings>(entity) {
                *s = data;
            }
        });
    }
}

#[derive(Default)]
pub struct AutoExposurePlugin;

impl Plugin for AutoExposurePlugin {
    fn build(&self, app: &mut App) {
        info!("[runtime] AutoExposurePlugin");
        app.add_plugins(LuminanceReadbackPlugin);
        app.init_resource::<renzora::core::CameraExposureState>();
        app.register_type::<AutoExposureSettings>();
        app.add_systems(Update, (drive_auto_exposure, cleanup_auto_exposure));
        #[cfg(feature = "editor")]
        app.register_inspector(auto_exposure_entry());
    }
}

renzora::add!(AutoExposurePlugin);
