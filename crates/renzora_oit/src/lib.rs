use bevy::core_pipeline::oit::OrderIndependentTransparencySettings;
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component, Serialize, Deserialize)]
pub struct OitSettings {
    pub layer_count: i32,
    pub alpha_threshold: f32,
    pub enabled: bool,
}

impl Default for OitSettings {
    fn default() -> Self {
        Self {
            layer_count: 8,
            alpha_threshold: 0.0,
            enabled: true,
        }
    }
}

fn sync_oit(
    mut commands: Commands,
    sources: Query<(Entity, Ref<OitSettings>)>,
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
                    commands.entity(*target).insert(Msaa::Off).insert(
                        OrderIndependentTransparencySettings {
                            layer_count: settings.layer_count,
                            alpha_threshold: settings.alpha_threshold,
                        },
                    );
                } else {
                    commands
                        .entity(*target)
                        .remove::<OrderIndependentTransparencySettings>();
                }
                found = true;
                break;
            }
        }
        if !found && routing_changed {
            if let Ok(mut ec) = commands.get_entity(*target) {
                ec.remove::<OrderIndependentTransparencySettings>();
            }
        }
    }
}

fn cleanup_oit(
    mut commands: Commands,
    mut removed: RemovedComponents<OitSettings>,
    routing: Res<renzora::EffectRouting>,
) {
    if removed.read().next().is_some() {
        for (target, _) in routing.iter() {
            if let Ok(mut ec) = commands.get_entity(*target) {
                ec.remove::<OrderIndependentTransparencySettings>();
            }
        }
    }
}

#[derive(Default)]
pub struct OitPlugin;

impl Plugin for OitPlugin {
    fn build(&self, app: &mut App) {
        info!("[runtime] OitPlugin");
        app.register_type::<OitSettings>();
        app.add_systems(Update, (sync_oit, cleanup_oit));
    }
}

renzora::add!(OitPlugin);
