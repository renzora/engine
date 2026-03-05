//! Renzora Clouds — procedural cloud dome rendered with FBM noise shader.

use bevy::prelude::*;
use bevy::pbr::Material;
use bevy::render::render_resource::AsBindGroup;
use bevy::shader::ShaderRef;
#[cfg(feature = "editor")]
use egui_phosphor::regular::CLOUD_SUN;
#[cfg(feature = "editor")]
use renzora_editor::{
    FieldDef, FieldType, FieldValue, InspectorEntry, InspectorRegistry,
};
use serde::{Deserialize, Serialize};

// ============================================================================
// Data
// ============================================================================

/// Procedural clouds settings.
#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component, Serialize, Deserialize)]
pub struct CloudsData {
    pub enabled: bool,
    /// Cloud coverage (0 = clear, 1 = overcast)
    pub coverage: f32,
    /// Cloud density/opacity (0 = transparent, 1 = opaque)
    pub density: f32,
    /// Noise scale (larger = bigger cloud formations)
    pub scale: f32,
    /// Wind animation speed
    pub speed: f32,
    /// Wind direction in degrees (0-360)
    pub wind_direction: f32,
    /// Altitude threshold (0 = horizon, 1 = zenith)
    pub altitude: f32,
    /// Cloud color (lit side) RGB
    pub color: (f32, f32, f32),
    /// Shadow color (dark underside) RGB
    pub shadow_color: (f32, f32, f32),
}

impl Default for CloudsData {
    fn default() -> Self {
        Self {
            enabled: true,
            coverage: 0.5,
            density: 0.8,
            scale: 4.0,
            speed: 0.02,
            wind_direction: 45.0,
            altitude: 0.3,
            color: (1.0, 1.0, 1.0),
            shadow_color: (0.6, 0.65, 0.7),
        }
    }
}

// ============================================================================
// Cloud Material
// ============================================================================

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct CloudMaterial {
    /// coverage, density, scale, speed
    #[uniform(0)]
    pub params_a: Vec4,
    /// wind_dir_x, wind_dir_y, altitude, unused
    #[uniform(1)]
    pub params_b: Vec4,
    /// Cloud lit color
    #[uniform(2)]
    pub cloud_color: LinearRgba,
    /// Cloud shadow color
    #[uniform(3)]
    pub shadow_color: LinearRgba,
}

impl Material for CloudMaterial {
    fn fragment_shader() -> ShaderRef {
        ShaderRef::Path("embedded://renzora_clouds/clouds.wgsl".into())
    }

    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Blend
    }

    fn specialize(
        _pipeline: &bevy::pbr::MaterialPipeline,
        descriptor: &mut bevy::render::render_resource::RenderPipelineDescriptor,
        _layout: &bevy::mesh::MeshVertexBufferLayoutRef,
        _key: bevy::pbr::MaterialPipelineKey<Self>,
    ) -> Result<(), bevy::render::render_resource::SpecializedMeshPipelineError> {
        descriptor.primitive.cull_mode = None;
        Ok(())
    }
}

// ============================================================================
// Marker & State
// ============================================================================

#[derive(Component)]
struct CloudDomeMarker;

#[derive(Resource, Default)]
struct CloudsState {
    entity: Option<Entity>,
    material_handle: Option<Handle<CloudMaterial>>,
    mesh_handle: Option<Handle<Mesh>>,
}

// ============================================================================
// Sync System
// ============================================================================

