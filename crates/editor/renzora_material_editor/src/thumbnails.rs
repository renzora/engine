//! Texture thumbnail management for material graph nodes.
//!
//! Watches texture nodes in the current MaterialGraph, loads referenced images,
//! and registers them with egui so they can be displayed as node thumbnails.
//! Images with incompatible GPU formats are converted to Rgba8UnormSrgb.

use std::collections::HashMap;

use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy_egui::egui;
use bevy_egui::{EguiTextureHandle, EguiUserTextures};

use renzora_material::graph::PinValue;

use crate::MaterialEditorState;

/// Returns true if the image format is safe to register with egui.
fn is_egui_compatible(format: TextureFormat) -> bool {
    matches!(
        format,
        TextureFormat::Rgba8Unorm
            | TextureFormat::Rgba8UnormSrgb
            | TextureFormat::Bgra8Unorm
            | TextureFormat::Bgra8UnormSrgb
            | TextureFormat::Rgba16Float
            | TextureFormat::Rgba32Float
            | TextureFormat::R8Unorm
            | TextureFormat::Rg8Unorm
            | TextureFormat::R16Float
            | TextureFormat::Rg16Float
            | TextureFormat::Rg11b10Ufloat
    )
}

/// Convert an image with an incompatible format to Rgba8UnormSrgb for thumbnail use.
fn convert_to_rgba8(image: &Image) -> Option<Image> {
    let format = image.texture_descriptor.format;
    let data = image.data.as_ref()?;
    let w = image.texture_descriptor.size.width as usize;
    let h = image.texture_descriptor.size.height as usize;
    let pixel_count = w * h;

    let mut rgba = vec![0u8; pixel_count * 4];

    match format {
        TextureFormat::R16Uint | TextureFormat::R16Unorm => {
            if data.len() < pixel_count * 2 { return None; }
            for i in 0..pixel_count {
                let val = u16::from_le_bytes([data[i * 2], data[i * 2 + 1]]);
                let byte = (val >> 8) as u8;
                rgba[i * 4] = byte;
                rgba[i * 4 + 1] = byte;
                rgba[i * 4 + 2] = byte;
                rgba[i * 4 + 3] = 255;
            }
        }
        TextureFormat::R16Sint | TextureFormat::R16Snorm => {
            if data.len() < pixel_count * 2 { return None; }
            for i in 0..pixel_count {
                let val = i16::from_le_bytes([data[i * 2], data[i * 2 + 1]]);
                let byte = ((val as f32 / i16::MAX as f32).clamp(0.0, 1.0) * 255.0) as u8;
                rgba[i * 4] = byte;
                rgba[i * 4 + 1] = byte;
                rgba[i * 4 + 2] = byte;
                rgba[i * 4 + 3] = 255;
            }
        }
        TextureFormat::R32Float => {
            if data.len() < pixel_count * 4 { return None; }
            for i in 0..pixel_count {
                let val = f32::from_le_bytes([
                    data[i * 4], data[i * 4 + 1], data[i * 4 + 2], data[i * 4 + 3],
                ]);
                let byte = (val.clamp(0.0, 1.0) * 255.0) as u8;
                rgba[i * 4] = byte;
                rgba[i * 4 + 1] = byte;
                rgba[i * 4 + 2] = byte;
                rgba[i * 4 + 3] = 255;
            }
        }
        TextureFormat::R32Uint => {
            if data.len() < pixel_count * 4 { return None; }
            for i in 0..pixel_count {
                let val = u32::from_le_bytes([
                    data[i * 4], data[i * 4 + 1], data[i * 4 + 2], data[i * 4 + 3],
                ]);
                let byte = (val >> 24) as u8;
                rgba[i * 4] = byte;
                rgba[i * 4 + 1] = byte;
                rgba[i * 4 + 2] = byte;
                rgba[i * 4 + 3] = 255;
            }
        }
        TextureFormat::Rgba16Unorm => {
            if data.len() < pixel_count * 8 { return None; }
            for i in 0..pixel_count {
                let off = i * 8;
                rgba[i * 4] = (u16::from_le_bytes([data[off], data[off + 1]]) >> 8) as u8;
                rgba[i * 4 + 1] = (u16::from_le_bytes([data[off + 2], data[off + 3]]) >> 8) as u8;
                rgba[i * 4 + 2] = (u16::from_le_bytes([data[off + 4], data[off + 5]]) >> 8) as u8;
                rgba[i * 4 + 3] = (u16::from_le_bytes([data[off + 6], data[off + 7]]) >> 8) as u8;
            }
        }
        TextureFormat::Rg16Uint | TextureFormat::Rg16Unorm => {
            if data.len() < pixel_count * 4 { return None; }
            for i in 0..pixel_count {
                let off = i * 4;
                let r = (u16::from_le_bytes([data[off], data[off + 1]]) >> 8) as u8;
                let g = (u16::from_le_bytes([data[off + 2], data[off + 3]]) >> 8) as u8;
                rgba[i * 4] = r;
                rgba[i * 4 + 1] = g;
                rgba[i * 4 + 2] = 0;
                rgba[i * 4 + 3] = 255;
            }
        }
        _ => return None,
    }

    Some(Image::new(
        Extent3d { width: w as u32, height: h as u32, depth_or_array_layers: 1 },
        TextureDimension::D2,
        rgba,
        TextureFormat::Rgba8UnormSrgb,
        default(),
    ))
}

