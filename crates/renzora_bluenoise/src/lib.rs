use bevy::prelude::*;
use bevy_light::ShadowFilteringMethod;
use serde::{Deserialize, Serialize};

#[cfg(feature = "editor")]
use {
    egui_phosphor::regular,
    renzora_editor::{AppEditorExt, InspectorEntry},
};

/// Blue Noise Temporal Jittered shadow filtering.
/// Uses blue noise textures (requires `bluenoise_texture` Bevy feature) for
/// temporally-stable dithered shadow sampling, reducing banding artifacts.
#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component, Serialize, Deserialize)]
pub struct BlueNoiseSettings {
    pub enabled: bool,
}

impl Default for BlueNoiseSettings {
    fn default() -> Self {
        Self { enabled: true }
    }
}

fn sync_bluenoise(
    mut commands: Commands,
    sources: Query<(Entity, Ref<BlueNoiseSettings>)>,
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
                    commands.entity(*target).insert(
                        ShadowFilteringMethod::Temporal,
                    );
                } else {
                    commands.entity(*target).remove::<ShadowFilteringMethod>();
                }
                found = true;
                break;
            }
        }
        if !found && routing_changed {
            if let Ok(mut ec) = commands.get_entity(*target) {
                ec.remove::<ShadowFilteringMethod>();
            }
        }
    }
}

fn cleanup_bluenoise(
    mut commands: Commands,
    mut removed: RemovedComponents<BlueNoiseSettings>,
    routing: Res<renzora::EffectRouting>,
) {
    if removed.read().next().is_some() {
        for (target, _) in routing.iter() {
            if let Ok(mut ec) = commands.get_entity(*target) {
                ec.remove::<ShadowFilteringMethod>();
            }
        }
    }
}

#[cfg(feature = "editor")]
fn bluenoise_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "bluenoise",
        display_name: "Blue Noise Shadows",
        icon: regular::DOTS_SIX,
        category: "rendering",
        has_fn: |world, entity| world.get::<BlueNoiseSettings>(entity).is_some(),
        add_fn: Some(|world, entity| {
            world.entity_mut(entity).insert(BlueNoiseSettings::default());
        }),
        remove_fn: Some(|world, entity| {
            world.entity_mut(entity).remove::<BlueNoiseSettings>();
        }),
        is_enabled_fn: Some(|world, entity| {
            world.get::<BlueNoiseSettings>(entity).map(|s| s.enabled).unwrap_or(false)
        }),
        set_enabled_fn: Some(|world, entity, val| {
            if let Some(mut s) = world.get_mut::<BlueNoiseSettings>(entity) { s.enabled = val; }
        }),
        fields: vec![],
        custom_ui_fn: None,
    }
}

pub struct BlueNoisePlugin;

impl Plugin for BlueNoisePlugin {
    fn build(&self, app: &mut App) {
        info!("[runtime] BlueNoisePlugin");
        app.register_type::<BlueNoiseSettings>();
        app.add_systems(Update, (sync_bluenoise, cleanup_bluenoise));
        #[cfg(feature = "editor")]
        app.register_inspector(bluenoise_entry());
    }
}
