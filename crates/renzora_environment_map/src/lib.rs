//! Environment map (IBL) component.
//!
//! Image-based lighting drives PBR reflections + ambient diffuse from a
//! cubemap. Right now we use Bevy's atmosphere-derived cubemap
//! (`AtmosphereEnvironmentMapLight`) — the procedural sky gets baked into
//! a cubemap each frame and fed back into the PBR pipeline.
//!
//! Architecturally separate from the atmosphere component because the
//! choice of "should reflections happen" is independent of "should the
//! sky render with scattering." A future HDR-cubemap variant can extend
//! the same component (see `EnvironmentMapKind` placeholder for where
//! that would live).
//!
//! ## Bevy 0.18 caveat
//!
//! Bevy locks the camera's bind group layout the first frame it renders,
//! with IBL slots present iff `AtmosphereEnvironmentMapLight` existed at
//! that moment. Adding/removing it later crashes wgpu. The camera spawn
//! site (in `renzora_engine`) attaches the component at low intensity so
//! the layout is stable; this plugin updates `intensity` in-place via
//! `EffectRouting`. `enabled = false` collapses intensity to 0 — visually
//! "off" without touching the bindings.

use bevy::light::AtmosphereEnvironmentMapLight;
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

#[cfg(feature = "editor")]
use {
    bevy_egui::egui,
    egui_phosphor::regular,
    renzora_editor::{inline_property, AppEditorExt, EditorCommands, InspectorEntry},
    renzora_theme::Theme,
};

/// User-authored settings for sky-driven image-based lighting. Attach to
/// any non-camera entity (typically a "World Environment") and the plugin
/// routes its values onto every active camera via `EffectRouting`.
#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component, Serialize, Deserialize)]
pub struct EnvironmentMapComponentSettings {
    /// IBL contribution strength. 1.0 = sky-bright reflections + ambient
    /// (often too strong, washes out direct sun shadows). 0.3 is a good
    /// "modern engine default" — visible reflections, contrast preserved.
    pub intensity: f32,
    pub enabled: bool,
}

impl Default for EnvironmentMapComponentSettings {
    fn default() -> Self {
        Self {
            intensity: 0.3,
            enabled: true,
        }
    }
}

fn sync_environment_map(
    mut commands: Commands,
    sources: Query<Ref<EnvironmentMapComponentSettings>>,
    routing: Res<renzora::EffectRouting>,
) {
    let routing_changed = routing.is_changed();
    for (target, source_list) in routing.iter() {
        // Find a source on the routing list that has the settings.
        let source_settings = source_list
            .iter()
            .find_map(|&src| sources.get(src).ok());

        match source_settings {
            Some(settings) => {
                if !routing_changed && !settings.is_changed() {
                    continue;
                }
                let intensity = if settings.enabled { settings.intensity } else { 0.0 };
                // Replace the existing component in place — the camera
                // spawn site attached it up front so the bind group
                // layout stays stable across enables/disables.
                commands.entity(*target).insert(AtmosphereEnvironmentMapLight {
                    intensity,
                    ..default()
                });
            }
            None => {
                // No source for this target — only push the "off" value
                // when the routing actually changed (e.g. the
                // WorldEnvironment was just removed from the source list).
                // Otherwise we'd thrash the camera every frame.
                if routing_changed {
                    commands.entity(*target).insert(AtmosphereEnvironmentMapLight {
                        intensity: 0.0,
                        ..default()
                    });
                }
            }
        }
    }
}

/// When the source `EnvironmentMapComponentSettings` is removed (entity
/// despawn or component removed via inspector), zero IBL intensity on
/// every camera the routing currently targets. Without this the camera
/// would keep its last-applied intensity until something else updated it.
fn cleanup_environment_map(
    mut commands: Commands,
    mut removed: RemovedComponents<EnvironmentMapComponentSettings>,
    routing: Res<renzora::EffectRouting>,
) {
    if removed.read().next().is_some() {
        for (target, _) in routing.iter() {
            commands.entity(*target).insert(AtmosphereEnvironmentMapLight {
                intensity: 0.0,
                ..default()
            });
        }
    }
}

pub struct EnvironmentMapPlugin;

impl Plugin for EnvironmentMapPlugin {
    fn build(&self, app: &mut App) {
        info!("[runtime] EnvironmentMapPlugin");
        app.register_type::<EnvironmentMapComponentSettings>();
        app.add_systems(Update, (sync_environment_map, cleanup_environment_map));
        #[cfg(feature = "editor")]
        app.register_inspector(inspector_entry());
    }
}

#[cfg(feature = "editor")]
fn inspector_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "environment_map",
        display_name: "Environment Map",
        icon: regular::SUN_HORIZON,
        category: "rendering",
        has_fn: |world, entity| world.get::<EnvironmentMapComponentSettings>(entity).is_some(),
        add_fn: Some(|world, entity| {
            world
                .entity_mut(entity)
                .insert(EnvironmentMapComponentSettings::default());
        }),
        remove_fn: Some(|world, entity| {
            world
                .entity_mut(entity)
                .remove::<EnvironmentMapComponentSettings>();
        }),
        is_enabled_fn: Some(|world, entity| {
            world
                .get::<EnvironmentMapComponentSettings>(entity)
                .map(|s| s.enabled)
                .unwrap_or(false)
        }),
        set_enabled_fn: Some(|world, entity, val| {
            if let Some(mut s) =
                world.get_mut::<EnvironmentMapComponentSettings>(entity)
            {
                s.enabled = val;
            }
        }),
        fields: vec![],
        custom_ui_fn: Some(environment_map_custom_ui),
    }
}

#[cfg(feature = "editor")]
fn environment_map_custom_ui(
    ui: &mut egui::Ui,
    world: &World,
    entity: Entity,
    cmds: &EditorCommands,
    theme: &Theme,
) {
    let Some(settings) = world.get::<EnvironmentMapComponentSettings>(entity) else {
        return;
    };

    let mut intensity = settings.intensity;
    inline_property(ui, 0, "Intensity", theme, |ui| {
        let orig = intensity;
        ui.add(
            egui::DragValue::new(&mut intensity)
                .speed(0.01)
                .range(0.0..=10.0),
        );
        if intensity != orig {
            cmds.push(move |world: &mut World| {
                if let Some(mut s) =
                    world.get_mut::<EnvironmentMapComponentSettings>(entity)
                {
                    s.intensity = intensity;
                }
            });
        }
    });
}
