//! Sun component definition — directional light positioned by azimuth/elevation

use bevy::prelude::*;
use bevy_egui::egui;

use crate::component_system::{ComponentCategory, ComponentRegistry, PropertyValue, PropertyValueType};
use crate::core::{DisabledComponents, EditorEntity, ViewportCamera};
use crate::register_component;
use crate::shared::SunData;
use crate::ui::property_row;

use egui_phosphor::regular::SUN_HORIZON;

// ============================================================================
// Sun Disc (visual sun in the sky)
// ============================================================================

/// Marker component for the sun disc entity (not serialized)
#[derive(Component)]
pub struct SunDiscMarker;

/// Resource tracking the sun disc entity and its asset handles
#[derive(Resource, Default)]
pub struct SunDiscState {
    pub entity: Option<Entity>,
    pub material_handle: Option<Handle<StandardMaterial>>,
    pub mesh_handle: Option<Handle<Mesh>>,
    /// Cached angular diameter to detect mesh changes
    cached_angular_diameter: f32,
}

/// System that spawns/updates/despawns the visual sun disc billboard.
pub(crate) fn sync_sun_disc(
    mut commands: Commands,
    mut disc_state: ResMut<SunDiscState>,
    sun_query: Query<(&SunData, &EditorEntity, Option<&DisabledComponents>)>,
    camera_query: Query<&Transform, With<ViewportCamera>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // Find the first active, non-disabled sun
    let active_sun = sun_query.iter().find(|(_, editor, dc)| {
        editor.visible && !dc.map_or(false, |d| d.is_disabled("sun"))
    });

    let Some((sun_data, _, _)) = active_sun else {
        // No active sun — despawn disc if it exists
        if let Some(disc_entity) = disc_state.entity.take() {
            commands.entity(disc_entity).despawn();
            disc_state.material_handle = None;
            disc_state.mesh_handle = None;
            disc_state.cached_angular_diameter = 0.0;
        }
        return;
    };

    let Ok(camera_transform) = camera_query.single() else {
        return;
    };

    let camera_pos = camera_transform.translation;
    // direction() returns the direction light travels (away from sun toward scene),
    // so negate it to get the direction FROM camera TOWARD the sun
    let sun_dir = -sun_data.direction();
    let distance = 900.0;
    let disc_position = camera_pos + sun_dir * distance;

    // Compute disc radius from angular diameter
    let half_angle_rad = (sun_data.angular_diameter * 0.5).to_radians();
    let disc_radius = distance * half_angle_rad.tan();

    // Compute emissive color (HDR for bloom)
    let emissive_strength = (sun_data.illuminance / 100_000.0) * 50.0;
    let emissive = LinearRgba::new(
        sun_data.color.x * emissive_strength,
        sun_data.color.y * emissive_strength,
        sun_data.color.z * emissive_strength,
        1.0,
    );

    if let Some(disc_entity) = disc_state.entity {
        // Entity exists — update its transform, material, and mesh if needed
        if commands.get_entity(disc_entity).is_ok() {
            // Update mesh if angular diameter changed
            if (sun_data.angular_diameter - disc_state.cached_angular_diameter).abs() > 0.001 {
                let mesh = meshes.add(Circle::new(disc_radius));
                disc_state.mesh_handle = Some(mesh.clone());
                disc_state.cached_angular_diameter = sun_data.angular_diameter;
                commands.entity(disc_entity).insert(Mesh3d(mesh));
            }

            // Update material
            if let Some(ref mat_handle) = disc_state.material_handle {
                if let Some(mat) = materials.get_mut(mat_handle) {
                    mat.emissive = emissive;
                    mat.base_color = Color::WHITE;
                }
            }

            // Update transform: position at disc_position, facing the camera
            let look_transform = Transform::from_translation(disc_position)
                .looking_at(camera_pos, Vec3::Y);
            commands.entity(disc_entity).insert(look_transform);
        } else {
            // Entity was despawned externally, reset state
            disc_state.entity = None;
            disc_state.material_handle = None;
            disc_state.mesh_handle = None;
            disc_state.cached_angular_diameter = 0.0;
        }
    }

    if disc_state.entity.is_none() {
        // Spawn new disc entity
        let mesh_handle = meshes.add(Circle::new(disc_radius));
        let material_handle = materials.add(StandardMaterial {
            base_color: Color::WHITE,
            emissive,
            unlit: true,
            alpha_mode: AlphaMode::Blend,
            ..default()
        });

        let transform = Transform::from_translation(disc_position)
            .looking_at(camera_pos, Vec3::Y);

        let disc_entity = commands.spawn((
            Mesh3d(mesh_handle.clone()),
            MeshMaterial3d(material_handle.clone()),
            transform,
            SunDiscMarker,
        )).id();

        disc_state.entity = Some(disc_entity);
        disc_state.mesh_handle = Some(mesh_handle);
        disc_state.material_handle = Some(material_handle);
        disc_state.cached_angular_diameter = sun_data.angular_diameter;
    }
}

