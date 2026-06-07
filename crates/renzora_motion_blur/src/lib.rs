use bevy::post_process::motion_blur::MotionBlur;
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component, Serialize, Deserialize)]
pub struct MotionBlurSettings {
    pub shutter_angle: f32,
    pub samples: f32,
    pub enabled: bool,
}

impl Default for MotionBlurSettings {
    fn default() -> Self {
        Self {
            shutter_angle: 0.5,
            samples: 2.0,
            enabled: true,
        }
    }
}

fn sync_motion_blur(
    mut commands: Commands,
    sources: Query<(Entity, Ref<MotionBlurSettings>)>,
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
                    commands.entity(*target).insert(MotionBlur {
                        shutter_angle: settings.shutter_angle,
                        samples: settings.samples as u32,
                    });
                } else {
                    commands.entity(*target).remove::<MotionBlur>();
                }
                found = true;
                break;
            }
        }
        if !found && routing_changed {
            if let Ok(mut ec) = commands.get_entity(*target) {
                ec.remove::<MotionBlur>();
            }
        }
    }
}

fn cleanup_motion_blur(
    mut commands: Commands,
    mut removed: RemovedComponents<MotionBlurSettings>,
    routing: Res<renzora::EffectRouting>,
) {
    if removed.read().next().is_some() {
        for (target, _) in routing.iter() {
            if let Ok(mut ec) = commands.get_entity(*target) {
                ec.remove::<MotionBlur>();
            }
        }
    }
}

#[derive(Default)]
pub struct MotionBlurPlugin;

impl Plugin for MotionBlurPlugin {
    fn build(&self, app: &mut App) {
        info!("[runtime] MotionBlurPlugin");
        app.register_type::<MotionBlurSettings>();
        app.add_systems(Update, (sync_motion_blur, cleanup_motion_blur));
    }
}

renzora::add!(MotionBlurPlugin);