/// Try to register an image with egui, converting if needed.
fn register_thumbnail(
    image: &Image,
    original_handle: &Handle<Image>,
    images: &mut Assets<Image>,
    user_textures: &mut EguiUserTextures,
) -> Option<egui::TextureId> {
    if is_egui_compatible(image.texture_descriptor.format) {
        user_textures.add_image(EguiTextureHandle::Strong(original_handle.clone()));
        return user_textures.image_id(original_handle.id());
    }

    if let Some(converted) = convert_to_rgba8(image) {
        let converted_handle = images.add(converted);
        user_textures.add_image(EguiTextureHandle::Strong(converted_handle.clone()));
        return user_textures.image_id(converted_handle.id());
    }

    None
}

/// Resource mapping texture asset paths → egui TextureIds for node thumbnails.
#[derive(Resource, Default)]
pub struct NodeThumbnails {
    /// Path → (bevy image handle, egui texture id).
    pub entries: HashMap<String, ThumbnailEntry>,
}

pub struct ThumbnailEntry {
    pub image_handle: Handle<Image>,
    pub texture_id: Option<egui::TextureId>,
    /// True if we already attempted registration (don't retry every frame).
    pub resolved: bool,
}

impl NodeThumbnails {
    /// Build a TextureThumbnailMap for the graph editor sync.
    pub fn to_map(&self) -> HashMap<String, egui::TextureId> {
        self.entries
            .iter()
            .filter_map(|(path, entry)| {
                entry.texture_id.map(|id| (path.clone(), id))
            })
            .collect()
    }
}

/// System that scans material graph nodes for texture paths,
/// loads them if needed, and registers with egui.
fn update_node_thumbnails(
    editor_state: Res<MaterialEditorState>,
    asset_server: Res<AssetServer>,
    mut thumbnails: ResMut<NodeThumbnails>,
    mut user_textures: ResMut<EguiUserTextures>,
    mut images: ResMut<Assets<Image>>,
) {
    // Collect all texture paths from the current graph
    let mut needed_paths: Vec<String> = Vec::new();
    for node in &editor_state.graph.nodes {
        if let Some(PinValue::TexturePath(ref path)) = node.input_values.get("texture") {
            if !path.is_empty() {
                needed_paths.push(path.clone());
            }
        }
    }

    // Remove entries no longer needed
    thumbnails.entries.retain(|path, _| needed_paths.contains(path));

    // Load new textures and register with egui
    for path in &needed_paths {
        if thumbnails.entries.contains_key(path) {
            let entry = thumbnails.entries.get_mut(path).unwrap();
            if entry.texture_id.is_none() && !entry.resolved {
                if let Some(image) = images.get(&entry.image_handle) {
                    let image_clone = image.clone();
                    entry.resolved = true;
                    entry.texture_id = register_thumbnail(
                        &image_clone,
                        &entry.image_handle.clone(),
                        &mut images,
                        &mut user_textures,
                    );
                }
            }
            continue;
        }

        // New texture — start loading
        let owned_path: String = path.clone();
        let image_handle: Handle<Image> = asset_server.load(owned_path);
        let mut texture_id = None;
        let mut resolved = false;
        if let Some(image) = images.get(&image_handle) {
            let image_clone = image.clone();
            resolved = true;
            texture_id = register_thumbnail(
                &image_clone,
                &image_handle,
                &mut images,
                &mut user_textures,
            );
        }

        thumbnails.entries.insert(
            path.clone(),
            ThumbnailEntry {
                image_handle,
                texture_id,
                resolved,
            },
        );
    }
}

pub struct NodeThumbnailPlugin;

impl Plugin for NodeThumbnailPlugin {
    fn build(&self, app: &mut App) {
        info!("[editor] NodeThumbnailPlugin");
        app.init_resource::<NodeThumbnails>()
            .add_systems(Update, update_node_thumbnails);
    }
}