// ============================================================================
// Custom Add/Remove/Deserialize
// ============================================================================

fn add_sun(
    commands: &mut Commands,
    entity: Entity,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) {
    let data = SunData::default();
    let dir = data.direction();
    commands.entity(entity).insert((
        DirectionalLight {
            color: Color::srgb(data.color.x, data.color.y, data.color.z),
            illuminance: data.illuminance,
            shadows_enabled: data.shadows_enabled,
            ..default()
        },
        Transform::from_rotation(Quat::from_rotation_arc(Vec3::NEG_Z, dir)),
        data,
    ));
}

fn remove_sun(commands: &mut Commands, entity: Entity) {
    commands
        .entity(entity)
        .remove::<DirectionalLight>()
        .remove::<SunData>();
}

fn deserialize_sun(
    entity_commands: &mut EntityCommands,
    json: &serde_json::Value,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) {
    if let Ok(data) = serde_json::from_value::<SunData>(json.clone()) {
        let dir = data.direction();
        entity_commands.insert((
            DirectionalLight {
                color: Color::srgb(data.color.x, data.color.y, data.color.z),
                illuminance: data.illuminance,
                shadows_enabled: data.shadows_enabled,
                ..default()
            },
            Transform::from_rotation(Quat::from_rotation_arc(Vec3::NEG_Z, dir)),
            data,
        ));
    }
}

// ============================================================================
// Custom Inspector
// ============================================================================

fn inspect_sun(
    ui: &mut egui::Ui,
    world: &mut World,
    entity: Entity,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) -> bool {
    let Some(mut data) = world.get_mut::<SunData>(entity) else {
        return false;
    };
    let mut changed = false;

    // Azimuth
    property_row(ui, 0, |ui| {
        ui.horizontal(|ui| {
            ui.label("Azimuth");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add(
                        egui::DragValue::new(&mut data.azimuth)
                            .speed(1.0)
                            .range(0.0..=360.0)
                            .suffix("°"),
                    )
                    .changed()
                {
                    changed = true;
                }
            });
        });
    });

    // Elevation
    property_row(ui, 1, |ui| {
        ui.horizontal(|ui| {
            ui.label("Elevation");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add(
                        egui::DragValue::new(&mut data.elevation)
                            .speed(1.0)
                            .range(-90.0..=90.0)
                            .suffix("°"),
                    )
                    .changed()
                {
                    changed = true;
                }
            });
        });
    });

    // Color
    property_row(ui, 2, |ui| {
        ui.horizontal(|ui| {
            ui.label("Color");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let mut color = egui::Color32::from_rgb(
                    (data.color.x * 255.0) as u8,
                    (data.color.y * 255.0) as u8,
                    (data.color.z * 255.0) as u8,
                );
                if ui.color_edit_button_srgba(&mut color).changed() {
                    data.color = Vec3::new(
                        color.r() as f32 / 255.0,
                        color.g() as f32 / 255.0,
                        color.b() as f32 / 255.0,
                    );
                    changed = true;
                }
            });
        });
    });

    // Illuminance
    property_row(ui, 3, |ui| {
        ui.horizontal(|ui| {
            ui.label("Illuminance");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add(
                        egui::DragValue::new(&mut data.illuminance)
                            .speed(100.0)
                            .range(0.0..=f32::MAX),
                    )
                    .changed()
                {
                    changed = true;
                }
            });
        });
    });

    // Angular Diameter
    property_row(ui, 4, |ui| {
        ui.horizontal(|ui| {
            ui.label("Angular Diameter");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add(
                        egui::DragValue::new(&mut data.angular_diameter)
                            .speed(0.01)
                            .range(0.0..=10.0)
                            .suffix("°"),
                    )
                    .changed()
                {
                    changed = true;
                }
            });
        });
    });

    // Shadows
    property_row(ui, 5, |ui| {
        ui.horizontal(|ui| {
            ui.label("Shadows");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.checkbox(&mut data.shadows_enabled, "").changed() {
                    changed = true;
                }
            });
        });
    });

    changed
}

