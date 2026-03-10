//! Renzora Night Stars — procedural starfield on a sky dome.

use bevy::pbr::Material;
use bevy::prelude::*;
use bevy::render::render_resource::AsBindGroup;
use bevy::shader::ShaderRef;
use serde::{Deserialize, Serialize};

#[cfg(feature = "editor")]
use bevy_egui::egui;
#[cfg(feature = "editor")]
use egui_phosphor::regular::MOON_STARS;
#[cfg(feature = "editor")]
use renzora_editor::{get_theme_colors, inline_property, AppEditorExt, EditorCommands, InspectorEntry};
#[cfg(feature = "editor")]
use renzora_theme::Theme;

// ============================================================================
// Data types
// ============================================================================

/// Night stars — procedural starfield rendered on a sky dome.
#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component, Serialize, Deserialize)]
pub struct NightStarsData {
    /// Star density (0 = very few, 1 = dense starfield)
    pub density: f32,
    /// Brightness multiplier (0..10)
    pub brightness: f32,
    /// Star angular size (0.2 = tiny dots, 5.0 = large blobs)
    pub star_size: f32,
    /// Twinkling animation speed (0 = static, 10 = fast)
    pub twinkle_speed: f32,
    /// Twinkling intensity (0 = no twinkle, 1 = strong)
    pub twinkle_amount: f32,
    /// Elevation angle at which stars fade in above the horizon (0..1)
    pub horizon_fade: f32,
    /// Star color tint (RGB)
    pub color: (f32, f32, f32),
}

impl Default for NightStarsData {
    fn default() -> Self {
        Self {
            density: 0.55,
            brightness: 1.5,
            star_size: 1.2,
            twinkle_speed: 1.0,
            twinkle_amount: 0.35,
            horizon_fade: 0.08,
            color: (1.0, 0.97, 0.9),
        }
    }
}

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
        ShaderRef::Path("embedded://renzora_night_stars/night_stars.wgsl".into())
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
pub struct NightStarsDomeMarker;

#[derive(Resource, Default)]
pub struct NightStarsState {
    pub entity: Option<Entity>,
    pub material_handle: Option<Handle<NightStarsMaterial>>,
    pub mesh_handle: Option<Handle<Mesh>>,
}

// ============================================================================
// Sync System
// ============================================================================