fn sync_clouds(
    mut commands: Commands,
    mut clouds_state: ResMut<CloudsState>,
    clouds_query: Query<&CloudsData>,
    camera_query: Query<(&Transform, &Camera), With<renzora_core::EditorCamera>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut cloud_materials: ResMut<Assets<CloudMaterial>>,
) {
    // Find first active clouds component
    let active_clouds = clouds_query.iter().next();

    let Some(clouds_data) = active_clouds else {
        // No clouds — despawn dome if it exists
        if let Some(dome_entity) = clouds_state.entity.take() {
            commands.entity(dome_entity).despawn();
            clouds_state.material_handle = None;
            clouds_state.mesh_handle = None;
        }
        return;
    };

    let Some((camera_transform, _)) = camera_query.iter()
        .find(|(_, cam)| cam.is_active)
        .or_else(|| camera_query.iter().next())
    else {
        return;
    };

    let camera_pos = camera_transform.translation;

    let wind_rad = clouds_data.wind_direction.to_radians();
    let wind_dir = Vec2::new(wind_rad.cos(), wind_rad.sin());

    let params_a = Vec4::new(
        clouds_data.coverage,
        clouds_data.density,
        clouds_data.scale,
        clouds_data.speed,
    );
    let params_b = Vec4::new(wind_dir.x, wind_dir.y, clouds_data.altitude, 0.0);
    let cloud_color = LinearRgba::new(
        clouds_data.color.0,
        clouds_data.color.1,
        clouds_data.color.2,
        1.0,
    );
    let shadow_color = LinearRgba::new(
        clouds_data.shadow_color.0,
        clouds_data.shadow_color.1,
        clouds_data.shadow_color.2,
        1.0,
    );

    if let Some(dome_entity) = clouds_state.entity {
        if commands.get_entity(dome_entity).is_ok() {
            if let Some(ref mat_handle) = clouds_state.material_handle {
                if let Some(mat) = cloud_materials.get_mut(mat_handle) {
                    mat.params_a = params_a;
                    mat.params_b = params_b;
                    mat.cloud_color = cloud_color;
                    mat.shadow_color = shadow_color;
                }
            }
            let transform =
                Transform::from_translation(camera_pos).with_scale(Vec3::splat(800.0));
            commands.entity(dome_entity).insert(transform);
        } else {
            clouds_state.entity = None;
            clouds_state.material_handle = None;
            clouds_state.mesh_handle = None;
        }
    }

    if clouds_state.entity.is_none() {
        let mesh_handle = meshes.add(Sphere::new(1.0).mesh().uv(64, 32));
        let material_handle = cloud_materials.add(CloudMaterial {
            params_a,
            params_b,
            cloud_color,
            shadow_color,
        });

        let transform =
            Transform::from_translation(camera_pos).with_scale(Vec3::splat(800.0));

        let dome_entity = commands
            .spawn((
                Mesh3d(mesh_handle.clone()),
                MeshMaterial3d(material_handle.clone()),
                transform,
                CloudDomeMarker,
                bevy::light::NotShadowCaster,
                bevy::light::NotShadowReceiver,
            ))
            .id();

        clouds_state.entity = Some(dome_entity);
        clouds_state.mesh_handle = Some(mesh_handle);
        clouds_state.material_handle = Some(material_handle);
    }
}

// ============================================================================
// Inspector
// ============================================================================

