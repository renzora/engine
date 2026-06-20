//! Lens distortion — a thin wrapper around **Bevy 0.19's built-in
//! `LensDistortion`** (`bevy::post_process::effect_stack`). Same
//! settings→sync→camera pattern as `renzora_ssr` / `renzora_vignette`:
//! `LensDistortionSettings` is authored on a `WorldEnvironment`-style entity and
//! routed onto cameras, where bevy's post-process effect stack renders it.

use bevy::post_process::effect_stack::LensDistortion;
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// Authored lens-distortion settings, routed to cameras as a bevy
/// `LensDistortion`.
#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component, Serialize, Deserialize)]
pub struct LensDistortionSettings {
    pub enabled: bool,
    /// Positive = **barrel** distortion (bulging out), negative = **pincushion**
    /// (pinching in). Roughly the radial coefficient `k1`.
    pub intensity: f32,
    /// Zoom factor that crops the stretched screen edges (1.0 = no zoom).
    pub scale: f32,
}

impl Default for LensDistortionSettings {
    // Mirrors `bevy::post_process::effect_stack::LensDistortion::default()` for the
    // exposed fields; the multiplier/center/edge-curvature keep bevy's defaults.
    fn default() -> Self {
        Self {
            enabled: true,
            intensity: 0.5,
            scale: 1.0,
        }
    }
}

fn lens_from(s: &LensDistortionSettings) -> LensDistortion {
    LensDistortion {
        intensity: s.intensity,
        scale: s.scale,
        ..default()
    }
}

fn sync_lens_distortion(
    mut commands: Commands,
    sources: Query<(Entity, Ref<LensDistortionSettings>)>,
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
                    commands.entity(*target).insert(lens_from(&settings));
                } else {
                    commands.entity(*target).remove::<LensDistortion>();
                }
                found = true;
                break;
            }
        }
        if !found && routing_changed {
            if let Ok(mut ec) = commands.get_entity(*target) {
                ec.remove::<LensDistortion>();
            }
        }
    }
}

fn cleanup_lens_distortion(
    mut commands: Commands,
    mut removed: RemovedComponents<LensDistortionSettings>,
    routing: Res<renzora::EffectRouting>,
) {
    if removed.read().next().is_some() {
        for (target, _) in routing.iter() {
            if let Ok(mut ec) = commands.get_entity(*target) {
                ec.remove::<LensDistortion>();
            }
        }
    }
}

#[derive(Default)]
pub struct LensDistortionPlugin;

impl Plugin for LensDistortionPlugin {
    fn build(&self, app: &mut App) {
        info!("[runtime] LensDistortionPlugin (bevy built-in)");
        app.register_type::<LensDistortionSettings>();
        app.add_systems(Update, (sync_lens_distortion, cleanup_lens_distortion));
    }
}

renzora::add!(LensDistortionPlugin);
