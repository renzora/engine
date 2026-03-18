//! Splatmap GPU systems — upload dirty splatmaps to textures,
//! switch chunk materials from checkerboard to splatmap when painting begins.

use bevy::prelude::*;
use bevy::image::{Image, ImageSampler};
use bevy::render::render_resource::{Extent3d, TextureFormat, TextureUsages};
use bevy::asset::RenderAssetUsages;

use crate::paint::{splatmap_to_rgba8_a, splatmap_to_rgba8_b, PaintableSurfaceData};
use crate::splatmap_material::{
    LayerColorBlock, LayerPropsBlock, TerrainLayerInfo, TerrainSplatmapMaterial,
};

/// Marker component indicating a chunk has been switched to splatmap rendering.
#[derive(Component)]
pub struct SplatmapActive {
    pub splatmap_a_handle: Handle<Image>,
    pub splatmap_b_handle: Handle<Image>,
    pub material_handle: Handle<TerrainSplatmapMaterial>,
}

/// Create a new splatmap `Image` from weight data (layers 0-3).
pub fn create_splatmap_image_a(weights: &[[f32; 8]], resolution: u32) -> Image {
    let pixels = splatmap_to_rgba8_a(weights, resolution);
    create_splatmap_from_pixels(pixels, resolution)
}

/// Create a new splatmap `Image` from weight data (layers 4-7).
pub fn create_splatmap_image_b(weights: &[[f32; 8]], resolution: u32) -> Image {
    let pixels = splatmap_to_rgba8_b(weights, resolution);
    create_splatmap_from_pixels(pixels, resolution)
}

fn create_splatmap_from_pixels(pixels: Vec<u8>, resolution: u32) -> Image {
    let size = Extent3d {
        width: resolution,
        height: resolution,
        depth_or_array_layers: 1,
    };
    let mut image = Image::new(
        size,
        bevy::render::render_resource::TextureDimension::D2,
        pixels,
        TextureFormat::Rgba8Unorm,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    );
    image.sampler = ImageSampler::linear();
    image.texture_descriptor.usage =
        TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST;
    image
}

fn pack_layer_color(layer: &crate::paint::MaterialLayer) -> Vec4 {
    Vec4::new(
        layer.color.x,
        layer.color.y,
        layer.color.z,
        layer.uv_scale.x.max(0.01),
    )
}

fn pack_layer_props(layer: &crate::paint::MaterialLayer) -> Vec4 {
    Vec4::new(
        layer.metallic,
        layer.roughness,
        layer.animation_type as u32 as f32,
        layer.animation_speed,
    )
}

/// Build a `TerrainSplatmapMaterial` from a `PaintableSurfaceData`.
pub fn build_splatmap_material(
    surface: &PaintableSurfaceData,
    splatmap_a_handle: Handle<Image>,
    splatmap_b_handle: Handle<Image>,
) -> TerrainSplatmapMaterial {
    let mut colors_a = LayerColorBlock::default();
    let mut props_a = LayerPropsBlock::default();
    let mut colors_b = LayerColorBlock {
        v0: Vec4::new(0.5, 0.5, 0.5, 0.1),
        v1: Vec4::new(0.5, 0.5, 0.5, 0.1),
        v2: Vec4::new(0.5, 0.5, 0.5, 0.1),
        v3: Vec4::new(0.5, 0.5, 0.5, 0.1),
    };
    let mut props_b = LayerPropsBlock {
        v0: Vec4::new(0.0, 0.5, 0.0, 0.0),
        v1: Vec4::new(0.0, 0.5, 0.0, 0.0),
        v2: Vec4::new(0.0, 0.5, 0.0, 0.0),
        v3: Vec4::new(0.0, 0.5, 0.0, 0.0),
    };

    // Map layers 0-3 to block A, layers 4-7 to block B
    for (i, layer) in surface.layers.iter().enumerate().take(8) {
        let color_vec = pack_layer_color(layer);
        let prop_vec = pack_layer_props(layer);
        match i {
            0 => { colors_a.v0 = color_vec; props_a.v0 = prop_vec; }
            1 => { colors_a.v1 = color_vec; props_a.v1 = prop_vec; }
            2 => { colors_a.v2 = color_vec; props_a.v2 = prop_vec; }
            3 => { colors_a.v3 = color_vec; props_a.v3 = prop_vec; }
            4 => { colors_b.v0 = color_vec; props_b.v0 = prop_vec; }
            5 => { colors_b.v1 = color_vec; props_b.v1 = prop_vec; }
            6 => { colors_b.v2 = color_vec; props_b.v2 = prop_vec; }
            7 => { colors_b.v3 = color_vec; props_b.v3 = prop_vec; }
            _ => {}
        }
    }

    TerrainSplatmapMaterial {
        layer_colors_a: colors_a,
        layer_props_a: props_a,
        splatmap_a: splatmap_a_handle,
        layer_colors_b: colors_b,
        layer_props_b: props_b,
        splatmap_b: splatmap_b_handle,
        layer_info: TerrainLayerInfo {
            layer_count: surface.layers.len().min(8) as u32,
            _pad0: 0,
            _pad1: 0,
            _pad2: 0,
        },
    }
}

