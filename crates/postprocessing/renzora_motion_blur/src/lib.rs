use bevy::prelude::*;
use bevy::post_process::motion_blur::MotionBlur;
use serde::{Deserialize, Serialize};

#[cfg(feature = "editor")]
use {
    egui_phosphor::regular,
    renzora_editor::{AppEditorExt, FieldDef, FieldType, FieldValue, InspectorEntry},
};

#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component, Serialize, Deserialize)]
pub struct MotionBlurSettings {
    pub shutter_angle: f32,
    pub samples: f32,
    pub enabled: bool,
}

impl Default for MotionBlurSettings {
    fn default() -> Self {
        Self {
            shutter_angle: 0.5,
            samples: 2.0,
            enabled: true,
        }
    }
}

fn sync_motion_blur(
    mut commands: Commands,
    sources: Query<(Entity, Ref<MotionBlurSettings>)>,
    routing: Res<renzora_core::EffectRouting>,
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
                    commands.entity(*target).insert(MotionBlur {
                        shutter_angle: settings.shutter_angle,
                        samples: settings.samples as u32,
                    });
                } else {
                    commands.entity(*target).remove::<MotionBlur>();
                }
                found = true;
                break;
            }
        }
        if !found && routing_changed {
            if let Ok(mut ec) = commands.get_entity(*target) {
                ec.remove::<MotionBlur>();
            }
        }
    }
}

#[cfg(feature = "editor")]
fn inspector_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "motion_blur",
        display_name: "Motion Blur",
        icon: regular::WIND,
        category: "rendering",
        has_fn: |world, entity| world.get::<MotionBlurSettings>(entity).is_some(),
        add_fn: Some(|world, entity| {
            world.entity_mut(entity).insert(MotionBlurSettings::default());
        }),
        remove_fn: Some(|world, entity| {
            world.entity_mut(entity).remove::<(MotionBlurSettings, MotionBlur)>();
        }),
        is_enabled_fn: Some(|world, entity| {
            world.get::<MotionBlurSettings>(entity).map(|s| s.enabled).unwrap_or(false)
        }),
        set_enabled_fn: Some(|world, entity, val| {
            if let Some(mut s) = world.get_mut::<MotionBlurSettings>(entity) { s.enabled = val; }
        }),
        fields: vec![
            FieldDef {
                name: "Shutter Angle",
                field_type: FieldType::Float { speed: 0.01, min: 0.0, max: 2.0 },
                get_fn: |world, entity| world.get::<MotionBlurSettings>(entity).map(|s| FieldValue::Float(s.shutter_angle)),
                set_fn: |world, entity, val| { if let FieldValue::Float(v) = val { if let Some(mut s) = world.get_mut::<MotionBlurSettings>(entity) { s.shutter_angle = v; } } },
            },
            FieldDef {
                name: "Samples",
                field_type: FieldType::Float { speed: 1.0, min: 0.0, max: 16.0 },
                get_fn: |world, entity| world.get::<MotionBlurSettings>(entity).map(|s| FieldValue::Float(s.samples)),
                set_fn: |world, entity, val| { if let FieldValue::Float(v) = val { if let Some(mut s) = world.get_mut::<MotionBlurSettings>(entity) { s.samples = v; } } },
            },
        ],
        custom_ui_fn: None,
    }
}

fn cleanup_motion_blur(
    mut commands: Commands,
    mut removed: RemovedComponents<MotionBlurSettings>,
    routing: Res<renzora_core::EffectRouting>,
) {
    if removed.read().next().is_some() {
        for (target, _) in routing.iter() {
            if let Ok(mut ec) = commands.get_entity(*target) {
                ec.remove::<MotionBlur>();
            }
        }
    }
}

pub struct MotionBlurPlugin;

impl Plugin for MotionBlurPlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<MotionBlurSettings>();
        app.add_systems(Update, (sync_motion_blur, cleanup_motion_blur));
        #[cfg(feature = "editor")]
        app.register_inspector(inspector_entry());
    }
}
