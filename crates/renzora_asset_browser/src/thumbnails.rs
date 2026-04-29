//! Asset thumbnail cache — loads image files as Bevy images and registers them
//! with egui so the asset browser grid can display visual previews.
//!
//! Images with incompatible GPU formats (R16Uint, R32Float, etc.) are
//! automatically converted to Rgba8UnormSrgb for thumbnail display.
//!
//! **Persistent cache** — once a source loads, a downscaled 256×256 PNG is
//! saved to `<project>/.cache/thumbnails/textures/<asset-rel>.png`. Future
//! sessions hit that cache directly instead of re-decoding the (often
//! multi-megabyte) source. Invalidation is automatic — the cache is only
//! considered fresh while its mtime ≥ the source's mtime.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use bevy::asset::LoadState;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy_egui::egui;
use bevy_egui::{EguiTextureHandle, EguiUserTextures};
use renzora::core::CurrentProject;
use renzora_editor::thumbnail_cache_dir;

/// Width/height the cached thumbnail PNG is resized to. Asset browser
/// renders at ~96px so 256 keeps headroom for HiDPI without bloating
/// the cache. Source aspect ratio is preserved within this bound.
const CACHE_THUMB_SIZE: u32 = 256;

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

/// Path on disk where the persistent thumbnail PNG for a texture lives.
/// Mirrors `material_thumb_path` so both kinds share the same root.
///
/// Example: `<project>/assets/textures/wall.png` →
/// `<project>/.cache/thumbnails/textures/wall.png`. Sources outside the
/// project's `assets/` directory aren't cached (return `None`).
pub fn texture_thumb_path(
    source_abs: &Path,
    project: &CurrentProject,
) -> Option<PathBuf> {
    let rel = project.make_relative(source_abs)?;
    let rel = rel.strip_prefix("assets/").unwrap_or(&rel);
    let mut out = thumbnail_cache_dir(project, "textures").join(rel);
    out.set_extension("png");
    Some(out)
}

/// True iff the cached thumbnail file at `cache_path` is fresh — exists
/// and its mtime is newer than (or equal to) the source's mtime. A
/// cached PNG with mtime older than the source is treated as stale and
/// regenerated on next request.
fn cached_thumb_is_fresh(cache_path: &Path, source_path: &Path) -> bool {
    let Ok(cache_meta) = std::fs::metadata(cache_path) else {
        return false;
    };
    let Ok(source_meta) = std::fs::metadata(source_path) else {
        // Source vanished — keep the cache. Asset browser will hide
        // the row anyway.
        return true;
    };
    let (Ok(cache_mtime), Ok(source_mtime)) =
        (cache_meta.modified(), source_meta.modified())
    else {
        return false;
    };
    cache_mtime >= source_mtime
}

/// Decode a Bevy `Image` to RGBA8 bytes suitable for the `image` crate.
/// Returns the (width, height, rgba) triple, or `None` if the format
/// isn't one we can encode. Mirrors the format coverage of
/// [`convert_to_rgba8`] so both paths agree on what's supported.
fn rgba8_bytes_for_encoding(image: &Image) -> Option<(u32, u32, Vec<u8>)> {
    let format = image.texture_descriptor.format;
    let w = image.texture_descriptor.size.width;
    let h = image.texture_descriptor.size.height;

    if matches!(
        format,
        TextureFormat::Rgba8Unorm | TextureFormat::Rgba8UnormSrgb
    ) {
        let data = image.data.as_ref()?;
        return Some((w, h, data.clone()));
    }
    if matches!(
        format,
        TextureFormat::Bgra8Unorm | TextureFormat::Bgra8UnormSrgb
    ) {
        let data = image.data.as_ref()?;
        let mut rgba = data.clone();
        for px in rgba.chunks_exact_mut(4) {
            px.swap(0, 2);
        }
        return Some((w, h, rgba));
    }
    // Anything else: route through the existing converter so HDR/single
    // channel sources get sensible greyscale thumbnails.
    let converted = convert_to_rgba8(image)?;
    let data = converted.data?;
    Some((w, h, data))
}

