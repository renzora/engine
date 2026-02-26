//! Night Stars component — registration, inspector, and sync system.
//!
//! Renders a procedural starfield on a sky dome mesh using a custom WGSL shader.
//! The dome follows the viewport camera. Stars are placed via a 3D voxel grid
//! (seam-free) with per-star brightness variation and optional twinkling.

use bevy::prelude::*;
use bevy::pbr::Material;
use bevy::render::render_resource::AsBindGroup;
use bevy::shader::ShaderRef;
use bevy_egui::egui;
use egui_phosphor::regular::MOON_STARS;

use crate::component_system::{ComponentCategory, ComponentRegistry, NightStarsData};
use crate::core::{DisabledComponents, EditorEntity, ViewportCamera};
use crate::register_component;
use crate::ui::inline_property;

// ============================================================================
// Star Material
// ============================================================================

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct NightStarsMaterial {
    /// density, brightness, star_size, twinkle_speed
    #[uniform(0)]
    pub params_a: Vec4,
    /// twinkle_amount, horizon_fade, unused, unused
    #[uniform(1)]
    pub params_b: Vec4,
    /// Star color tint (r, g, b, unused)
    #[uniform(2)]
    pub star_color: LinearRgba,
}

impl Material for NightStarsMaterial {
    fn fragment_shader() -> ShaderRef {
        ShaderRef::Path("shaders/night_stars.wgsl".into())
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

/// Marker for the night stars dome entity (not serialized)
#[derive(Component)]
pub struct NightStarsDomeMarker;

/// Resource tracking the dome entity and its asset handles
#[derive(Resource, Default)]
pub struct NightStarsState {
    pub entity: Option<Entity>,
    pub material_handle: Option<Handle<NightStarsMaterial>>,
    pub mesh_handle: Option<Handle<Mesh>>,
}

// ============================================================================
// Sync System
// ============================================================================

/// Spawns/updates/despawns the star dome to match the first active NightStarsData.
pub(crate) fn sync_night_stars(
    mut commands: Commands,
    mut state: ResMut<NightStarsState>,
    stars_query: Query<(&NightStarsData, &EditorEntity, Option<&DisabledComponents>)>,
    camera_query: Query<(&Transform, &Camera), With<ViewportCamera>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut star_materials: ResMut<Assets<NightStarsMaterial>>,
) {
    // Find the first active, non-disabled night stars component
    let active = stars_query.iter().find(|(_, editor, dc)| {
        editor.visible && !dc.map_or(false, |d| d.is_disabled("night_stars"))
    });

    let Some((data, _, _)) = active else {
        // No active component — despawn dome
        if let Some(ent) = state.entity.take() {
            commands.entity(ent).despawn();
            state.material_handle = None;
            state.mesh_handle = None;
        }
        return;
    };

    let Some((camera_transform, _)) = camera_query
        .iter()
        .find(|(_, cam)| cam.is_active)
        .or_else(|| camera_query.iter().next())
    else {
        return;
    };

    let camera_pos = camera_transform.translation;

    let params_a = Vec4::new(data.density, data.brightness, data.star_size, data.twinkle_speed);
    let params_b = Vec4::new(data.twinkle_amount, data.horizon_fade, 0.0, 0.0);
    let star_color = LinearRgba::new(data.color.0, data.color.1, data.color.2, 1.0);

    if let Some(dome_entity) = state.entity {
        if commands.get_entity(dome_entity).is_ok() {
            // Update material uniforms
            if let Some(ref mat_handle) = state.material_handle {
                if let Some(mat) = star_materials.get_mut(mat_handle) {
                    mat.params_a = params_a;
                    mat.params_b = params_b;
                    mat.star_color = star_color;
                }
            }
            // Follow camera
            let transform = Transform::from_translation(camera_pos).with_scale(Vec3::splat(800.0));
            commands.entity(dome_entity).insert(transform);
        } else {
            state.entity = None;
            state.material_handle = None;
            state.mesh_handle = None;
        }
    }

    if state.entity.is_none() {
        let mesh_handle = meshes.add(Sphere::new(1.0).mesh().uv(64, 32));
        let material_handle = star_materials.add(NightStarsMaterial {
            params_a,
            params_b,
            star_color,
        });
        let transform = Transform::from_translation(camera_pos).with_scale(Vec3::splat(800.0));

        let dome_entity = commands
            .spawn((
                Mesh3d(mesh_handle.clone()),
                MeshMaterial3d(material_handle.clone()),
                transform,
                NightStarsDomeMarker,
                bevy::light::NotShadowCaster,
                bevy::light::NotShadowReceiver,
            ))
            .id();

        state.entity = Some(dome_entity);
        state.mesh_handle = Some(mesh_handle);
        state.material_handle = Some(material_handle);
    }
}

// ============================================================================
// Inspector
// ============================================================================

fn inspect_night_stars(
    ui: &mut egui::Ui,
    world: &mut World,
    entity: Entity,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) -> bool {
    let Some(mut data) = world.get_mut::<NightStarsData>(entity) else {
        return false;
    };
    let mut changed = false;
    let mut row = 0;

    changed |= inline_property(ui, row, "Density", |ui| {
        ui.add(egui::DragValue::new(&mut data.density).speed(0.01).range(0.0..=1.0)).changed()
    });
    row += 1;

    changed |= inline_property(ui, row, "Brightness", |ui| {
        ui.add(egui::DragValue::new(&mut data.brightness).speed(0.05).range(0.0..=10.0)).changed()
    });
    row += 1;

    changed |= inline_property(ui, row, "Star Size", |ui| {
        ui.add(egui::DragValue::new(&mut data.star_size).speed(0.05).range(0.2..=5.0)).changed()
    });
    row += 1;

    changed |= inline_property(ui, row, "Twinkle Speed", |ui| {
        ui.add(egui::DragValue::new(&mut data.twinkle_speed).speed(0.05).range(0.0..=10.0)).changed()
    });
    row += 1;

    changed |= inline_property(ui, row, "Twinkle Amount", |ui| {
        ui.add(egui::DragValue::new(&mut data.twinkle_amount).speed(0.01).range(0.0..=1.0)).changed()
    });
    row += 1;

    changed |= inline_property(ui, row, "Horizon Fade", |ui| {
        ui.add(egui::DragValue::new(&mut data.horizon_fade).speed(0.01).range(0.0..=1.0)).changed()
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

    changed
}

// ============================================================================
// Registration
// ============================================================================

pub fn register(registry: &mut ComponentRegistry) {
    registry.register_owned(register_component!(NightStarsData {
        type_id: "night_stars",
        display_name: "Night Stars",
        category: ComponentCategory::Rendering,
        icon: MOON_STARS,
        custom_inspector: inspect_night_stars,
    }));
}
