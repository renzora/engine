//! Renzora Lumen — Phase 1 scaffold.
//!
//! Authors a `LumenLighting` component on a non-camera entity (typically
//! "World Environment"). The sync system routes the chosen quality tier
//! onto the active cameras. Mutually exclusive with a hand-attached
//! `RtLighting` — Lumen *owns* the screen-space tier when present.
//!
//! Phase 1 implements only `Off` and `ScreenSpace`. Higher tiers
//! (`SdfLow`/`SdfHigh`/`Hwrt`) parse but currently render the same as
//! `Off`; Phases 2-6 of `renzora_lumen_plan.md` fill them in.

use bevy::prelude::*;
use renzora_rt::{RtDebugMode, RtLighting, RtLightingExternallyManaged};
use serde::{Deserialize, Serialize};

#[cfg(feature = "editor")]
use {
    bevy_egui::egui,
    egui_phosphor::regular::LIGHTNING,
    renzora_editor::{inline_property, AppEditorExt, EditorCommands, InspectorEntry},
    renzora_theme::Theme,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Reflect, Serialize, Deserialize, Default)]
pub enum LumenQuality {
    Off,
    #[default]
    ScreenSpace,
    /// Reserved — Phase 5+: SDF tracing, low-res voxel cache.
    SdfLow,
    /// Reserved — Phase 5+: SDF tracing, full-res voxel cache.
    SdfHigh,
    /// Reserved — Phase 10: hardware ray tracing backend.
    Hwrt,
}

/// Debug visualization mode. Currently routes to `RtLighting.debug` when
/// the active quality tier is `ScreenSpace`. Future Lumen-specific
/// variants (`VoxelCache`, `GlobalSdf`, etc.) will get their own paths
/// in Phases 2-4.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Reflect, Serialize, Deserialize, Default)]
pub enum LumenDebug {
    #[default]
    None,
    /// Show only the indirect-light contribution, without the scene
    /// composite. Available in `ScreenSpace` tier today.
    IndirectOnly,
}

#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component, Serialize, Deserialize)]
pub struct LumenLighting {
    pub quality: LumenQuality,
    pub intensity: f32,
    pub debug: LumenDebug,
}

impl Default for LumenLighting {
    fn default() -> Self {
        Self {
            quality: LumenQuality::ScreenSpace,
            intensity: 1.0,
            debug: LumenDebug::None,
        }
    }
}

#[derive(Default)]
pub struct LumenPlugin;

impl Plugin for LumenPlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<LumenLighting>();
        app.add_systems(Update, (sync_lumen_lighting, cleanup_lumen_lighting));

        #[cfg(feature = "editor")]
        app.register_inspector(inspector_entry());
    }
}

/// Route `LumenLighting` from source entities onto target cameras and
/// translate quality into the matching engine-level component.
fn sync_lumen_lighting(
    mut commands: Commands,
    sources: Query<(Entity, Ref<LumenLighting>)>,
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
                apply_quality(&mut commands, *target, &settings);
                found = true;
                break;
            }
        }
        if !found && routing_changed {
            if let Ok(mut ec) = commands.get_entity(*target) {
                ec.remove::<(LumenLighting, RtLighting, RtLightingExternallyManaged)>();
            }
        }
    }
}

fn apply_quality(commands: &mut Commands, target: Entity, settings: &LumenLighting) {
    // Always mirror the component to the camera so the inspector reflects
    // what's active. The `RtLightingExternallyManaged` marker tells
    // `renzora_rt`'s sync system to leave RtLighting alone — without it,
    // RT would clobber what we set every frame because the authored source
    // entity has `LumenLighting`, not `RtLighting`.
    commands
        .entity(target)
        .insert((settings.clone(), RtLightingExternallyManaged));

    match settings.quality {
        LumenQuality::ScreenSpace => {
            let rt_debug = match settings.debug {
                LumenDebug::None => RtDebugMode::Composite,
                LumenDebug::IndirectOnly => RtDebugMode::IndirectOnly,
            };
            commands.entity(target).insert(RtLighting {
                enabled: true,
                intensity: settings.intensity,
                debug: rt_debug,
            });
        }
        LumenQuality::Off | LumenQuality::SdfLow | LumenQuality::SdfHigh | LumenQuality::Hwrt => {
            // Nothing higher than ScreenSpace is wired yet. Strip RtLighting
            // so the camera renders unaltered.
            commands.entity(target).remove::<RtLighting>();
        }
    }
}