// ============================================================================
// Script Property Access
// ============================================================================

fn sun_property_meta() -> Vec<(&'static str, PropertyValueType)> {
    vec![
        ("azimuth", PropertyValueType::Float),
        ("elevation", PropertyValueType::Float),
        ("illuminance", PropertyValueType::Float),
        ("sun_color_r", PropertyValueType::Float),
        ("sun_color_g", PropertyValueType::Float),
        ("sun_color_b", PropertyValueType::Float),
        ("shadows_enabled", PropertyValueType::Bool),
        ("angular_diameter", PropertyValueType::Float),
    ]
}

fn sun_get_props(world: &World, entity: Entity) -> Vec<(&'static str, PropertyValue)> {
    let Some(data) = world.get::<SunData>(entity) else { return vec![] };
    vec![
        ("azimuth", PropertyValue::Float(data.azimuth)),
        ("elevation", PropertyValue::Float(data.elevation)),
        ("illuminance", PropertyValue::Float(data.illuminance)),
        ("sun_color_r", PropertyValue::Float(data.color.x)),
        ("sun_color_g", PropertyValue::Float(data.color.y)),
        ("sun_color_b", PropertyValue::Float(data.color.z)),
        ("shadows_enabled", PropertyValue::Bool(data.shadows_enabled)),
        ("angular_diameter", PropertyValue::Float(data.angular_diameter)),
    ]
}

fn sun_set_prop(world: &mut World, entity: Entity, prop: &str, val: &PropertyValue) -> bool {
    let Some(mut data) = world.get_mut::<SunData>(entity) else { return false };
    match prop {
        "azimuth" => { if let PropertyValue::Float(v) = val { data.azimuth = *v; true } else { false } }
        "elevation" => { if let PropertyValue::Float(v) = val { data.elevation = *v; true } else { false } }
        "illuminance" => { if let PropertyValue::Float(v) = val { data.illuminance = *v; true } else { false } }
        "shadows_enabled" => { if let PropertyValue::Bool(v) = val { data.shadows_enabled = *v; true } else { false } }
        "angular_diameter" => { if let PropertyValue::Float(v) = val { data.angular_diameter = *v; true } else { false } }
        _ => false,
    }
}

// ============================================================================
// Registration
// ============================================================================

pub fn register(registry: &mut ComponentRegistry) {
    registry.register_owned(register_component!(SunData {
        type_id: "sun",
        display_name: "Sun",
        category: ComponentCategory::Lighting,
        icon: SUN_HORIZON,
        priority: 2,
        conflicts_with: ["directional_light"],
        custom_inspector: inspect_sun,
        custom_add: add_sun,
        custom_remove: remove_sun,
        custom_deserialize: deserialize_sun,
        custom_script_properties: sun_get_props,
        custom_script_set: sun_set_prop,
        custom_script_meta: sun_property_meta,
    }));
}
