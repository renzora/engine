use bevy::core_pipeline::prepass::DeferredPrepass;
use bevy::pbr::ScreenSpaceReflections;
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

#[cfg(feature = "editor")]
use {
    egui_phosphor::regular,
    renzora_editor::{AppEditorExt, InspectorEntry},
};

#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component, Serialize, Deserialize)]
pub struct SsrSettings {
    pub enabled: bool,
}

impl Default for SsrSettings {
    fn default() -> Self {
        Self { enabled: true }
    }
}

fn sync_ssr(
    mut commands: Commands,
    sources: Query<(Entity, Ref<SsrSettings>)>,
    deferred_cameras: Query<(), With<DeferredPrepass>>,
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
                    // Bevy 0.18's SSR is part of the deferred lighting
                    // path: inserting `ScreenSpaceReflections` switches
                    // the camera into deferred shading, which needs
                    // `DeferredPrepass` to supply the G-buffer
                    // (base color, normals, MR maps). On a forward-only
                    // camera the prepass isn't there, so shading
                    // collapses (shadows disappear, GI compositing
                    // breaks). Refuse to insert in that case.
                    if !deferred_cameras.contains(*target) {
                        warn!(
                            "SSR enabled but camera lacks DeferredPrepass; \
                             skipping (forward rendering path doesn't support SSR). \
                             Disable SSR in the inspector or wire up the deferred \
                             renderer."
                        );
                        commands.entity(*target).remove::<ScreenSpaceReflections>();
                    } else {
                        commands
                            .entity(*target)
                            .insert(ScreenSpaceReflections::default());
                    }
                } else {
                    commands.entity(*target).remove::<ScreenSpaceReflections>();
                }
                found = true;
                break;
            }
        }
        if !found && routing_changed {
            if let Ok(mut ec) = commands.get_entity(*target) {
                ec.remove::<ScreenSpaceReflections>();
            }
        }
    }
}

fn cleanup_ssr(
    mut commands: Commands,
    mut removed: RemovedComponents<SsrSettings>,
    routing: Res<renzora::EffectRouting>,
) {
    if removed.read().next().is_some() {
        for (target, _) in routing.iter() {
            if let Ok(mut ec) = commands.get_entity(*target) {
                ec.remove::<ScreenSpaceReflections>();
            }
        }
    }
}

#[cfg(feature = "editor")]
fn ssr_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "ssr",
        display_name: "SSR",
        icon: regular::SWAP,
        category: "rendering",
        has_fn: |world, entity| world.get::<SsrSettings>(entity).is_some(),
        add_fn: Some(|world, entity| {
            world.entity_mut(entity).insert(SsrSettings::default());
        }),
        remove_fn: Some(|world, entity| {
            world
                .entity_mut(entity)
                .remove::<(SsrSettings, ScreenSpaceReflections)>();
        }),
        is_enabled_fn: Some(|world, entity| {
            world
                .get::<SsrSettings>(entity)
                .map(|s| s.enabled)
                .unwrap_or(false)
        }),
        set_enabled_fn: Some(|world, entity, val| {
            if let Some(mut s) = world.get_mut::<SsrSettings>(entity) {
                s.enabled = val;
            }
        }),
        fields: vec![],
        custom_ui_fn: None,
    }
}

#[derive(Default)]
pub struct SsrPlugin;

impl Plugin for SsrPlugin {
    fn build(&self, app: &mut App) {
        info!("[runtime] SsrPlugin");
        app.register_type::<SsrSettings>();
        app.add_systems(Update, (sync_ssr, cleanup_ssr));
        #[cfg(feature = "editor")]
        app.register_inspector(ssr_entry());
    }
}

renzora::add!(SsrPlugin);
