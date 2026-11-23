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
        // Lights bloom, lit surfaces do not. At threshold 0.5 a sunlit wall
        // bloomed too, which lifted the whole frame into a haze; 1.5 keeps
        // the glow to lamps, bulbs and signs. Intensity 0.2: at 0.3 the halos
        // fogged the frame at night, once auto exposure opened up.
        Self {
            intensity: 0.2,
            low_frequency_boost: 0.5,
            high_pass_frequency: 0.8,
            threshold: 1.5,
            threshold_softness: 0.3,
            enabled: true,
        }
    }
}

fn sync_bloom(
    mut commands: Commands,
    sources: Query<(Entity, Ref<BloomSettings>)>,
    routing: Res<renzora_scene_api::EffectRouting>,
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
    routing: Res<renzora_scene_api::EffectRouting>,
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

        // HDR render passes run before bloom, not in whatever order the
        // schedule happens to pick: both sit before tonemapping in
        // `PostProcess`. The sun glare pass is what needs it. It hands bloom a
        // frame with the sun disc's overflowing pixels brought back into
        // range, and bloom that ran first read those infinities, turned them
        // into NaN and flashed every few frames.
        if let Some(render_app) = app.get_sub_app_mut(bevy::render::RenderApp) {
            render_app.configure_sets(
                bevy::core_pipeline::Core3d,
                renzora_scene_api::RenderPhase::HdrPost.before(bevy::post_process::bloom::bloom),
            );
        }
    }
}
