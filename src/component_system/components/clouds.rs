//! Clouds component — registration, inspector, and sync system.
//!
//! Renders procedural clouds on a sky dome mesh using a custom WGSL shader.
//! The dome follows the viewport camera and sits inside the sun disc radius.

use bevy::prelude::*;
use bevy::pbr::Material;
use bevy::render::render_resource::AsBindGroup;
use bevy::shader::ShaderRef;
use bevy_egui::egui;
use egui_phosphor::regular::CLOUD_SUN;

use crate::component_system::{ComponentCategory, ComponentRegistry};
use crate::core::{DisabledComponents, EditorEntity, ViewportCamera};
use crate::register_component;
use crate::component_system::CloudsData;
use crate::ui::inline_property;

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
        ShaderRef::Path("shaders/clouds.wgsl".into())
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
        // Disable backface culling so the dome is visible from the inside
        descriptor.primitive.cull_mode = None;
        Ok(())
    }
}

// ============================================================================
// Marker & State
// ============================================================================

/// Marker component for the cloud dome entity (not serialized)
#[derive(Component)]
pub struct CloudDomeMarker;

/// Resource tracking the cloud dome entity and its asset handles
#[derive(Resource, Default)]
pub struct CloudsState {
    pub entity: Option<Entity>,
    pub material_handle: Option<Handle<CloudMaterial>>,
    pub mesh_handle: Option<Handle<Mesh>>,
}

// ============================================================================
// Sync System
// ============================================================================

/// Spawns/updates/despawns the cloud dome to match the first active CloudsData.
pub(crate) fn sync_clouds(
    mut commands: Commands,
    mut clouds_state: ResMut<CloudsState>,
    clouds_query: Query<(&CloudsData, &EditorEntity, Option<&DisabledComponents>)>,
    camera_query: Query<(&Transform, &Camera), With<ViewportCamera>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut cloud_materials: ResMut<Assets<CloudMaterial>>,
) {
    // Find first active, non-disabled clouds component
    let active_clouds = clouds_query.iter().find(|(data, editor, dc)| {
        editor.visible && !dc.map_or(false, |d| d.is_disabled("clouds"))
    });

    let Some((clouds_data, _, _)) = active_clouds else {
        // No active clouds — despawn dome if it exists
        if let Some(dome_entity) = clouds_state.entity.take() {
            commands.entity(dome_entity).despawn();
            clouds_state.material_handle = None;
            clouds_state.mesh_handle = None;
        }
        return;
    };

    let Some((camera_transform, _)) = camera_query.iter()
        .find(|(_, cam)| cam.is_active)
        .or_else(|| camera_query.iter().next()) else {
        return;
    };

    let camera_pos = camera_transform.translation;

    // Convert wind direction from degrees to unit vector
    let wind_rad = clouds_data.wind_direction.to_radians();
    let wind_dir = Vec2::new(wind_rad.cos(), wind_rad.sin());

    // Build material uniforms
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
        // Entity exists — update material + transform
        if commands.get_entity(dome_entity).is_ok() {
            // Update material
            if let Some(ref mat_handle) = clouds_state.material_handle {
                if let Some(mat) = cloud_materials.get_mut(mat_handle) {
                    mat.params_a = params_a;
                    mat.params_b = params_b;
                    mat.cloud_color = cloud_color;
                    mat.shadow_color = shadow_color;
                }
            }

            // Follow camera
            let transform = Transform::from_translation(camera_pos)
                .with_scale(Vec3::splat(800.0));
            commands.entity(dome_entity).insert(transform);
        } else {
            // Entity was despawned externally
            clouds_state.entity = None;
            clouds_state.material_handle = None;
            clouds_state.mesh_handle = None;
        }
    }

    if clouds_state.entity.is_none() {
        // Spawn new cloud dome
        let mesh_handle = meshes.add(Sphere::new(1.0).mesh().uv(64, 32));
        let material_handle = cloud_materials.add(CloudMaterial {
            params_a,
            params_b,
            cloud_color,
            shadow_color,
        });

        let transform = Transform::from_translation(camera_pos)
            .with_scale(Vec3::splat(800.0));

        let dome_entity = commands.spawn((
            Mesh3d(mesh_handle.clone()),
            MeshMaterial3d(material_handle.clone()),
            transform,
            CloudDomeMarker,
            bevy::light::NotShadowCaster,
            bevy::light::NotShadowReceiver,
        )).id();

        clouds_state.entity = Some(dome_entity);
        clouds_state.mesh_handle = Some(mesh_handle);
        clouds_state.material_handle = Some(material_handle);
    }
}

// ============================================================================
// Inspector
// ============================================================================

fn inspect_clouds(
    ui: &mut egui::Ui,
    world: &mut World,
    entity: Entity,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) -> bool {
    let Some(mut data) = world.get_mut::<CloudsData>(entity) else {
        return false;
    };
    let mut changed = false;
    let mut row = 0;

    changed |= inline_property(ui, row, "Coverage", |ui| {
        ui.add(egui::DragValue::new(&mut data.coverage).speed(0.01).range(0.0..=1.0)).changed()
    });
    row += 1;

    changed |= inline_property(ui, row, "Density", |ui| {
        ui.add(egui::DragValue::new(&mut data.density).speed(0.01).range(0.0..=1.0)).changed()
    });
    row += 1;

    changed |= inline_property(ui, row, "Scale", |ui| {
        ui.add(egui::DragValue::new(&mut data.scale).speed(0.1).range(0.1..=50.0)).changed()
    });
    row += 1;

    changed |= inline_property(ui, row, "Speed", |ui| {
        ui.add(egui::DragValue::new(&mut data.speed).speed(0.001).range(0.0..=1.0)).changed()
    });
    row += 1;

    changed |= inline_property(ui, row, "Wind Direction", |ui| {
        ui.add(egui::DragValue::new(&mut data.wind_direction).speed(1.0).range(0.0..=360.0).suffix("°")).changed()
    });
    row += 1;

    changed |= inline_property(ui, row, "Altitude", |ui| {
        ui.add(egui::DragValue::new(&mut data.altitude).speed(0.01).range(0.0..=1.0)).changed()
    });
    row += 1;

    changed |= inline_property(ui, row, "Color", |ui| {
        let mut color = egui::Color32::from_rgb(
            (data.color.0 * 255.0) as u8,
            (data.color.1 * 255.0) as u8,
            (data.color.2 * 255.0) as u8,
        );
        let resp = ui.color_edit_button_srgba(&mut color).changed();
        if resp {
            data.color = (
                color.r() as f32 / 255.0,
                color.g() as f32 / 255.0,
                color.b() as f32 / 255.0,
            );
        }
        resp
    });
    row += 1;

    changed |= inline_property(ui, row, "Shadow Color", |ui| {
        let mut color = egui::Color32::from_rgb(
            (data.shadow_color.0 * 255.0) as u8,
            (data.shadow_color.1 * 255.0) as u8,
            (data.shadow_color.2 * 255.0) as u8,
        );
        let resp = ui.color_edit_button_srgba(&mut color).changed();
        if resp {
            data.shadow_color = (
                color.r() as f32 / 255.0,
                color.g() as f32 / 255.0,
                color.b() as f32 / 255.0,
            );
        }
        resp
    });

    changed
}

// ============================================================================
// Registration
// ============================================================================

pub fn register(registry: &mut ComponentRegistry) {
    registry.register_owned(register_component!(CloudsData {
        type_id: "clouds",
        display_name: "Clouds",
        category: ComponentCategory::Rendering,
        icon: CLOUD_SUN,
        custom_inspector: inspect_clouds,
    }));
}
