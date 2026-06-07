use bevy::post_process::bloom::{Bloom, BloomPrefilter};
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component, Serialize, Deserialize)]
pub struct BloomSettings {
    pub intensity: f32,
    pub low_frequency_boost: f32,
    pub high_pass_frequency: f32,
    pub threshold: f32,
    pub threshold_softness: f32,
    pub enabled: bool,
}

impl Default for BloomSettings {
    fn default() -> Self {
        // Tuned to match Bevy's `Bloom::NATURAL` preset more closely:
        // intensity 0.15 ≈ NATURAL, threshold 0.5 lets a bit more of the
        // mid-bright content bloom (the previous 0.8 was very strict and
        // killed the "emissive feel" on most lit surfaces).
        Self {
            intensity: 0.15,
            low_frequency_boost: 0.5,
            high_pass_frequency: 0.8,
            threshold: 0.5,
            threshold_softness: 0.3,
            enabled: true,
        }
    }
}

fn sync_bloom(
    mut commands: Commands,
    sources: Query<(Entity, Ref<BloomSettings>)>,
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
                    commands.entity(*target).insert(Bloom {
                        intensity: settings.intensity,
                        low_frequency_boost: settings.low_frequency_boost,
                        low_frequency_boost_curvature: 0.95,
                        high_pass_frequency: settings.high_pass_frequency,
                        prefilter: BloomPrefilter {
                            threshold: settings.threshold,
                            threshold_softness: settings.threshold_softness,
                        },
                        ..Bloom::NATURAL
                    });
                } else {
                    commands.entity(*target).remove::<Bloom>();
                }
                found = true;
                break;
            }
        }
        if !found && routing_changed {
            if let Ok(mut ec) = commands.get_entity(*target) {
                ec.remove::<Bloom>();
            }
        }
    }
}

fn cleanup_bloom(
    mut commands: Commands,
    mut removed: RemovedComponents<BloomSettings>,
    routing: Res<renzora::EffectRouting>,
) {
    if removed.read().next().is_some() {
        for (target, _) in routing.iter() {
            if let Ok(mut ec) = commands.get_entity(*target) {
                ec.remove::<Bloom>();
            }
        }
    }
}

#[derive(Default)]
pub struct BloomEffectPlugin;

impl Plugin for BloomEffectPlugin {
    fn build(&self, app: &mut App) {
        info!("[runtime] BloomEffectPlugin");
        app.register_type::<BloomSettings>();
        app.add_systems(Update, (sync_bloom, cleanup_bloom));
    }
}

renzora::add!(BloomEffectPlugin);
