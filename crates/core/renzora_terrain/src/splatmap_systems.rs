//! Splatmap GPU systems — upload dirty splatmaps to textures,
//! switch chunk materials from checkerboard to splatmap when painting begins.

use bevy::prelude::*;
use bevy::image::{Image, ImageSampler};
use bevy::render::render_resource::{Extent3d, TextureFormat, TextureUsages};
use bevy::asset::RenderAssetUsages;

use crate::paint::{splatmap_to_rgba8_a, splatmap_to_rgba8_b, PaintableSurfaceData};
use crate::splatmap_material::{
    LayerColorBlock, LayerPropsBlock, LayerTexFlags, TerrainLayerInfo, TerrainSplatmapMaterial,
};

/// Standardized resolution for all texture array slices.
pub const LAYER_TEX_SIZE: u32 = 512;

/// Resource tracking per-layer textures for the terrain splatmap material.
#[derive(Resource)]
pub struct TerrainLayerTextures {
    pub layer_albedo: [Option<Handle<Image>>; 8],
    pub layer_normal: [Option<Handle<Image>>; 8],
    pub layer_arm: [Option<Handle<Image>>; 8],
    pub albedo_array: Handle<Image>,
    pub normal_array: Handle<Image>,
    pub arm_array: Handle<Image>,
    pub flags: u32,
    pub dirty: bool,
}

impl FromWorld for TerrainLayerTextures {
    fn from_world(world: &mut World) -> Self {
        let mut images = world.resource_mut::<Assets<Image>>();
        let albedo_array = images.add(create_fallback_array(
            [255, 255, 255, 255],
            TextureFormat::Rgba8UnormSrgb,
        ));
        let normal_array = images.add(create_fallback_array(
            [128, 128, 255, 255],
            TextureFormat::Rgba8Unorm,
        ));
        let arm_array = images.add(create_fallback_array(
            [255, 128, 0, 255],
            TextureFormat::Rgba8Unorm,
        ));
        Self {
            layer_albedo: Default::default(),
            layer_normal: Default::default(),
            layer_arm: Default::default(),
            albedo_array,
            normal_array,
            arm_array,
            flags: 0,
            dirty: false,
        }
    }
}

/// Create a 1×1×8 array texture with a constant fallback pixel.
fn create_fallback_array(pixel: [u8; 4], format: TextureFormat) -> Image {
    let mut data = Vec::with_capacity(4 * 8);
    for _ in 0..8 {
        data.extend_from_slice(&pixel);
    }
    let size = Extent3d {
        width: 1,
        height: 1,
        depth_or_array_layers: 8,
    };
    let mut image = Image::new(
        size,
        bevy::render::render_resource::TextureDimension::D2,
        data,
        format,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    );
    image.sampler = ImageSampler::linear();
    image.texture_descriptor.usage =
        TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST;
    image
}

/// Build a texture array from per-layer source images, resizing to `LAYER_TEX_SIZE`.
fn build_texture_array(
    sources: &[Option<Handle<Image>>; 8],
    images: &Assets<Image>,
    default_pixel: [u8; 4],
    format: TextureFormat,
) -> Image {
    let size = LAYER_TEX_SIZE as usize;
    let bytes_per_slice = size * size * 4;
    let mut data = Vec::with_capacity(bytes_per_slice * 8);

    for slot in sources.iter() {
        if let Some(handle) = slot {
            if let Some(src) = images.get(handle) {
                if let Some(ref src_data) = src.data {
                    let src_w = src.texture_descriptor.size.width as usize;
                    let src_h = src.texture_descriptor.size.height as usize;
                    if src_w == size && src_h == size && src_data.len() == bytes_per_slice {
                        data.extend_from_slice(src_data);
                        continue;
                    }
                    // Nearest-neighbor resize
                    let resized = resize_nearest(src_data, src_w, src_h, size, size);
                    data.extend_from_slice(&resized);
                    continue;
                }
            }
        }
        // Fallback: fill with default pixel
        for _ in 0..(size * size) {
            data.extend_from_slice(&default_pixel);
        }
    }

    let extent = Extent3d {
        width: LAYER_TEX_SIZE,
        height: LAYER_TEX_SIZE,
        depth_or_array_layers: 8,
    };
    let mut image = Image::new(
        extent,
        bevy::render::render_resource::TextureDimension::D2,
        data,
        format,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    );
    image.sampler = ImageSampler::linear();
    image.texture_descriptor.usage =
        TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST;
    image
}

fn resize_nearest(src: &[u8], src_w: usize, src_h: usize, dst_w: usize, dst_h: usize) -> Vec<u8> {
    let mut out = vec![0u8; dst_w * dst_h * 4];
    for y in 0..dst_h {
        let sy = (y * src_h / dst_h).min(src_h - 1);
        for x in 0..dst_w {
            let sx = (x * src_w / dst_w).min(src_w - 1);
            let si = (sy * src_w + sx) * 4;
            let di = (y * dst_w + x) * 4;
            out[di..di + 4].copy_from_slice(&src[si..si + 4]);
        }
    }
    out
}

