use bevy::prelude::*;
use bevy::pbr::{DistanceFog, FogFalloff};
use serde::{Deserialize, Serialize};

#[cfg(feature = "editor")]
use {
    egui_phosphor::regular,
    renzora_editor::{AppEditorExt, FieldDef, FieldType, FieldValue, InspectorEntry},
};

#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component, Serialize, Deserialize)]
pub struct DistanceFogSettings {
    pub color_r: f32,
    pub color_g: f32,
    pub color_b: f32,
    pub start: f32,
    pub end: f32,
    pub directional_light_exponent: f32,
    pub enabled: bool,
}

impl Default for DistanceFogSettings {
    fn default() -> Self {
        Self {
            color_r: 0.5,
            color_g: 0.5,
            color_b: 0.5,
            start: 10.0,
            end: 100.0,
            directional_light_exponent: 8.0,
            enabled: true,
        }
    }
}

fn sync_distance_fog(
    mut commands: Commands,
    query: Query<(Entity, &DistanceFogSettings), Changed<DistanceFogSettings>>,
) {
    for (entity, settings) in &query {
        if !settings.enabled {
            commands.entity(entity).remove::<DistanceFog>();
            continue;
        }
        commands.entity(entity).insert(DistanceFog {
            color: Color::srgb(settings.color_r, settings.color_g, settings.color_b),
            directional_light_color: Color::NONE,
            directional_light_exponent: settings.directional_light_exponent,
            falloff: FogFalloff::Linear {
                start: settings.start,
                end: settings.end,
            },
        });
    }
}

#[cfg(feature = "editor")]
fn inspector_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "distance_fog",
        display_name: "Distance Fog",
        icon: regular::CLOUD_FOG,
        category: "rendering",
        has_fn: |world, entity| world.get::<DistanceFogSettings>(entity).is_some(),
        add_fn: Some(|world, entity| {
            world.entity_mut(entity).insert(DistanceFogSettings::default());
        }),
        remove_fn: Some(|world, entity| {
            world.entity_mut(entity).remove::<(DistanceFogSettings, DistanceFog)>();
        }),
        is_enabled_fn: Some(|world, entity| {
            world.get::<DistanceFogSettings>(entity).map(|s| s.enabled).unwrap_or(false)
        }),
        set_enabled_fn: Some(|world, entity, val| {
            if let Some(mut s) = world.get_mut::<DistanceFogSettings>(entity) { s.enabled = val; }
        }),
        fields: vec![
            FieldDef {
                name: "Color",
                field_type: FieldType::Color,
                get_fn: |world, entity| {
                    world.get::<DistanceFogSettings>(entity).map(|s| {
                        FieldValue::Color([s.color_r, s.color_g, s.color_b])
                    })
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Color([r, g, b]) = val {
                        if let Some(mut s) = world.get_mut::<DistanceFogSettings>(entity) {
                            s.color_r = r;
                            s.color_g = g;
                            s.color_b = b;
                        }
                    }
                },
            },
            FieldDef {
                name: "Start",
                field_type: FieldType::Float { speed: 0.5, min: 0.0, max: 10000.0 },
                get_fn: |world, entity| world.get::<DistanceFogSettings>(entity).map(|s| FieldValue::Float(s.start)),
                set_fn: |world, entity, val| { if let FieldValue::Float(v) = val { if let Some(mut s) = world.get_mut::<DistanceFogSettings>(entity) { s.start = v; } } },
            },
            FieldDef {
                name: "End",
                field_type: FieldType::Float { speed: 0.5, min: 0.0, max: 10000.0 },
                get_fn: |world, entity| world.get::<DistanceFogSettings>(entity).map(|s| FieldValue::Float(s.end)),
                set_fn: |world, entity, val| { if let FieldValue::Float(v) = val { if let Some(mut s) = world.get_mut::<DistanceFogSettings>(entity) { s.end = v; } } },
            },
            FieldDef {
                name: "Light Exponent",
                field_type: FieldType::Float { speed: 0.1, min: 1.0, max: 64.0 },
                get_fn: |world, entity| world.get::<DistanceFogSettings>(entity).map(|s| FieldValue::Float(s.directional_light_exponent)),
                set_fn: |world, entity, val| { if let FieldValue::Float(v) = val { if let Some(mut s) = world.get_mut::<DistanceFogSettings>(entity) { s.directional_light_exponent = v; } } },
            },
        ],
        custom_ui_fn: None,
    }
}

fn cleanup_distance_fog(mut commands: Commands, mut removed: RemovedComponents<DistanceFogSettings>) {
    for entity in removed.read() {
        if let Ok(mut ec) = commands.get_entity(entity) {
            ec.remove::<DistanceFog>();
        }
    }
}

pub struct DistanceFogPlugin;

impl Plugin for DistanceFogPlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<DistanceFogSettings>();
        app.add_systems(Update, (sync_distance_fog, cleanup_distance_fog));
        #[cfg(feature = "editor")]
        app.register_inspector(inspector_entry());
    }
}