fn cleanup_lumen_lighting(
    mut commands: Commands,
    mut removed: RemovedComponents<LumenLighting>,
    routing: Res<renzora::EffectRouting>,
) {
    if removed.read().next().is_some() {
        for (target, _) in routing.iter() {
            if let Ok(mut ec) = commands.get_entity(*target) {
                ec.remove::<(LumenLighting, RtLighting, RtLightingExternallyManaged)>();
            }
        }
    }
}

#[cfg(feature = "editor")]
fn inspector_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "lumen_lighting",
        display_name: "Lumen Global Illumination",
        icon: LIGHTNING,
        category: "lighting",
        has_fn: |world, entity| world.get::<LumenLighting>(entity).is_some(),
        add_fn: Some(|world, entity| {
            world.entity_mut(entity).insert(LumenLighting::default());
            // Lumen owns the screen-space tier when present — strip any
            // hand-attached `RtLighting` so the two don't double-apply.
            world.entity_mut(entity).remove::<RtLighting>();
        }),
        remove_fn: Some(|world, entity| {
            world.entity_mut(entity).remove::<(LumenLighting, RtLighting)>();
        }),
        is_enabled_fn: Some(|world, entity| {
            world
                .get::<LumenLighting>(entity)
                .map(|s| s.quality != LumenQuality::Off)
                .unwrap_or(false)
        }),
        set_enabled_fn: Some(|world, entity, val| {
            if let Some(mut s) = world.get_mut::<LumenLighting>(entity) {
                s.quality = if val { LumenQuality::ScreenSpace } else { LumenQuality::Off };
            }
        }),
        fields: vec![],
        custom_ui_fn: Some(lumen_custom_ui),
    }
}

#[cfg(feature = "editor")]
fn lumen_custom_ui(
    ui: &mut egui::Ui,
    world: &World,
    entity: Entity,
    cmds: &EditorCommands,
    theme: &Theme,
) {
    let Some(settings) = world.get::<LumenLighting>(entity) else { return; };
    let mut data = settings.clone();
    let mut changed = false;

    inline_property(ui, 0, "Quality", theme, |ui| {
        let orig = data.quality;
        egui::ComboBox::from_id_salt("lumen_quality")
            .selected_text(format!("{:?}", data.quality))
            .show_ui(ui, |ui| {
                for q in [
                    LumenQuality::Off,
                    LumenQuality::ScreenSpace,
                    LumenQuality::SdfLow,
                    LumenQuality::SdfHigh,
                    LumenQuality::Hwrt,
                ] {
                    let label = match q {
                        LumenQuality::Off => "Off",
                        LumenQuality::ScreenSpace => "Screen Space (SSGI)",
                        LumenQuality::SdfLow => "SDF Low (Phase 5)",
                        LumenQuality::SdfHigh => "SDF High (Phase 5)",
                        LumenQuality::Hwrt => "Hardware RT (Phase 10)",
                    };
                    ui.selectable_value(&mut data.quality, q, label);
                }
            });
        if data.quality != orig {
            changed = true;
        }
    });

    inline_property(ui, 1, "Intensity", theme, |ui| {
        let orig = data.intensity;
        ui.add(egui::DragValue::new(&mut data.intensity).speed(0.05).range(0.0..=5.0));
        if data.intensity != orig {
            changed = true;
        }
    });

    inline_property(ui, 2, "Debug", theme, |ui| {
        let orig = data.debug;
        egui::ComboBox::from_id_salt("lumen_debug")
            .selected_text(match data.debug {
                LumenDebug::None => "None",
                LumenDebug::IndirectOnly => "Indirect Only",
            })
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut data.debug, LumenDebug::None, "None");
                ui.selectable_value(
                    &mut data.debug,
                    LumenDebug::IndirectOnly,
                    "Indirect Only",
                );
            });
        if data.debug != orig {
            changed = true;
        }
    });

    if changed {
        cmds.push(move |world: &mut World| {
            if let Some(mut s) = world.get_mut::<LumenLighting>(entity) {
                *s = data;
            }
        });
    }
}

renzora::add!(LumenPlugin);
