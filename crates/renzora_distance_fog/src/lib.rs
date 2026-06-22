//! Distance fog — the slice-1 piece of the `WorldEnvironment` reconcile.
//!
//! Fog is mesh-view **binding 13** in PBR's shared layout, so toggling the
//! `DistanceFog` component's *presence* at runtime restructures that layout and
//! crashes wgpu. Instead, `DistanceFog` is attached **resident** at camera spawn
//! (see `renzora_engine::camera`), and [`reconcile_world_environment`] only ever
//! *updates* it from [`renzora::WorldEnvironment`]'s fog section — disabled = a
//! no-op falloff, never a removal. See `docs/world-environment-spec.md`.

use bevy::pbr::{DistanceFog, FogFalloff};
use bevy::prelude::*;
use renzora::{FogSection, WorldEnvironment};
use serde::{Deserialize, Serialize};

/// Legacy per-effect fog settings. Superseded by [`WorldEnvironment::fog`]; kept
/// (registered, but no longer synced or shown in the inspector) so scenes saved
/// before the unification still deserialize. Migration to `WorldEnvironment`
/// lands with the later slices.
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

/// The resident "no fog" value: present in the layout (binding 13), produces
/// nothing. Density 0 → zero fog factor everywhere.
fn no_fog() -> DistanceFog {
    DistanceFog {
        color: Color::NONE,
        directional_light_color: Color::NONE,
        directional_light_exponent: 8.0,
        falloff: FogFalloff::Exponential { density: 0.0 },
    }
}

/// Build a `DistanceFog` from an enabled fog section.
fn fog_from(section: &FogSection) -> DistanceFog {
    let falloff = match section.mode {
        1 => FogFalloff::Exponential {
            density: section.density,
        },
        2 => FogFalloff::ExponentialSquared {
            density: section.density,
        },
        3 => FogFalloff::Atmospheric {
            extinction: Vec3::from_array(section.extinction),
            inscattering: Vec3::from_array(section.inscattering),
        },
        _ => FogFalloff::Linear {
            start: section.start,
            end: section.end,
        },
    };
    DistanceFog {
        color: Color::srgb(section.color[0], section.color[1], section.color[2]),
        directional_light_color: Color::srgb(
            section.directional_light_color[0],
            section.directional_light_color[1],
            section.directional_light_color[2],
        ),
        directional_light_exponent: section.directional_light_exponent,
        falloff,
    }
}

/// Slice-1 of the single `reconcile_world_environment` writer: drive the
/// **resident** `DistanceFog` on every routed camera from `WorldEnvironment`'s
/// fog section. Never inserts/removes `DistanceFog` — disabled is a no-op
/// falloff, so toggling fog can't restructure PBR's mesh-view layout (the wgpu
/// crash class). Cameras without a `DistanceFog` (non-viewport utility cameras)
/// are simply skipped.
fn reconcile_world_environment(
    mut fogs: Query<&mut DistanceFog>,
    sources: Query<Ref<WorldEnvironment>>,
    routing: Res<renzora::EffectRouting>,
) {
    let routing_changed = routing.is_changed();
    for (target, source_list) in routing.iter() {
        // First routed source carrying a WorldEnvironment wins.
        let Some(env) = source_list.iter().find_map(|&s| sources.get(s).ok()) else {
            continue;
        };
        // Cheap reconcile: only rewrite the camera on an actual change.
        if !routing_changed && !env.is_changed() {
            continue;
        }
        if let Ok(mut fog) = fogs.get_mut(*target) {
            *fog = if env.fog.enabled {
                fog_from(&env.fog)
            } else {
                no_fog()
            };
        }
    }
}

#[derive(Default)]
pub struct DistanceFogPlugin;

impl Plugin for DistanceFogPlugin {
    fn build(&self, app: &mut App) {
        info!("[runtime] DistanceFogPlugin");
        app.register_type::<DistanceFogSettings>();
        app.add_systems(Update, reconcile_world_environment);
    }
}

renzora::add!(DistanceFogPlugin);
