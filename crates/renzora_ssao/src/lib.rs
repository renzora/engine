use bevy::prelude::*;
use bevy::pbr::ScreenSpaceAmbientOcclusion;
use serde::{Deserialize, Serialize};

#[cfg(feature = "editor")]
use {
    egui_phosphor::regular,
    renzora_editor_framework::{AppEditorExt, InspectorEntry},
};

#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component, Serialize, Deserialize)]
pub struct SsaoSettings {
    pub enabled: bool,
}

impl Default for SsaoSettings {
    fn default() -> Self {
        Self { enabled: true }
    }
}

fn sync_ssao(
    mut commands: Commands,
    sources: Query<(Entity, Ref<SsaoSettings>)>,
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
                    commands.entity(*target).insert(ScreenSpaceAmbientOcclusion::default());
                } else {
                    commands.entity(*target).remove::<ScreenSpaceAmbientOcclusion>();
                }
                found = true;
                break;
            }
        }
        if !found && routing_changed {
            if let Ok(mut ec) = commands.get_entity(*target) {
                ec.remove::<ScreenSpaceAmbientOcclusion>();
            }
        }
    }
}

fn cleanup_ssao(
    mut commands: Commands,
    mut removed: RemovedComponents<SsaoSettings>,
    routing: Res<renzora::EffectRouting>,
) {
    if removed.read().next().is_some() {
        for (target, _) in routing.iter() {
            if let Ok(mut ec) = commands.get_entity(*target) {
                ec.remove::<ScreenSpaceAmbientOcclusion>();
            }
        }
    }
}

#[cfg(feature = "editor")]
fn ssao_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "ssao",
        display_name: "SSAO",
        icon: regular::CIRCLE_HALF,
        category: "rendering",
        has_fn: |world, entity| world.get::<SsaoSettings>(entity).is_some(),
        add_fn: Some(|world, entity| {
            world.entity_mut(entity).insert(SsaoSettings::default());
        }),
        remove_fn: Some(|world, entity| {
            world.entity_mut(entity).remove::<(SsaoSettings, ScreenSpaceAmbientOcclusion)>();
        }),
        is_enabled_fn: Some(|world, entity| {
            world.get::<SsaoSettings>(entity).map(|s| s.enabled).unwrap_or(false)
        }),
        set_enabled_fn: Some(|world, entity, val| {
            if let Some(mut s) = world.get_mut::<SsaoSettings>(entity) { s.enabled = val; }
        }),
        fields: vec![],
        custom_ui_fn: None,
    }
}

pub struct SsaoPlugin;

impl Plugin for SsaoPlugin {
    fn build(&self, app: &mut App) {
        info!("[runtime] SsaoPlugin");
        app.register_type::<SsaoSettings>();
        app.add_systems(Update, (sync_ssao, cleanup_ssao));
        #[cfg(feature = "editor")]
        app.register_inspector(ssao_entry());
    }
}
