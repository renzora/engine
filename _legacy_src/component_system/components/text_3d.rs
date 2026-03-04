//! 3D Text component definition
//!
//! Text is rendered off-screen via a Camera2d to a render-target texture,
//! which is then applied to a Mesh3d quad positioned in world space.
//! This gives true 3D perspective — the text moves, rotates, and scales
//! exactly like any other mesh in the scene.

use bevy::prelude::*;
use bevy::camera::{RenderTarget, visibility::RenderLayers};
use bevy::render::render_resource::{
    Extent3d, TextureDescriptor, TextureDimension, TextureFormat, TextureUsages,
};
use bevy_egui::egui;
use serde_json::json;

use crate::component_system::{ComponentCategory, ComponentRegistry};
use crate::register_component;
use crate::component_system::Text3DData;
use crate::ui::property_row;

use egui_phosphor::regular::TEXT_T;

// Render layer reserved for off-screen text cameras/glyphs (not seen by scene cameras)
const TEXT_3D_RENDER_LAYER: usize = 3;

// Render target dimensions. Aspect ratio 4:1 matches the default quad.
const TEXT_3D_IMAGE_WIDTH: u32 = 512;
const TEXT_3D_IMAGE_HEIGHT: u32 = 128;

// ============================================================================
// Runtime State (non-serializable)
// ============================================================================

/// Tracks the off-screen rendering infrastructure for one Text3D entity.
/// Created by `text3d_setup_system`, cleaned up by `text3d_cleanup_system`.
#[derive(Component)]
pub struct Text3DRenderState {
    /// The Camera2d that renders text to `image_handle`
    camera_entity: Entity,
    /// The Text2d glyph entity rendered by the camera
    text_entity: Entity,
    /// The render-target image used as a material texture on the 3D quad
    pub image_handle: Handle<Image>,
}

// ============================================================================
// Systems
// ============================================================================

/// Runs when `Text3DData` is first added. Creates:
/// - A render-target `Image`
/// - An off-screen `Camera2d` that renders to the image
/// - A `Text2d` glyph entity visible only to that camera
/// - A `Mesh3d` quad + `MeshMaterial3d` on the owning entity, textured with the image
pub fn text3d_setup_system(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
    query: Query<(Entity, &Text3DData), Added<Text3DData>>,
) {
    for (entity, data) in query.iter() {
        // --- Render target image ---
        let size = Extent3d {
            width: TEXT_3D_IMAGE_WIDTH,
            height: TEXT_3D_IMAGE_HEIGHT,
            depth_or_array_layers: 1,
        };
        let mut image = Image {
            texture_descriptor: TextureDescriptor {
                label: Some("text_3d_render_target"),
                size,
                dimension: TextureDimension::D2,
                format: TextureFormat::Bgra8UnormSrgb,
                mip_level_count: 1,
                sample_count: 1,
                usage: TextureUsages::TEXTURE_BINDING
                    | TextureUsages::COPY_DST
                    | TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            },
            ..default()
        };
        image.resize(size);
        let image_handle = images.add(image);

        // Offset each text entity's off-screen world position so cameras don't overlap.
        // entity.to_bits() gives a stable u64 unique per entity; cast to f32 is exact
        // for the small index values typical in an editor scene.
        let offset_x = entity.to_bits() as f32 * 10000.0;
        let text_layer = RenderLayers::layer(TEXT_3D_RENDER_LAYER);

        // --- Off-screen Camera2d ---
        // RenderTarget is a separate component, not a field of Camera
        let camera_entity = commands.spawn((
            Camera2d,
            Camera {
                clear_color: ClearColorConfig::Custom(Color::srgba(0.0, 0.0, 0.0, 0.0)),
                order: -100,
                ..default()
            },
            RenderTarget::Image(image_handle.clone().into()),
            Transform::from_xyz(offset_x, 0.0, 999.0),
            text_layer.clone(),
            Name::new(format!("Text3D Camera [{:?}]", entity)),
        )).id();

        // --- Text2d glyph entity ---
        let text_entity = commands.spawn((
            Text2d::new(data.text.clone()),
            TextFont {
                font_size: data.font_size,
                ..default()
            },
            TextColor(Color::srgba(data.color.x, data.color.y, data.color.z, data.color.w)),
            Transform::from_xyz(offset_x, 0.0, 0.0),
            text_layer,
            Name::new(format!("Text3D Glyph [{:?}]", entity)),
        )).id();

        // --- 3D quad mesh ---
        let aspect = TEXT_3D_IMAGE_WIDTH as f32 / TEXT_3D_IMAGE_HEIGHT as f32;
        let quad_height = 0.5_f32;
        let quad_width = quad_height * aspect;

        let mesh = meshes.add(Rectangle::new(quad_width, quad_height));
        let material = materials.add(StandardMaterial {
            base_color_texture: Some(image_handle.clone()),
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            double_sided: true,
            cull_mode: None,
            ..default()
        });

        commands.entity(entity).insert((
            Mesh3d(mesh),
            MeshMaterial3d(material),
            Text3DRenderState {
                camera_entity,
                text_entity,
                image_handle,
            },
        ));
    }
}