#[cfg(feature = "editor")]
fn inspector_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "clouds",
        display_name: "Clouds",
        icon: CLOUD_SUN,
        category: "rendering",
        has_fn: |world, entity| world.get::<CloudsData>(entity).is_some(),
        add_fn: Some(|world, entity| {
            world.entity_mut(entity).insert(CloudsData::default());
        }),
        remove_fn: Some(|world, entity| {
            world.entity_mut(entity).remove::<CloudsData>();
        }),
        is_enabled_fn: Some(|world, entity| {
            world
                .get::<CloudsData>(entity)
                .map(|d| d.enabled)
                .unwrap_or(false)
        }),
        set_enabled_fn: Some(|world, entity, val| {
            if let Some(mut d) = world.get_mut::<CloudsData>(entity) {
                d.enabled = val;
            }
        }),
        fields: vec![
            FieldDef {
                name: "Coverage",
                field_type: FieldType::Float {
                    speed: 0.01,
                    min: 0.0,
                    max: 1.0,
                },
                get_fn: |world, entity| {
                    world
                        .get::<CloudsData>(entity)
                        .map(|d| FieldValue::Float(d.coverage))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Float(v) = val {
                        if let Some(mut d) = world.get_mut::<CloudsData>(entity) {
                            d.coverage = v;
                        }
                    }
                },
            },
            FieldDef {
                name: "Density",
                field_type: FieldType::Float {
                    speed: 0.01,
                    min: 0.0,
                    max: 1.0,
                },
                get_fn: |world, entity| {
                    world
                        .get::<CloudsData>(entity)
                        .map(|d| FieldValue::Float(d.density))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Float(v) = val {
                        if let Some(mut d) = world.get_mut::<CloudsData>(entity) {
                            d.density = v;
                        }
                    }
                },
            },
            FieldDef {
                name: "Scale",
                field_type: FieldType::Float {
                    speed: 0.1,
                    min: 0.1,
                    max: 50.0,
                },
                get_fn: |world, entity| {
                    world
                        .get::<CloudsData>(entity)
                        .map(|d| FieldValue::Float(d.scale))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Float(v) = val {
                        if let Some(mut d) = world.get_mut::<CloudsData>(entity) {
                            d.scale = v;
                        }
                    }
                },
            },
            FieldDef {
                name: "Speed",
                field_type: FieldType::Float {
                    speed: 0.001,
                    min: 0.0,
                    max: 1.0,
                },
                get_fn: |world, entity| {
                    world
                        .get::<CloudsData>(entity)
                        .map(|d| FieldValue::Float(d.speed))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Float(v) = val {
                        if let Some(mut d) = world.get_mut::<CloudsData>(entity) {
                            d.speed = v;
                        }
                    }
                },
            },
            FieldDef {
                name: "Wind Direction",
                field_type: FieldType::Float {
                    speed: 1.0,
                    min: 0.0,
                    max: 360.0,
                },
                get_fn: |world, entity| {
                    world
                        .get::<CloudsData>(entity)
                        .map(|d| FieldValue::Float(d.wind_direction))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Float(v) = val {
                        if let Some(mut d) = world.get_mut::<CloudsData>(entity) {
                            d.wind_direction = v;
                        }
                    }
                },
            },
            FieldDef {
                name: "Altitude",
                field_type: FieldType::Float {
                    speed: 0.01,
                    min: 0.0,
                    max: 1.0,
                },
                get_fn: |world, entity| {
                    world
                        .get::<CloudsData>(entity)
                        .map(|d| FieldValue::Float(d.altitude))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Float(v) = val {
                        if let Some(mut d) = world.get_mut::<CloudsData>(entity) {
                            d.altitude = v;
                        }
                    }
                },
            },
            FieldDef {
                name: "Color",
                field_type: FieldType::Color,
                get_fn: |world, entity| {
                    world
                        .get::<CloudsData>(entity)
                        .map(|d| FieldValue::Color([d.color.0, d.color.1, d.color.2]))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Color([r, g, b]) = val {
                        if let Some(mut d) = world.get_mut::<CloudsData>(entity) {
                            d.color = (r, g, b);
                        }
                    }
                },
            },
            FieldDef {
                name: "Shadow Color",
                field_type: FieldType::Color,
                get_fn: |world, entity| {
                    world
                        .get::<CloudsData>(entity)
                        .map(|d| {
                            FieldValue::Color([
                                d.shadow_color.0,
                                d.shadow_color.1,
                                d.shadow_color.2,
                            ])
                        })
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Color([r, g, b]) = val {
                        if let Some(mut d) = world.get_mut::<CloudsData>(entity) {
                            d.shadow_color = (r, g, b);
                        }
                    }
                },
            },
        ],
        custom_ui_fn: None,
    }
}

// ============================================================================
// Plugin
// ============================================================================

pub struct CloudsPlugin;

impl Plugin for CloudsPlugin {
    fn build(&self, app: &mut App) {
        bevy::asset::embedded_asset!(app, "clouds.wgsl");
        app.register_type::<CloudsData>()
            .add_plugins(MaterialPlugin::<CloudMaterial>::default())
            .init_resource::<CloudsState>()
            .add_systems(Update, sync_clouds);

        #[cfg(feature = "editor")]
        {
            app.init_resource::<InspectorRegistry>();
            let world = app.world_mut();
            if let Some(mut registry) = world.get_resource_mut::<InspectorRegistry>() {
                registry.register(inspector_entry());
            }
        }
    }
}