/// System that rebuilds texture arrays when layer textures change.
pub fn terrain_layer_texture_system(
    mut layer_tex: ResMut<TerrainLayerTextures>,
    mut images: ResMut<Assets<Image>>,
    mut splatmap_materials: ResMut<Assets<TerrainSplatmapMaterial>>,
    active_query: Query<&SplatmapActive>,
) {
    if !layer_tex.dirty {
        return;
    }

    // Check that all assigned textures are loaded
    for slot in layer_tex.layer_albedo.iter().chain(layer_tex.layer_normal.iter()).chain(layer_tex.layer_arm.iter()) {
        if let Some(handle) = slot {
            if images.get(handle).is_none() {
                return; // Not loaded yet, wait
            }
        }
    }

    // Build arrays
    let albedo = build_texture_array(
        &layer_tex.layer_albedo, &images,
        [255, 255, 255, 255], TextureFormat::Rgba8UnormSrgb,
    );
    let normal = build_texture_array(
        &layer_tex.layer_normal, &images,
        [128, 128, 255, 255], TextureFormat::Rgba8Unorm,
    );
    let arm = build_texture_array(
        &layer_tex.layer_arm, &images,
        [255, 128, 0, 255], TextureFormat::Rgba8Unorm,
    );

    // Compute flags
    let mut flags = 0u32;
    for i in 0..8u32 {
        if layer_tex.layer_albedo[i as usize].is_some() {
            flags |= 1 << i;
        }
        if layer_tex.layer_normal[i as usize].is_some() {
            flags |= 1 << (i + 8);
        }
        if layer_tex.layer_arm[i as usize].is_some() {
            flags |= 1 << (i + 16);
        }
    }
    layer_tex.flags = flags;

    // Replace array images
    if let Some(img) = images.get_mut(&layer_tex.albedo_array) {
        *img = albedo;
    } else {
        layer_tex.albedo_array = images.add(albedo);
    }
    if let Some(img) = images.get_mut(&layer_tex.normal_array) {
        *img = normal;
    } else {
        layer_tex.normal_array = images.add(normal);
    }
    if let Some(img) = images.get_mut(&layer_tex.arm_array) {
        *img = arm;
    } else {
        layer_tex.arm_array = images.add(arm);
    }

    // Update all active materials
    for active in active_query.iter() {
        if let Some(mat) = splatmap_materials.get_mut(&active.material_handle) {
            mat.layer_albedo_array = layer_tex.albedo_array.clone();
            mat.layer_normal_array = layer_tex.normal_array.clone();
            mat.layer_arm_array = layer_tex.arm_array.clone();
            mat.layer_tex_flags = LayerTexFlags {
                flags,
                _pad: UVec3::ZERO,
            };
        }
    }

    layer_tex.dirty = false;
}

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
    layer_tex: &TerrainLayerTextures,
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
        layer_albedo_array: layer_tex.albedo_array.clone(),
        layer_normal_array: layer_tex.normal_array.clone(),
        layer_arm_array: layer_tex.arm_array.clone(),
        layer_tex_flags: LayerTexFlags {
            flags: layer_tex.flags,
            _pad: UVec3::ZERO,
        },
    }
}

/// System that uploads dirty splatmaps to GPU and updates material properties.
pub fn splatmap_upload_system(
    mut images: ResMut<Assets<Image>>,
    mut splatmap_materials: ResMut<Assets<TerrainSplatmapMaterial>>,
    mut surface_query: Query<(&mut PaintableSurfaceData, Option<&SplatmapActive>)>,
    layer_tex: Res<TerrainLayerTextures>,
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
                    &layer_tex,
                );
                mat.layer_colors_a = updated.layer_colors_a;
                mat.layer_props_a = updated.layer_props_a;
                mat.layer_colors_b = updated.layer_colors_b;
                mat.layer_props_b = updated.layer_props_b;
                mat.layer_info = updated.layer_info;
                mat.layer_albedo_array = updated.layer_albedo_array;
                mat.layer_normal_array = updated.layer_normal_array;
                mat.layer_arm_array = updated.layer_arm_array;
                mat.layer_tex_flags = updated.layer_tex_flags;
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
    layer_tex: &TerrainLayerTextures,
) {
    let image_a = create_splatmap_image_a(&surface.splatmap_weights, surface.splatmap_resolution);
    let image_b = create_splatmap_image_b(&surface.splatmap_weights, surface.splatmap_resolution);
    let splatmap_a_handle = images.add(image_a);
    let splatmap_b_handle = images.add(image_b);
    let material = build_splatmap_material(surface, splatmap_a_handle.clone(), splatmap_b_handle.clone(), layer_tex);
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