/// Save a downscaled 256×256 (max) PNG of `image` to `cache_path`.
/// Best-effort — failures are logged at debug level and don't propagate;
/// the in-memory thumbnail still works for this session and the next
/// session will retry.
fn save_thumbnail_to_disk(image: &Image, cache_path: &Path) {
    let Some((w, h, rgba)) = rgba8_bytes_for_encoding(image) else {
        debug!(
            "[thumbnails] format {:?} unsupported for caching {}",
            image.texture_descriptor.format,
            cache_path.display()
        );
        return;
    };
    let Some(buf) = image::ImageBuffer::<image::Rgba<u8>, _>::from_raw(w, h, rgba)
    else {
        return;
    };
    // Lanczos3 to match `renzora_rmip`'s mipmap baker — high-quality
    // downscale is worth the extra ms when the result is cached forever.
    let (target_w, target_h) = if w >= h {
        let aspect = h as f32 / w as f32;
        let tw = CACHE_THUMB_SIZE.min(w);
        (tw, ((tw as f32 * aspect).round() as u32).max(1))
    } else {
        let aspect = w as f32 / h as f32;
        let th = CACHE_THUMB_SIZE.min(h);
        (((th as f32 * aspect).round() as u32).max(1), th)
    };
    let resized = image::imageops::resize(&buf, target_w, target_h, image::imageops::FilterType::Lanczos3);

    if let Some(parent) = cache_path.parent() {
        if let Err(e) = std::fs::create_dir_all(parent) {
            debug!("[thumbnails] couldn't create {}: {}", parent.display(), e);
            return;
        }
    }
    if let Err(e) = resized.save(cache_path) {
        debug!(
            "[thumbnails] couldn't save thumbnail {}: {}",
            cache_path.display(),
            e
        );
    }
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
    /// Source paths whose handle currently points at the *source* file,
    /// not the persistent cache PNG. After their image lands in
    /// `Assets<Image>` we'll downscale + write a cache PNG, so future
    /// sessions can hit the cache. Source paths whose request resolved
    /// from the disk cache are absent here — there's nothing more to
    /// save.
    pending_disk_save: HashSet<PathBuf>,
}

impl ThumbnailCache {
    /// Get the egui texture ID for a loaded thumbnail, if ready.
    pub fn get_texture_id(&self, path: &PathBuf) -> Option<egui::TextureId> {
        self.texture_ids.get(path).copied()
    }

    /// Request a thumbnail load. Converts the absolute `path` to an
    /// asset-relative path via `CurrentProject` before handing it to the
    /// asset server. Returns `true` if the request was enqueued.
    ///
    /// Tries the persistent thumbnail cache (`<project>/.cache/thumbnails/
    /// textures/<rel>.png`) first — on a fresh hit, the asset_server
    /// loads the small cached PNG instead of decoding the (potentially
    /// multi-megabyte) source. On a miss or stale cache, the source
    /// loads as before, and `update_thumbnail_cache` writes a fresh
    /// downscaled PNG so the next session can hit the cache.
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
        // Persistent cache hit: load the cached PNG instead of the
        // source. The egui texture this session ends up using is the
        // 256-px PNG, so memory + decode time both shrink. Without a
        // project we can't compute the cache path, so fall through.
        if let Some(p) = project {
            if let Some(cache_path) = texture_thumb_path(&path, p) {
                if cached_thumb_is_fresh(&cache_path, &path) {
                    let cache_rel = p.make_asset_relative(&cache_path);
                    if !Path::new(&cache_rel).is_absolute() {
                        let handle: Handle<Image> = asset_server.load(cache_rel);
                        self.loading.insert(path.clone());
                        self.handles.insert(path, handle);
                        // No `pending_disk_save` insert — the cache is
                        // already on disk for this asset.
                        return true;
                    }
                }
            }
        }

        // Cache miss / no project — load the source. Convert absolute
        // path → asset-relative (e.g. "ui/Action_panel.png"). If the
        // file isn't under the project's assets/ directory,
        // make_asset_relative falls back to the full absolute path
        // which the asset server will reject. Skip those.
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
        // Mark this source path so `update_thumbnail_cache` knows to
        // downscale + save once the image lands.
        if project.is_some() {
            self.pending_disk_save.insert(path.clone());
        }
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
    project: Option<Res<CurrentProject>>,
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
                    // Persist a downscaled PNG once we know the bytes are
                    // stable. `register_thumbnail` may swap the handle
                    // (format conversion path), so do this *before* it
                    // runs — `image_clone` already has the source bytes.
                    if cache.pending_disk_save.remove(&path) {
                        if let Some(p) = project.as_deref() {
                            if let Some(cache_path) = texture_thumb_path(&path, p) {
                                save_thumbnail_to_disk(&image_clone, &cache_path);
                            }
                        }
                    }
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
                cache.pending_disk_save.remove(&path);
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
                if cache.pending_disk_save.remove(&path) {
                    if let Some(p) = project.as_deref() {
                        if let Some(cache_path) = texture_thumb_path(&path, p) {
                            save_thumbnail_to_disk(&image_clone, &cache_path);
                        }
                    }
                }
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
