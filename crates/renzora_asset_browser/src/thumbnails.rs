//! Asset thumbnail cache — loads image files as Bevy images and registers them
//! with egui so the asset browser grid can display visual previews.
//!
//! Images with incompatible GPU formats (R16Uint, R32Float, etc.) are
//! automatically converted to Rgba8UnormSrgb for thumbnail display.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use bevy::asset::LoadState;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy_egui::egui;
use bevy_egui::{EguiTextureHandle, EguiUserTextures};
use renzora::core::CurrentProject;

/// Returns true if the image format is safe to register with egui
/// (filterable float sample type).
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
/// Returns None if the format is unrecognized or the data is malformed.
fn convert_to_rgba8(image: &Image) -> Option<Image> {
    let format = image.texture_descriptor.format;
    let data = image.data.as_ref()?;
    let w = image.texture_descriptor.size.width as usize;
    let h = image.texture_descriptor.size.height as usize;
    let pixel_count = w * h;

    let mut rgba = vec![0u8; pixel_count * 4];

    match format {
        // 16-bit single channel (unsigned int) — common for displacement/height maps
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
        // 16-bit single channel (signed int)
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
        // 32-bit single channel float — common for HDR height/displacement
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
        // 32-bit uint single channel
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
        // 16-bit RGBA float — HDR textures
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
        // 16-bit RG (two channel)
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

/// Maximum number of thumbnails loaded at once (prevents loading entire projects).
const MAX_LOADED: usize = 256;

/// Resource that caches image thumbnails for the asset browser.
#[derive(Resource, Default)]
pub struct ThumbnailCache {
    /// Path → loaded Bevy image handle.
    handles: HashMap<PathBuf, Handle<Image>>,
    /// Path → registered egui texture ID (ready to display).
    texture_ids: HashMap<PathBuf, egui::TextureId>,
    /// Paths currently in-flight (waiting for asset server to load).
    loading: HashSet<PathBuf>,
    /// Paths that failed to load.
    failed: HashSet<PathBuf>,
}

impl ThumbnailCache {
    /// Get the egui texture ID for a loaded thumbnail, if ready.
    pub fn get_texture_id(&self, path: &PathBuf) -> Option<egui::TextureId> {
        self.texture_ids.get(path).copied()
    }

    /// Request a thumbnail load. Converts the absolute `path` to an
    /// asset-relative path via `CurrentProject` before handing it to the
    /// asset server. Returns `true` if the request was enqueued.
    pub fn request(
        &mut self,
        path: PathBuf,
        asset_server: &AssetServer,
        project: Option<&CurrentProject>,
    ) -> bool {
        if self.texture_ids.contains_key(&path)
            || self.handles.contains_key(&path)
            || self.loading.contains(&path)
            || self.failed.contains(&path)
        {
            return false;
        }
        if self.handles.len() + self.loading.len() >= MAX_LOADED {
            return false;
        }
        // Convert absolute path → asset-relative (e.g. "ui/Action_panel.png").
        // If the file isn't under the project's assets/ directory,
        // make_asset_relative falls back to the full absolute path which the
        // asset server will reject. Skip those files.
        let load_path = match project {
            Some(p) => {
                let rel = p.make_asset_relative(&path);
                if Path::new(&rel).is_absolute() {
                    self.failed.insert(path);
                    return false;
                }
                rel
            }
            None => path.to_string_lossy().replace('\\', "/"),
        };
        let handle: Handle<Image> = asset_server.load(load_path);
        self.loading.insert(path.clone());
        self.handles.insert(path, handle);
        true
    }

    /// Check if a path is currently being loaded.
    pub fn is_loading(&self, path: &PathBuf) -> bool {
        self.loading.contains(path)
    }

    /// Return a snapshot of all ready texture IDs (for passing to the grid).
    pub fn texture_id_map(&self) -> HashMap<PathBuf, egui::TextureId> {
        self.texture_ids.clone()
    }
}

/// Returns `true` if the file extension is a supported image thumbnail format.
/// EXR is excluded — Bevy's EXR loader doesn't support all compression methods
/// or single-channel layouts, which causes errors on common PBR texture sets.
pub fn supports_thumbnail(filename: &str) -> bool {
    let ext = filename.rsplit('.').next().unwrap_or("").to_lowercase();
    matches!(
        ext.as_str(),
        "png" | "jpg" | "jpeg" | "bmp" | "tga" | "webp" | "hdr"
    )
}

/// Returns `true` if the file has a rendered thumbnail available through the
/// material thumbnail registry rather than the image thumbnail cache.
pub fn supports_material_thumbnail(filename: &str) -> bool {
    let ext = filename.rsplit('.').next().unwrap_or("").to_lowercase();
    ext == "material"
}

/// Try to register the image with egui. If the format is incompatible, convert
/// it to Rgba8UnormSrgb first. Returns the egui TextureId on success.
fn register_thumbnail(
    image: &Image,
    original_handle: &Handle<Image>,
    images: &mut Assets<Image>,
    user_textures: &mut EguiUserTextures,
) -> Option<egui::TextureId> {
    if is_egui_compatible(image.texture_descriptor.format) {
        // Format is fine — register directly
        user_textures.add_image(EguiTextureHandle::Strong(original_handle.clone()));
        return user_textures.image_id(original_handle.id());
    }

    // Convert to RGBA8 for thumbnail display
    if let Some(converted) = convert_to_rgba8(image) {
        let converted_handle = images.add(converted);
        user_textures.add_image(EguiTextureHandle::Strong(converted_handle.clone()));
        return user_textures.image_id(converted_handle.id());
    }

    warn!(
        "Cannot convert thumbnail format {:?} to RGBA8",
        image.texture_descriptor.format
    );
    None
}

/// System that checks loading state and registers completed thumbnails with egui.
pub fn update_thumbnail_cache(
    asset_server: Res<AssetServer>,
    mut cache: ResMut<ThumbnailCache>,
    mut user_textures: ResMut<EguiUserTextures>,
    mut images: ResMut<Assets<Image>>,
) {
    // Collect paths that are still in the loading set and check their state.
    let loading: Vec<PathBuf> = cache.loading.iter().cloned().collect();

    for path in loading {
        let Some(handle) = cache.handles.get(&path).cloned() else {
            cache.loading.remove(&path);
            continue;
        };

        match asset_server.get_load_state(&handle) {
            Some(LoadState::Loaded) => {
                cache.loading.remove(&path);
                if let Some(image) = images.get(&handle) {
                    // Clone data we need before borrowing images mutably
                    let image_clone = image.clone();
                    if let Some(id) = register_thumbnail(&image_clone, &handle, &mut images, &mut user_textures) {
                        cache.texture_ids.insert(path, id);
                    } else {
                        cache.failed.insert(path.clone());
                        cache.handles.remove(&path);
                    }
                }
            }
            Some(LoadState::Failed(_)) => {
                cache.loading.remove(&path);
                cache.failed.insert(path.clone());
                cache.handles.remove(&path);
            }
            _ => {} // Still loading
        }
    }

    // Also register any handles that loaded before we got to check (race).
    let unregistered: Vec<PathBuf> = cache
        .handles
        .iter()
        .filter(|(p, _)| !cache.texture_ids.contains_key(*p) && !cache.loading.contains(*p) && !cache.failed.contains(*p))
        .map(|(p, _)| p.clone())
        .collect();

    for path in unregistered {
        if let Some(handle) = cache.handles.get(&path).cloned() {
            if let Some(image) = images.get(&handle) {
                let image_clone = image.clone();
                if let Some(id) = register_thumbnail(&image_clone, &handle, &mut images, &mut user_textures) {
                    cache.texture_ids.insert(path, id);
                } else {
                    cache.failed.insert(path.clone());
                    cache.handles.remove(&path);
                }
            }
        }
    }
}