/// System that uploads dirty splatmaps to GPU and updates material properties.
pub fn splatmap_upload_system(
    mut images: ResMut<Assets<Image>>,
    mut splatmap_materials: ResMut<Assets<TerrainSplatmapMaterial>>,
    mut surface_query: Query<(&mut PaintableSurfaceData, Option<&SplatmapActive>)>,
) {
    for (mut surface, splatmap_active) in surface_query.iter_mut() {
        if !surface.dirty {
            continue;
        }

        if let Some(active) = splatmap_active {
            // Update splatmap A (layers 0-3)
            let pixels_a = splatmap_to_rgba8_a(&surface.splatmap_weights, surface.splatmap_resolution);
            if let Some(image) = images.get_mut(&active.splatmap_a_handle) {
                image.data = Some(pixels_a);
            }

            // Update splatmap B (layers 4-7)
            let pixels_b = splatmap_to_rgba8_b(&surface.splatmap_weights, surface.splatmap_resolution);
            if let Some(image) = images.get_mut(&active.splatmap_b_handle) {
                image.data = Some(pixels_b);
            }

            // Update material properties (layer colors/props may have changed)
            if let Some(mat) = splatmap_materials.get_mut(&active.material_handle) {
                let updated = build_splatmap_material(
                    &surface,
                    active.splatmap_a_handle.clone(),
                    active.splatmap_b_handle.clone(),
                );
                mat.layer_colors_a = updated.layer_colors_a;
                mat.layer_props_a = updated.layer_props_a;
                mat.layer_colors_b = updated.layer_colors_b;
                mat.layer_props_b = updated.layer_props_b;
                mat.layer_info = updated.layer_info;
            }

            surface.dirty = false;
        }
    }
}

/// Activate splatmap rendering on a chunk entity.
pub fn activate_splatmap_on_chunk(
    commands: &mut Commands,
    entity: Entity,
    surface: &PaintableSurfaceData,
    images: &mut Assets<Image>,
    splatmap_materials: &mut Assets<TerrainSplatmapMaterial>,
) {
    let image_a = create_splatmap_image_a(&surface.splatmap_weights, surface.splatmap_resolution);
    let image_b = create_splatmap_image_b(&surface.splatmap_weights, surface.splatmap_resolution);
    let splatmap_a_handle = images.add(image_a);
    let splatmap_b_handle = images.add(image_b);
    let material = build_splatmap_material(surface, splatmap_a_handle.clone(), splatmap_b_handle.clone());
    let material_handle = splatmap_materials.add(material);

    commands.entity(entity).try_insert((
        MeshMaterial3d(material_handle.clone()),
        SplatmapActive {
            splatmap_a_handle,
            splatmap_b_handle,
            material_handle,
        },
    ));
}