fn sync_night_stars(
    mut commands: Commands,
    mut state: ResMut<NightStarsState>,
    stars_query: Query<&NightStarsData>,
    camera_query: Query<
        &Transform,
        (With<Camera3d>, Without<renzora_core::IsolatedCamera>),
    >,
    mut meshes: ResMut<Assets<Mesh>>,
    mut star_materials: ResMut<Assets<NightStarsMaterial>>,
    has_data: Query<(), With<NightStarsData>>,
    mut removed: RemovedComponents<NightStarsData>,
) {
    let had_removals = removed.read().count() > 0;
    if had_removals && has_data.is_empty() {
        if let Some(ent) = state.entity.take() {
            commands.entity(ent).despawn();
            state.material_handle = None;
            state.mesh_handle = None;
        }
        return;
    }

    let Some(data) = stars_query.iter().next() else {
        return;
    };

    let Some(camera_transform) = camera_query.iter().next() else {
        return;
    };

    let camera_pos = camera_transform.translation;

    let params_a = Vec4::new(data.density, data.brightness, data.star_size, data.twinkle_speed);
    let params_b = Vec4::new(data.twinkle_amount, data.horizon_fade, 0.0, 0.0);
    let star_color = LinearRgba::new(data.color.0, data.color.1, data.color.2, 1.0);

    if let Some(dome_entity) = state.entity {
        if commands.get_entity(dome_entity).is_ok() {
            if let Some(ref mat_handle) = state.material_handle {
                if let Some(mat) = star_materials.get_mut(mat_handle) {
                    mat.params_a = params_a;
                    mat.params_b = params_b;
                    mat.star_color = star_color;
                }
            }
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
// Custom inspector UI (editor only)
// ============================================================================

#[cfg(feature = "editor")]
fn rgb_to_color32(r: f32, g: f32, b: f32) -> egui::Color32 {
    egui::Color32::from_rgb(
        (r * 255.0) as u8,
        (g * 255.0) as u8,
        (b * 255.0) as u8,
    )
}

#[cfg(feature = "editor")]
fn color32_to_rgb(color: egui::Color32) -> (f32, f32, f32) {
    (
        color.r() as f32 / 255.0,
        color.g() as f32 / 255.0,
        color.b() as f32 / 255.0,
    )
}

#[cfg(feature = "editor")]
fn night_stars_custom_ui(
    ui: &mut egui::Ui,
    world: &World,
    entity: Entity,
    cmds: &EditorCommands,
    theme: &Theme,
) {
    let _theme_colors = get_theme_colors(ui.ctx());

    let Some(stars) = world.get::<NightStarsData>(entity) else {
        return;
    };

    let mut data = stars.clone();
    let mut changed = false;
    let mut row = 0;

    changed |= inline_property(ui, row, "Density", theme, |ui| {
        ui.add(egui::DragValue::new(&mut data.density).speed(0.01).range(0.0..=1.0)).changed()
    });
    row += 1;

    changed |= inline_property(ui, row, "Brightness", theme, |ui| {
        ui.add(egui::DragValue::new(&mut data.brightness).speed(0.05).range(0.0..=10.0)).changed()
    });
    row += 1;

    changed |= inline_property(ui, row, "Star Size", theme, |ui| {
        ui.add(egui::DragValue::new(&mut data.star_size).speed(0.05).range(0.2..=5.0)).changed()
    });
    row += 1;

    changed |= inline_property(ui, row, "Twinkle Speed", theme, |ui| {
        ui.add(egui::DragValue::new(&mut data.twinkle_speed).speed(0.05).range(0.0..=10.0)).changed()
    });
    row += 1;

    changed |= inline_property(ui, row, "Twinkle Amount", theme, |ui| {
        ui.add(egui::DragValue::new(&mut data.twinkle_amount).speed(0.01).range(0.0..=1.0)).changed()
    });
    row += 1;

    changed |= inline_property(ui, row, "Horizon Fade", theme, |ui| {
        ui.add(egui::DragValue::new(&mut data.horizon_fade).speed(0.01).range(0.0..=1.0)).changed()
    });
    row += 1;

    changed |= inline_property(ui, row, "Color", theme, |ui| {
        let mut color = rgb_to_color32(data.color.0, data.color.1, data.color.2);
        let resp = ui.color_edit_button_srgba(&mut color).changed();
        if resp {
            data.color = color32_to_rgb(color);
        }
        resp
    });

    if changed {
        let new_data = data;
        cmds.push(move |world: &mut World| {
            if let Some(mut stars) = world.get_mut::<NightStarsData>(entity) {
                *stars = new_data;
            }
        });
    }
}

// ============================================================================
// Inspector entry (editor only)
// ============================================================================

#[cfg(feature = "editor")]
fn inspector_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "night_stars",
        display_name: "Night Stars",
        icon: MOON_STARS,
        category: "rendering",
        has_fn: |world, entity| world.get::<NightStarsData>(entity).is_some(),
        add_fn: Some(|world, entity| {
            world.entity_mut(entity).insert(NightStarsData::default());
        }),
        remove_fn: Some(|world, entity| {
            world.entity_mut(entity).remove::<NightStarsData>();
        }),
        is_enabled_fn: None,
        set_enabled_fn: None,
        fields: vec![],
        custom_ui_fn: Some(night_stars_custom_ui),
    }
}

// ============================================================================
// Plugin
// ============================================================================

pub struct NightStarsPlugin;

impl Plugin for NightStarsPlugin {
    fn build(&self, app: &mut App) {
        info!("[runtime] NightStarsPlugin");
        bevy::asset::embedded_asset!(app, "night_stars.wgsl");

        app.register_type::<NightStarsData>()
            .init_resource::<NightStarsState>()
            .add_plugins(MaterialPlugin::<NightStarsMaterial>::default())
            .add_systems(Update, sync_night_stars);

        #[cfg(feature = "editor")]
        app.register_inspector(inspector_entry());
    }
}
