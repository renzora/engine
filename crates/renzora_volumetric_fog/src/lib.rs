//! Renzora Volumetric Fog — settings wrapper for Bevy's `VolumetricFog`.
//!
//! Renders ambient volumetric scattering (a soft global haze) and lets
//! lights tagged `bevy::pbr::VolumetricLight` cast god rays through it.
//! Bevy's `VolumetricFogPlugin` (part of `DefaultPlugins`) does the
//! actual rendering; this crate just authors user-facing settings on a
//! `WorldEnvironment`-style entity and routes them onto each active
//! camera via `EffectRouting`.
//!
//! Pair with `VolumetricLight` on a directional / point / spot light
//! to get visible sunbeams and light shafts.

use bevy::light::VolumetricFog;
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

#[cfg(feature = "editor")]
use {
    bevy_egui::egui,
    egui_phosphor::regular,
    renzora_editor::{inline_property, AppEditorExt, EditorCommands, InspectorEntry},
    renzora_theme::Theme,
};

#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component, Serialize, Deserialize)]
pub struct VolumetricFogSettings {
    /// Color of the ambient scattering. Slight blue mimics atmospheric
    /// Rayleigh scattering; pure white reads as fog.
    pub ambient_color: (f32, f32, f32),
    /// Strength of the ambient haze. 0 = invisible, 1 = strong.
    pub ambient_intensity: f32,
    /// Raymarch step count along each view ray. Higher = smoother god
    /// rays at higher cost. 64 is Bevy's default; 32 reads as cheaper
    /// but slightly banded, 96+ as crisp at noticeable perf cost.
    pub step_count: u32,
    /// Per-frame jitter that hides banding artifacts when paired with
    /// TAA. 0 = banded but stable, 1 = maximum jitter (TAA hides it).
    pub jitter: f32,
    pub enabled: bool,
}

impl Default for VolumetricFogSettings {
    fn default() -> Self {
        // Subtle defaults — gives a hint of haze + lets VolumetricLight
        // sources (sun, point lights) cast god rays through it without
        // overwhelming the scene out-of-the-box.
        Self {
            ambient_color: (0.55, 0.6, 0.7),
            ambient_intensity: 0.08,
            step_count: 64,
            jitter: 0.5,
            enabled: true,
        }
    }
}

fn sync_volumetric_fog(
    mut commands: Commands,
    sources: Query<(Entity, Ref<VolumetricFogSettings>)>,
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
                    let (r, g, b) = settings.ambient_color;
                    commands.entity(*target).insert(VolumetricFog {
                        ambient_color: Color::srgb(r, g, b),
                        ambient_intensity: settings.ambient_intensity,
                        step_count: settings.step_count,
                        jitter: settings.jitter,
                        ..default()
                    });
                } else {
                    commands.entity(*target).remove::<VolumetricFog>();
                }
                found = true;
                break;
            }
        }
        if !found && routing_changed {
            if let Ok(mut ec) = commands.get_entity(*target) {
                ec.remove::<VolumetricFog>();
            }
        }
    }
}

fn cleanup_volumetric_fog(
    mut commands: Commands,
    mut removed: RemovedComponents<VolumetricFogSettings>,
    routing: Res<renzora::EffectRouting>,
) {
    if removed.read().next().is_some() {
        for (target, _) in routing.iter() {
            if let Ok(mut ec) = commands.get_entity(*target) {
                ec.remove::<VolumetricFog>();
            }
        }
    }
}

#[cfg(feature = "editor")]
fn inspector_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "volumetric_fog",
        display_name: "Volumetric Fog",
        icon: regular::CLOUD_FOG,
        category: "environment",
        has_fn: |world, entity| world.get::<VolumetricFogSettings>(entity).is_some(),
        add_fn: Some(|world, entity| {
            world
                .entity_mut(entity)
                .insert(VolumetricFogSettings::default());
        }),
        remove_fn: Some(|world, entity| {
            world
                .entity_mut(entity)
                .remove::<(VolumetricFogSettings, VolumetricFog)>();
        }),
        is_enabled_fn: Some(|world, entity| {
            world
                .get::<VolumetricFogSettings>(entity)
                .map(|s| s.enabled)
                .unwrap_or(false)
        }),
        set_enabled_fn: Some(|world, entity, val| {
            if let Some(mut s) = world.get_mut::<VolumetricFogSettings>(entity) {
                s.enabled = val;
            }
        }),
        fields: vec![],
        custom_ui_fn: Some(volumetric_fog_custom_ui),
    }
}

#[cfg(feature = "editor")]
fn volumetric_fog_custom_ui(
    ui: &mut egui::Ui,
    world: &World,
    entity: Entity,
    cmds: &EditorCommands,
    theme: &Theme,
) {
    let Some(settings) = world.get::<VolumetricFogSettings>(entity) else {
        return;
    };

    let mut row = 0;

    let (r, g, b) = settings.ambient_color;
    inline_property(ui, row, "Color", theme, |ui| {
        let mut color = egui::Color32::from_rgb(
            (r * 255.0) as u8,
            (g * 255.0) as u8,
            (b * 255.0) as u8,
        );
        if ui.color_edit_button_srgba(&mut color).changed() {
            let new_color = (
                color.r() as f32 / 255.0,
                color.g() as f32 / 255.0,
                color.b() as f32 / 255.0,
            );
            cmds.push(move |world: &mut World| {
                if let Some(mut s) = world.get_mut::<VolumetricFogSettings>(entity) {
                    s.ambient_color = new_color;
                }
            });
        }
    });
    row += 1;

    let mut intensity = settings.ambient_intensity;
    inline_property(ui, row, "Ambient Intensity", theme, |ui| {
        let orig = intensity;
        ui.add(
            egui::DragValue::new(&mut intensity)
                .speed(0.01)
                .range(0.0..=4.0),
        );
        if intensity != orig {
            cmds.push(move |world: &mut World| {
                if let Some(mut s) = world.get_mut::<VolumetricFogSettings>(entity) {
                    s.ambient_intensity = intensity;
                }
            });
        }
    });
    row += 1;

    let mut steps = settings.step_count as f32;
    inline_property(ui, row, "Step Count", theme, |ui| {
        let orig = steps;
        ui.add(egui::DragValue::new(&mut steps).speed(1.0).range(8.0..=256.0));
        if steps != orig {
            let v = steps as u32;
            cmds.push(move |world: &mut World| {
                if let Some(mut s) = world.get_mut::<VolumetricFogSettings>(entity) {
                    s.step_count = v;
                }
            });
        }
    });
    row += 1;

    let mut jitter = settings.jitter;
    inline_property(ui, row, "Jitter", theme, |ui| {
        let orig = jitter;
        ui.add(
            egui::DragValue::new(&mut jitter)
                .speed(0.01)
                .range(0.0..=1.0),
        );
        if jitter != orig {
            cmds.push(move |world: &mut World| {
                if let Some(mut s) = world.get_mut::<VolumetricFogSettings>(entity) {
                    s.jitter = jitter;
                }
            });
        }
    });
}

#[derive(Default)]
pub struct VolumetricFogPlugin;

impl Plugin for VolumetricFogPlugin {
    fn build(&self, app: &mut App) {
        info!("[runtime] VolumetricFogPlugin");
        app.register_type::<VolumetricFogSettings>();
        app.add_systems(Update, (sync_volumetric_fog, cleanup_volumetric_fog));
        #[cfg(feature = "editor")]
        app.register_inspector(inspector_entry());
    }
}

renzora::add!(VolumetricFogPlugin);
