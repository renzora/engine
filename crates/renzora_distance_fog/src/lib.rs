use bevy::pbr::{DistanceFog, FogFalloff};
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// Fog falloff mode:
/// 0 = Linear, 1 = Exponential, 2 = ExponentialSquared, 3 = Atmospheric
#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component, Serialize, Deserialize)]
pub struct DistanceFogSettings {
    pub color_r: f32,
    pub color_g: f32,
    pub color_b: f32,
    pub directional_light_color_r: f32,
    pub directional_light_color_g: f32,
    pub directional_light_color_b: f32,
    pub directional_light_exponent: f32,
    /// 0=Linear, 1=Exponential, 2=ExponentialSquared, 3=Atmospheric
    pub mode: u32,
    pub start: f32,
    pub end: f32,
    pub density: f32,
    pub extinction_r: f32,
    pub extinction_g: f32,
    pub extinction_b: f32,
    pub inscattering_r: f32,
    pub inscattering_g: f32,
    pub inscattering_b: f32,
    pub enabled: bool,
}

impl Default for DistanceFogSettings {
    fn default() -> Self {
        Self {
            color_r: 0.72,
            color_g: 0.78,
            color_b: 0.9,
            directional_light_color_r: 1.0,
            directional_light_color_g: 0.92,
            directional_light_color_b: 0.75,
            directional_light_exponent: 12.0,
            mode: 3,
            start: 50.0,
            end: 800.0,
            density: 0.005,
            extinction_r: 0.006,
            extinction_g: 0.005,
            extinction_b: 0.004,
            inscattering_r: 0.008,
            inscattering_g: 0.01,
            inscattering_b: 0.014,
            enabled: true,
        }
    }
}

fn sync_distance_fog(
    mut commands: Commands,
    sources: Query<(Entity, Ref<DistanceFogSettings>)>,
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
                if !settings.enabled {
                    commands.entity(*target).remove::<DistanceFog>();
                    found = true;
                    break;
                }
                let falloff = match settings.mode {
                    1 => FogFalloff::Exponential {
                        density: settings.density,
                    },
                    2 => FogFalloff::ExponentialSquared {
                        density: settings.density,
                    },
                    3 => FogFalloff::Atmospheric {
                        extinction: Vec3::new(
                            settings.extinction_r,
                            settings.extinction_g,
                            settings.extinction_b,
                        ),
                        inscattering: Vec3::new(
                            settings.inscattering_r,
                            settings.inscattering_g,
                            settings.inscattering_b,
                        ),
                    },
                    _ => FogFalloff::Linear {
                        start: settings.start,
                        end: settings.end,
                    },
                };
                commands.entity(*target).insert(DistanceFog {
                    color: Color::srgb(settings.color_r, settings.color_g, settings.color_b),
                    directional_light_color: Color::srgb(
                        settings.directional_light_color_r,
                        settings.directional_light_color_g,
                        settings.directional_light_color_b,
                    ),
                    directional_light_exponent: settings.directional_light_exponent,
                    falloff,
                });
                found = true;
                break;
            }
        }
        if !found && routing_changed {
            if let Ok(mut ec) = commands.get_entity(*target) {
                ec.remove::<DistanceFog>();
            }
        }
    }
}

fn cleanup_distance_fog(
    mut commands: Commands,
    mut removed: RemovedComponents<DistanceFogSettings>,
    routing: Res<renzora::EffectRouting>,
) {
    if removed.read().next().is_some() {
        for (target, _) in routing.iter() {
            if let Ok(mut ec) = commands.get_entity(*target) {
                ec.remove::<DistanceFog>();
            }
        }
    }
}

#[derive(Default)]
pub struct DistanceFogPlugin;

impl Plugin for DistanceFogPlugin {
    fn build(&self, app: &mut App) {
        info!("[runtime] DistanceFogPlugin");
        app.register_type::<DistanceFogSettings>();
        app.add_systems(Update, (sync_distance_fog, cleanup_distance_fog));
    }
}

renzora::add!(DistanceFogPlugin);
