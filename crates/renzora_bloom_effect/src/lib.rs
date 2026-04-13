use bevy::prelude::*;
use bevy::post_process::bloom::{Bloom, BloomPrefilter};
use serde::{Deserialize, Serialize};

#[cfg(feature = "editor")]
use {
    egui_phosphor::regular,
    renzora_editor_framework::{AppEditorExt, FieldDef, FieldType, FieldValue, InspectorEntry},
};

#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component, Serialize, Deserialize)]
pub struct BloomSettings {
    pub intensity: f32,
    pub low_frequency_boost: f32,
    pub high_pass_frequency: f32,
    pub threshold: f32,
    pub threshold_softness: f32,
    pub enabled: bool,
}

impl Default for BloomSettings {
    fn default() -> Self {
        Self {
            intensity: 0.08,
            low_frequency_boost: 0.5,
            high_pass_frequency: 0.8,
            threshold: 0.8,
            threshold_softness: 0.3,
            enabled: true,
        }
    }
}

fn sync_bloom(
    mut commands: Commands,
    sources: Query<(Entity, Ref<BloomSettings>)>,
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
                    commands.entity(*target).insert(Bloom {
                        intensity: settings.intensity,
                        low_frequency_boost: settings.low_frequency_boost,
                        low_frequency_boost_curvature: 0.95,
                        high_pass_frequency: settings.high_pass_frequency,
                        prefilter: BloomPrefilter {
                            threshold: settings.threshold,
                            threshold_softness: settings.threshold_softness,
                        },
                        ..Bloom::NATURAL
                    });
                } else {
                    commands.entity(*target).remove::<Bloom>();
                }
                found = true;
                break;
            }
        }
        if !found && routing_changed {
            if let Ok(mut ec) = commands.get_entity(*target) {
                ec.remove::<Bloom>();
            }
        }
    }
}

#[cfg(feature = "editor")]
fn inspector_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "bloom",
        display_name: "Bloom",
        icon: regular::SPARKLE,
        category: "rendering",
        has_fn: |world, entity| world.get::<BloomSettings>(entity).is_some(),
        add_fn: Some(|world, entity| {
            world.entity_mut(entity).insert(BloomSettings::default());
        }),
        remove_fn: Some(|world, entity| {
            world.entity_mut(entity).remove::<(BloomSettings, Bloom)>();
        }),
        is_enabled_fn: Some(|world, entity| {
            world.get::<BloomSettings>(entity).map(|s| s.enabled).unwrap_or(false)
        }),
        set_enabled_fn: Some(|world, entity, val| {
            if let Some(mut s) = world.get_mut::<BloomSettings>(entity) { s.enabled = val; }
        }),
        fields: vec![
            FieldDef {
                name: "Intensity",
                field_type: FieldType::Float { speed: 0.01, min: 0.0, max: 1.0 },
                get_fn: |world, entity| world.get::<BloomSettings>(entity).map(|s| FieldValue::Float(s.intensity)),
                set_fn: |world, entity, val| { if let FieldValue::Float(v) = val { if let Some(mut s) = world.get_mut::<BloomSettings>(entity) { s.intensity = v; } } },
            },
            FieldDef {
                name: "Low Freq Boost",
                field_type: FieldType::Float { speed: 0.01, min: 0.0, max: 1.0 },
                get_fn: |world, entity| world.get::<BloomSettings>(entity).map(|s| FieldValue::Float(s.low_frequency_boost)),
                set_fn: |world, entity, val| { if let FieldValue::Float(v) = val { if let Some(mut s) = world.get_mut::<BloomSettings>(entity) { s.low_frequency_boost = v; } } },
            },
            FieldDef {
                name: "High Pass Freq",
                field_type: FieldType::Float { speed: 0.01, min: 0.0, max: 1.0 },
                get_fn: |world, entity| world.get::<BloomSettings>(entity).map(|s| FieldValue::Float(s.high_pass_frequency)),
                set_fn: |world, entity, val| { if let FieldValue::Float(v) = val { if let Some(mut s) = world.get_mut::<BloomSettings>(entity) { s.high_pass_frequency = v; } } },
            },
            FieldDef {
                name: "Threshold",
                field_type: FieldType::Float { speed: 0.01, min: 0.0, max: 5.0 },
                get_fn: |world, entity| world.get::<BloomSettings>(entity).map(|s| FieldValue::Float(s.threshold)),
                set_fn: |world, entity, val| { if let FieldValue::Float(v) = val { if let Some(mut s) = world.get_mut::<BloomSettings>(entity) { s.threshold = v; } } },
            },
            FieldDef {
                name: "Threshold Softness",
                field_type: FieldType::Float { speed: 0.01, min: 0.0, max: 1.0 },
                get_fn: |world, entity| world.get::<BloomSettings>(entity).map(|s| FieldValue::Float(s.threshold_softness)),
                set_fn: |world, entity, val| { if let FieldValue::Float(v) = val { if let Some(mut s) = world.get_mut::<BloomSettings>(entity) { s.threshold_softness = v; } } },
            },
        ],
        custom_ui_fn: None,
    }
}

fn cleanup_bloom(
    mut commands: Commands,
    mut removed: RemovedComponents<BloomSettings>,
    routing: Res<renzora_core::EffectRouting>,
) {
    if removed.read().next().is_some() {
        for (target, _) in routing.iter() {
            if let Ok(mut ec) = commands.get_entity(*target) {
                ec.remove::<Bloom>();
            }
        }
    }
}

pub struct BloomEffectPlugin;

impl Plugin for BloomEffectPlugin {
    fn build(&self, app: &mut App) {
        info!("[runtime] BloomEffectPlugin");
        app.register_type::<BloomSettings>();
        app.add_systems(Update, (sync_bloom, cleanup_bloom));
        #[cfg(feature = "editor")]
        app.register_inspector(inspector_entry());
    }
}
