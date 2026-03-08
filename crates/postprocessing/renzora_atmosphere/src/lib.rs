use bevy::prelude::*;
use bevy::pbr::{Atmosphere, AtmosphereMode, AtmosphereSettings, ScatteringMedium};
use serde::{Deserialize, Serialize};

#[cfg(feature = "editor")]
use {
    bevy_egui::egui,
    egui_phosphor::regular,
    renzora_editor::{AppEditorExt, EditorCommands, FieldDef, FieldType, FieldValue, InspectorEntry},
    renzora_theme::Theme,
};

#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component, Serialize, Deserialize)]
pub struct AtmosphereComponentSettings {
    pub bottom_radius: f32,
    pub top_radius: f32,
    pub ground_albedo: f32,
    pub scene_units_to_m: f32,
    /// 0 = LookupTexture, 1 = Raymarched
    pub mode: u32,
    pub enabled: bool,
}

impl Default for AtmosphereComponentSettings {
    fn default() -> Self {
        Self {
            bottom_radius: 6_360_000.0,
            top_radius: 6_460_000.0,
            ground_albedo: 0.3,
            scene_units_to_m: 1.0,
            mode: 0,
            enabled: true,
        }
    }
}

/// Stores the ScatteringMedium handle so we don't recreate it every frame.
#[derive(Component)]
struct AtmosphereMediumHandle(Handle<ScatteringMedium>);

fn sync_atmosphere(
    mut commands: Commands,
    mut mediums: ResMut<Assets<ScatteringMedium>>,
    query: Query<(Entity, &AtmosphereComponentSettings, Option<&AtmosphereMediumHandle>), Changed<AtmosphereComponentSettings>>,
) {
    for (entity, settings, existing_handle) in &query {
        if !settings.enabled {
            commands.entity(entity).remove::<(Atmosphere, AtmosphereSettings)>();
            continue;
        }

        let handle = if let Some(h) = existing_handle {
            h.0.clone()
        } else {
            let h = mediums.add(ScatteringMedium::default());
            commands.entity(entity).insert(AtmosphereMediumHandle(h.clone()));
            h
        };

        let rendering_method = match settings.mode {
            1 => AtmosphereMode::Raymarched,
            _ => AtmosphereMode::LookupTexture,
        };

        commands.entity(entity).insert((
            Atmosphere {
                bottom_radius: settings.bottom_radius,
                top_radius: settings.top_radius,
                ground_albedo: Vec3::splat(settings.ground_albedo),
                medium: handle,
            },
            AtmosphereSettings {
                scene_units_to_m: settings.scene_units_to_m,
                rendering_method,
                ..default()
            },
        ));
    }
}

fn cleanup_atmosphere(mut commands: Commands, mut removed: RemovedComponents<AtmosphereComponentSettings>) {
    for entity in removed.read() {
        if let Ok(mut ec) = commands.get_entity(entity) {
            ec.remove::<(Atmosphere, AtmosphereSettings, AtmosphereMediumHandle)>();
        }
    }
}

#[cfg(feature = "editor")]
const ATMO_MODE_LABELS: [&str; 2] = ["Lookup Texture", "Raymarched"];

#[cfg(feature = "editor")]
fn inspector_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "atmosphere",
        display_name: "Atmosphere",
        icon: regular::CLOUD_SUN,
        category: "rendering",
        has_fn: |world, entity| world.get::<AtmosphereComponentSettings>(entity).is_some(),
        add_fn: Some(|world, entity| {
            world.entity_mut(entity).insert(AtmosphereComponentSettings::default());
        }),
        remove_fn: Some(|world, entity| {
            world.entity_mut(entity).remove::<(AtmosphereComponentSettings, Atmosphere, AtmosphereSettings, AtmosphereMediumHandle)>();
        }),
        is_enabled_fn: Some(|world, entity| {
            world.get::<AtmosphereComponentSettings>(entity).map(|s| s.enabled).unwrap_or(false)
        }),
        set_enabled_fn: Some(|world, entity, val| {
            if let Some(mut s) = world.get_mut::<AtmosphereComponentSettings>(entity) { s.enabled = val; }
        }),
        fields: vec![
            FieldDef {
                name: "Bottom Radius",
                field_type: FieldType::Float { speed: 1000.0, min: 0.0, max: 100_000_000.0 },
                get_fn: |world, entity| world.get::<AtmosphereComponentSettings>(entity).map(|s| FieldValue::Float(s.bottom_radius)),
                set_fn: |world, entity, val| { if let FieldValue::Float(v) = val { if let Some(mut s) = world.get_mut::<AtmosphereComponentSettings>(entity) { s.bottom_radius = v; } } },
            },
            FieldDef {
                name: "Top Radius",
                field_type: FieldType::Float { speed: 1000.0, min: 0.0, max: 100_000_000.0 },
                get_fn: |world, entity| world.get::<AtmosphereComponentSettings>(entity).map(|s| FieldValue::Float(s.top_radius)),
                set_fn: |world, entity, val| { if let FieldValue::Float(v) = val { if let Some(mut s) = world.get_mut::<AtmosphereComponentSettings>(entity) { s.top_radius = v; } } },
            },
            FieldDef {
                name: "Ground Albedo",
                field_type: FieldType::Float { speed: 0.01, min: 0.0, max: 1.0 },
                get_fn: |world, entity| world.get::<AtmosphereComponentSettings>(entity).map(|s| FieldValue::Float(s.ground_albedo)),
                set_fn: |world, entity, val| { if let FieldValue::Float(v) = val { if let Some(mut s) = world.get_mut::<AtmosphereComponentSettings>(entity) { s.ground_albedo = v; } } },
            },
            FieldDef {
                name: "Scene Units → m",
                field_type: FieldType::Float { speed: 0.1, min: 0.001, max: 10000.0 },
                get_fn: |world, entity| world.get::<AtmosphereComponentSettings>(entity).map(|s| FieldValue::Float(s.scene_units_to_m)),
                set_fn: |world, entity, val| { if let FieldValue::Float(v) = val { if let Some(mut s) = world.get_mut::<AtmosphereComponentSettings>(entity) { s.scene_units_to_m = v; } } },
            },
        ],
        custom_ui_fn: Some(atmosphere_custom_ui),
    }
}

#[cfg(feature = "editor")]
fn atmosphere_custom_ui(
    ui: &mut egui::Ui,
    world: &World,
    entity: Entity,
    cmds: &EditorCommands,
    _theme: &Theme,
) {
    let Some(settings) = world.get::<AtmosphereComponentSettings>(entity) else { return };
    let current = settings.mode as usize;
    let mut new_idx = current;

    ui.horizontal(|ui| {
        ui.label("Rendering");
        egui::ComboBox::from_id_salt("atmo_mode")
            .selected_text(*ATMO_MODE_LABELS.get(current).unwrap_or(&"Unknown"))
            .show_ui(ui, |ui| {
                for (i, label) in ATMO_MODE_LABELS.iter().enumerate() {
                    if ui.selectable_value(&mut new_idx, i, *label).changed() {
                        let mode = new_idx as u32;
                        cmds.push(move |world: &mut World| {
                            if let Some(mut s) = world.get_mut::<AtmosphereComponentSettings>(entity) {
                                s.mode = mode;
                            }
                        });
                    }
                }
            });
    });
}

pub struct AtmospherePlugin;

impl Plugin for AtmospherePlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<AtmosphereComponentSettings>();
        app.add_systems(Update, (sync_atmosphere, cleanup_atmosphere));
        #[cfg(feature = "editor")]
        app.register_inspector(inspector_entry());
    }
}
