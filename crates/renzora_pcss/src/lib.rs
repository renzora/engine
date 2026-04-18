use bevy::prelude::*;
use bevy_light::ShadowFilteringMethod;
use serde::{Deserialize, Serialize};

#[cfg(feature = "editor")]
use {
    egui_phosphor::regular,
    renzora_editor_framework::{AppEditorExt, FieldDef, FieldType, FieldValue, InspectorEntry},
};

/// Percentage-Closer Soft Shadows settings.
/// Wraps Bevy's `ShadowFilteringMethod::Pcss` with configurable soft shadow size
/// on directional, point, and spot lights.
#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component, Serialize, Deserialize)]
pub struct PcssSettings {
    pub soft_shadow_size: f32,
    pub enabled: bool,
}

impl Default for PcssSettings {
    fn default() -> Self {
        Self {
            soft_shadow_size: 10.0,
            enabled: true,
        }
    }
}

fn sync_pcss_camera(
    mut commands: Commands,
    sources: Query<(Entity, Ref<PcssSettings>)>,
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
                    commands.entity(*target).insert(ShadowFilteringMethod::Temporal);
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

fn sync_pcss_lights(
    sources: Query<&PcssSettings>,
    routing: Res<renzora::EffectRouting>,
    mut dir_lights: Query<&mut DirectionalLight>,
    mut point_lights: Query<&mut PointLight>,
    mut spot_lights: Query<&mut SpotLight>,
) {
    // Find the first PcssSettings from any source entity
    let mut soft_size = 10.0f32;
    let mut pcss_enabled = false;
    for (_, source_list) in routing.iter() {
        for &src in source_list {
            if let Ok(settings) = sources.get(src) {
                soft_size = settings.soft_shadow_size;
                pcss_enabled = settings.enabled;
                break;
            }
        }
        if pcss_enabled { break; }
    }

    if pcss_enabled {
        for mut light in dir_lights.iter_mut() {
            light.soft_shadow_size = Some(soft_size);
        }
        for mut light in point_lights.iter_mut() {
            light.soft_shadows_enabled = true;
        }
        for mut light in spot_lights.iter_mut() {
            light.soft_shadows_enabled = true;
        }
    }
}

fn cleanup_pcss(
    mut commands: Commands,
    mut removed: RemovedComponents<PcssSettings>,
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
fn pcss_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "pcss",
        display_name: "PCSS Soft Shadows",
        icon: regular::SUN_DIM,
        category: "rendering",
        has_fn: |world, entity| world.get::<PcssSettings>(entity).is_some(),
        add_fn: Some(|world, entity| {
            world.entity_mut(entity).insert(PcssSettings::default());
        }),
        remove_fn: Some(|world, entity| {
            world.entity_mut(entity).remove::<PcssSettings>();
        }),
        is_enabled_fn: Some(|world, entity| {
            world.get::<PcssSettings>(entity).map(|s| s.enabled).unwrap_or(false)
        }),
        set_enabled_fn: Some(|world, entity, val| {
            if let Some(mut s) = world.get_mut::<PcssSettings>(entity) { s.enabled = val; }
        }),
        fields: vec![
            FieldDef {
                name: "Soft Shadow Size",
                field_type: FieldType::Float { speed: 0.5, min: 0.1, max: 100.0 },
                get_fn: |world, entity| world.get::<PcssSettings>(entity).map(|s| FieldValue::Float(s.soft_shadow_size)),
                set_fn: |world, entity, val| { if let FieldValue::Float(v) = val { if let Some(mut s) = world.get_mut::<PcssSettings>(entity) { s.soft_shadow_size = v; } } },
            },
        ],
        custom_ui_fn: None,
    }
}

pub struct PcssPlugin;

impl Plugin for PcssPlugin {
    fn build(&self, app: &mut App) {
        info!("[runtime] PcssPlugin");
        app.register_type::<PcssSettings>();
        app.add_systems(Update, (sync_pcss_camera, sync_pcss_lights, cleanup_pcss));
        #[cfg(feature = "editor")]
        app.register_inspector(pcss_entry());
    }
}