/// Propagates `Text3DData` changes to the underlying off-screen `Text2d` entity.
pub fn text3d_sync_system(
    text_3d_query: Query<(&Text3DData, &Text3DRenderState), Changed<Text3DData>>,
    mut text_query: Query<(&mut Text2d, &mut TextFont, &mut TextColor)>,
) {
    for (data, state) in text_3d_query.iter() {
        let Ok((mut text2d, mut font, mut color)) = text_query.get_mut(state.text_entity) else {
            continue;
        };
        text2d.0 = data.text.clone();
        font.font_size = data.font_size;
        color.0 = Color::srgba(data.color.x, data.color.y, data.color.z, data.color.w);
    }
}

/// Despawns orphaned off-screen camera/glyph entities when `Text3DData` is removed.
pub fn text3d_cleanup_system(
    mut commands: Commands,
    query: Query<(Entity, &Text3DRenderState), Without<Text3DData>>,
) {
    for (entity, state) in query.iter() {
        commands.entity(state.camera_entity).despawn();
        commands.entity(state.text_entity).despawn();
        commands.entity(entity).remove::<(
            Text3DRenderState,
            Mesh3d,
            MeshMaterial3d<StandardMaterial>,
        )>();
    }
}

// ============================================================================
// Custom Add / Remove / Serialize / Deserialize
// ============================================================================

fn add_text_3d(
    commands: &mut Commands,
    entity: Entity,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) {
    // Insert data only — text3d_setup_system handles the rendering infrastructure
    commands.entity(entity).insert(Text3DData::default());
}

fn remove_text_3d(commands: &mut Commands, entity: Entity) {
    // Remove data + visible mesh components.
    // Text3DRenderState remains temporarily; text3d_cleanup_system despawns
    // the orphaned camera/glyph entities and removes the state next frame.
    commands.entity(entity)
        .remove::<Text3DData>()
        .remove::<Mesh3d>()
        .remove::<MeshMaterial3d<StandardMaterial>>();
}

fn serialize_text_3d(world: &World, entity: Entity) -> Option<serde_json::Value> {
    let data = world.get::<Text3DData>(entity)?;
    Some(json!({
        "text": data.text,
        "font_size": data.font_size,
        "color": [data.color.x, data.color.y, data.color.z, data.color.w],
    }))
}

fn deserialize_text_3d(
    entity_commands: &mut EntityCommands,
    data: &serde_json::Value,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) {
    let text = data
        .get("text")
        .and_then(|v| v.as_str())
        .unwrap_or("Text")
        .to_string();

    let font_size = data
        .get("font_size")
        .and_then(|v| v.as_f64())
        .unwrap_or(64.0) as f32;

    let color = data
        .get("color")
        .and_then(|c| c.as_array())
        .map(|arr| Vec4::new(
            arr.first().and_then(|v| v.as_f64()).unwrap_or(1.0) as f32,
            arr.get(1).and_then(|v| v.as_f64()).unwrap_or(1.0) as f32,
            arr.get(2).and_then(|v| v.as_f64()).unwrap_or(1.0) as f32,
            arr.get(3).and_then(|v| v.as_f64()).unwrap_or(1.0) as f32,
        ))
        .unwrap_or(Vec4::ONE);

    // Insert data only — text3d_setup_system handles rendering infrastructure
    entity_commands.insert(Text3DData { text, font_size, color });
}

// ============================================================================
// Custom Inspector
// ============================================================================

fn inspect_text_3d(
    ui: &mut egui::Ui,
    world: &mut World,
    entity: Entity,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) -> bool {
    let mut changed = false;

    let Some(data) = world.get::<Text3DData>(entity) else {
        return false;
    };
    let mut text = data.text.clone();
    let mut font_size = data.font_size;
    let mut color = data.color;

    // Text
    property_row(ui, 0, |ui| {
        ui.horizontal(|ui| {
            ui.label(&crate::locale::t("comp.text_3d.text"));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.text_edit_singleline(&mut text).changed() {
                    changed = true;
                }
            });
        });
    });

    // Font Size
    property_row(ui, 1, |ui| {
        ui.horizontal(|ui| {
            ui.label(&crate::locale::t("comp.text_3d.font_size"));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add(egui::DragValue::new(&mut font_size).speed(0.5).range(1.0..=512.0))
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
            ui.label(&crate::locale::t("comp.text_3d.color"));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let mut egui_color = egui::Color32::from_rgba_unmultiplied(
                    (color.x * 255.0) as u8,
                    (color.y * 255.0) as u8,
                    (color.z * 255.0) as u8,
                    (color.w * 255.0) as u8,
                );
                if ui.color_edit_button_srgba(&mut egui_color).changed() {
                    color = Vec4::new(
                        egui_color.r() as f32 / 255.0,
                        egui_color.g() as f32 / 255.0,
                        egui_color.b() as f32 / 255.0,
                        egui_color.a() as f32 / 255.0,
                    );
                    changed = true;
                }
            });
        });
    });

    if changed {
        if let Some(mut d) = world.get_mut::<Text3DData>(entity) {
            d.text = text;
            d.font_size = font_size;
            d.color = color;
        }
    }

    changed
}

// ============================================================================
// Registration
// ============================================================================

pub fn register(registry: &mut ComponentRegistry) {
    registry.register_owned(register_component!(Text3DData {
        type_id: "text_3d",
        display_name: "Text 3D",
        category: ComponentCategory::Rendering,
        icon: TEXT_T,
        priority: 2,
        conflicts_with: ["mesh_renderer", "sprite_2d"],
        custom_inspector: inspect_text_3d,
        custom_add: add_text_3d,
        custom_remove: remove_text_3d,
        custom_serialize: serialize_text_3d,
        custom_deserialize: deserialize_text_3d,
    }));
}
