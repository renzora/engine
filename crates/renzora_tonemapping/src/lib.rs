use bevy::camera::Exposure;
use bevy::core_pipeline::tonemapping::{DebandDither, Tonemapping};
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component, Serialize, Deserialize)]
pub struct TonemappingSettings {
    /// 0=None, 1=Reinhard, 2=ReinhardLuminance, 3=AcesFitted,
    /// 4=AgX, 5=SomewhatBoring, 6=TonyMcMapface, 7=BlenderFilmic
    pub mode: u32,
    pub ev100: f32,
    pub enabled: bool,
}

impl Default for TonemappingSettings {
    fn default() -> Self {
        // TonyMcMapface (mode 6) — modern picture-formation algorithm
        // that preserves saturated highlights better than AgX or ACES.
        // It's also Bevy's default tonemapper for HDR cameras, so this
        // matches what users see before adding any tonemapping settings.
        Self {
            mode: 6,
            ev100: 9.7,
            enabled: true,
        }
    }
}

fn mode_to_tonemapping(mode: u32) -> Tonemapping {
    match mode {
        0 => Tonemapping::None,
        1 => Tonemapping::Reinhard,
        2 => Tonemapping::ReinhardLuminance,
        3 => Tonemapping::AcesFitted,
        4 => Tonemapping::AgX,
        5 => Tonemapping::SomewhatBoringDisplayTransform,
        6 => Tonemapping::TonyMcMapface,
        7 => Tonemapping::BlenderFilmic,
        _ => Tonemapping::TonyMcMapface,
    }
}

fn sync_tonemapping(
    mut commands: Commands,
    sources: Query<(Entity, Ref<TonemappingSettings>)>,
    routing: Res<renzora::EffectRouting>,
) {
    let routing_changed = routing.is_changed();
    for (target, source_list) in routing.iter() {
        for &src in source_list {
            if let Ok((_, settings)) = sources.get(src) {
                if !routing_changed && !settings.is_changed() {
                    break;
                }
                let tm = if settings.enabled {
                    mode_to_tonemapping(settings.mode)
                } else {
                    Tonemapping::None
                };
                commands.entity(*target).insert(tm).insert(Exposure {
                    ev100: settings.ev100,
                });
                break;
            }
        }
    }
}

// ── Deband Dither ──

#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component, Serialize, Deserialize)]
pub struct DebandDitherSettings {
    pub enabled: bool,
}

impl Default for DebandDitherSettings {
    fn default() -> Self {
        Self { enabled: true }
    }
}

fn sync_deband_dither(
    mut commands: Commands,
    sources: Query<(Entity, Ref<DebandDitherSettings>)>,
    routing: Res<renzora::EffectRouting>,
) {
    let routing_changed = routing.is_changed();
    for (target, source_list) in routing.iter() {
        for &src in source_list {
            if let Ok((_, settings)) = sources.get(src) {
                if !routing_changed && !settings.is_changed() {
                    break;
                }
                commands.entity(*target).insert(if settings.enabled {
                    DebandDither::Enabled
                } else {
                    DebandDither::Disabled
                });
                break;
            }
        }
    }
}

fn cleanup_deband_dither(
    mut commands: Commands,
    mut removed: RemovedComponents<DebandDitherSettings>,
    routing: Res<renzora::EffectRouting>,
) {
    if removed.read().next().is_some() {
        for (target, _) in routing.iter() {
            if let Ok(mut ec) = commands.get_entity(*target) {
                ec.insert(DebandDither::Disabled);
            }
        }
    }
}

fn cleanup_tonemapping(
    mut commands: Commands,
    mut removed: RemovedComponents<TonemappingSettings>,
    routing: Res<renzora::EffectRouting>,
) {
    if removed.read().next().is_some() {
        for (target, _) in routing.iter() {
            if let Ok(mut ec) = commands.get_entity(*target) {
                ec.insert((Tonemapping::default(), Exposure::default()));
            }
        }
    }
}

#[derive(Default)]
pub struct TonemappingPlugin;

impl Plugin for TonemappingPlugin {
    fn build(&self, app: &mut App) {
        info!("[runtime] TonemappingPlugin");
        app.register_type::<TonemappingSettings>();
        app.register_type::<DebandDitherSettings>();
        app.add_systems(
            Update,
            (
                sync_tonemapping,
                cleanup_tonemapping,
                sync_deband_dither,
                cleanup_deband_dither,
            ),
        );
    }
}

renzora::add!(TonemappingPlugin);
