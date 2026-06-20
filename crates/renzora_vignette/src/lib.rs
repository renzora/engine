//! Vignette — a thin wrapper around **Bevy 0.19's built-in `Vignette`**
//! (`bevy::post_process`), replacing renzora's old custom WGSL vignette.
//!
//! Follows the same settings→sync→camera pattern as the other bevy-built-in
//! wrappers (`renzora_ssr`, `renzora_bloom_effect`): `VignetteSettings` is
//! authored on a `WorldEnvironment`-style entity and routed onto cameras by
//! `sync_vignette`, which toggles bevy's `Vignette` component. The bevy
//! post-process effect stack (`PostProcessPlugin`) renders it.

use bevy::post_process::effect_stack::Vignette;
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

#[cfg(feature = "editor")]
use renzora::{AppEditorExt, InspectorEntry};

/// Authored vignette settings, routed to cameras as a bevy `Vignette`.
#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component, Serialize, Deserialize)]
pub struct VignetteSettings {
    pub enabled: bool,
    /// Strength of the darkening at the edges.
    pub intensity: f32,
    /// Radius (0..1) at which the vignette starts.
    pub radius: f32,
    /// Falloff softness from the radius to the corners.
    pub smoothness: f32,
    /// 0 = elliptical (follows aspect), 1 = circular.
    pub roundness: f32,
    /// Vignette tint (linear RGB; black is the classic look).
    pub color: Vec3,
    /// Compensates the darkening in the very corners.
    pub edge_compensation: f32,
}

impl Default for VignetteSettings {
    // Mirrors `bevy::post_process::Vignette::default()`.
    fn default() -> Self {
        Self {
            enabled: true,
            intensity: 1.0,
            radius: 0.75,
            smoothness: 5.0,
            roundness: 1.0,
            color: Vec3::ZERO,
            edge_compensation: 1.0,
        }
    }
}

fn vignette_from(s: &VignetteSettings) -> Vignette {
    Vignette {
        intensity: s.intensity,
        radius: s.radius,
        smoothness: s.smoothness,
        roundness: s.roundness,
        color: Color::srgb(s.color.x, s.color.y, s.color.z),
        edge_compensation: s.edge_compensation,
        center: Vec2::new(0.5, 0.5),
    }
}

fn sync_vignette(
    mut commands: Commands,
    sources: Query<(Entity, Ref<VignetteSettings>)>,
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
                    commands.entity(*target).insert(vignette_from(&settings));
                } else {
                    commands.entity(*target).remove::<Vignette>();
                }
                found = true;
                break;
            }
        }
        if !found && routing_changed {
            if let Ok(mut ec) = commands.get_entity(*target) {
                ec.remove::<Vignette>();
            }
        }
    }
}

fn cleanup_vignette(
    mut commands: Commands,
    mut removed: RemovedComponents<VignetteSettings>,
    routing: Res<renzora::EffectRouting>,
) {
    if removed.read().next().is_some() {
        for (target, _) in routing.iter() {
            if let Ok(mut ec) = commands.get_entity(*target) {
                ec.remove::<Vignette>();
            }
        }
    }
}

#[cfg(feature = "editor")]
fn inspector_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "vignette",
        display_name: "Vignette",
        icon: "aperture",
        category: "effects",
        has_fn: |world, entity| world.get::<VignetteSettings>(entity).is_some(),
        add_fn: Some(|world, entity| {
            world.entity_mut(entity).insert(VignetteSettings::default());
        }),
        remove_fn: Some(|world, entity| {
            world
                .entity_mut(entity)
                .remove::<(VignetteSettings, Vignette)>();
        }),
        is_enabled_fn: Some(|world, entity| {
            world
                .get::<VignetteSettings>(entity)
                .map(|s| s.enabled)
                .unwrap_or(false)
        }),
        set_enabled_fn: Some(|world, entity, val| {
            if let Some(mut s) = world.get_mut::<VignetteSettings>(entity) {
                s.enabled = val;
            }
        }),
        fields: vec![
            renzora::float_field!("Intensity", VignetteSettings, intensity, 0.01, 0.0, 5.0),
            renzora::float_field!("Radius", VignetteSettings, radius, 0.01, 0.0, 2.0),
            renzora::float_field!("Smoothness", VignetteSettings, smoothness, 0.05, 0.0, 20.0),
            renzora::float_field!("Roundness", VignetteSettings, roundness, 0.01, 0.0, 1.0),
            renzora::vec3_color_field!("Color", VignetteSettings, color),
            renzora::float_field!(
                "Edge Compensation",
                VignetteSettings,
                edge_compensation,
                0.01,
                0.0,
                2.0
            ),
        ],
    }
}

#[derive(Default)]
pub struct VignettePlugin;

impl Plugin for VignettePlugin {
    fn build(&self, app: &mut App) {
        info!("[runtime] VignettePlugin (bevy built-in)");
        app.register_type::<VignetteSettings>();
        app.add_systems(Update, (sync_vignette, cleanup_vignette));
        #[cfg(feature = "editor")]
        app.register_inspector(inspector_entry());
    }
}

renzora::add!(VignettePlugin);
