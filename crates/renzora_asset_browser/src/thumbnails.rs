//! Asset thumbnail cache — loads image files as Bevy images and publishes their
//! `Handle<Image>` so the bevy-native asset browser grid can display visual
//! previews via `ImageNode`.
//!
//! Images with incompatible GPU formats (R16Uint, R32Float, etc.) are
//! automatically converted to Rgba8UnormSrgb when encoding the cached PNG.
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
use renzora::core::CurrentProject;
use renzora_editor_framework::thumbnail_cache_dir;

/// Width/height the cached thumbnail PNG is resized to. Asset browser
/// renders at ~96px so 256 keeps headroom for HiDPI without bloating
/// the cache. Source aspect ratio is preserved within this bound.
const CACHE_THUMB_SIZE: u32 = 256;

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
            if data.len() < pixel_count * 2 {
                return None;
            }
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
            if data.len() < pixel_count * 2 {
                return None;
            }
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
            if data.len() < pixel_count * 4 {
                return None;
            }
            for i in 0..pixel_count {
                let val = f32::from_le_bytes([
                    data[i * 4],
                    data[i * 4 + 1],
                    data[i * 4 + 2],
                    data[i * 4 + 3],
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
            if data.len() < pixel_count * 4 {
                return None;
            }
            for i in 0..pixel_count {
                let val = u32::from_le_bytes([
                    data[i * 4],
                    data[i * 4 + 1],
                    data[i * 4 + 2],
                    data[i * 4 + 3],
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
            if data.len() < pixel_count * 8 {
                return None;
            }
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
            if data.len() < pixel_count * 4 {
                return None;
            }
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
        Extent3d {
            width: w as u32,
            height: h as u32,
            depth_or_array_layers: 1,
        },
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
pub fn texture_thumb_path(source_abs: &Path, project: &CurrentProject) -> Option<PathBuf> {
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
    let (Ok(cache_mtime), Ok(source_mtime)) = (cache_meta.modified(), source_meta.modified())
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
    let Some(buf) = image::ImageBuffer::<image::Rgba<u8>, _>::from_raw(w, h, rgba) else {
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
    let resized = image::imageops::resize(
        &buf,
        target_w,
        target_h,
        image::imageops::FilterType::Lanczos3,
    );

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
    /// Get the `Handle<Image>` for a loaded thumbnail, if ready — for the
    /// bevy-native browser, which displays it via `ImageNode` (no egui texture).
    pub fn handle(&self, path: &PathBuf) -> Option<Handle<Image>> {
        self.handles.get(path).cloned()
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
        if self.handles.contains_key(&path)
            || self.loading.contains(&path)
            || self.failed.contains(&path)
        {
            return false;
        }
        if self.handles.len() + self.loading.len() >= MAX_LOADED {
            return false;
        }
        // Persistent cache hit: load the cached PNG instead of the
        // source. The handle this session ends up publishing points at
        // the 256-px PNG, so memory + decode time both shrink. Without a
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
}

// ============================================================================
// Folder previews
// ============================================================================

/// How many images a folder tile's mosaic shows. Four fills a 2×2 grid; more
/// than that is unreadable at a 96 px tile.
pub const FOLDER_PREVIEW_MAX: usize = 4;

/// Ceiling on directory entries examined per folder scan. A texture library can
/// hold thousands of files, and the mosaic only needs the first handful — this
/// stops a scan from walking the whole tree to find images it won't show.
const FOLDER_SCAN_BUDGET: usize = 512;

/// How far below a folder the scan looks. Assets are usually one or two levels
/// down (`textures/wood/`, `characters/hero/`), so a folder holding only
/// subfolders still gets a mosaic instead of a bare glyph.
const FOLDER_SCAN_DEPTH: usize = 2;

/// Folders scanned per frame. Navigating into a directory of 50 subfolders
/// would otherwise do 50 recursive walks in the frame the tiles appear.
const FOLDER_SCANS_PER_FRAME: usize = 4;

/// How long a folder's scan is trusted before a visible tile rescans it. The
/// safety net for images added by another tool, mirroring the listing's own
/// slow rescan — but far lazier, because a scan walks subdirectories.
const FOLDER_PREVIEW_TTL: f32 = 5.0;

/// The images each folder shows in its tile, so the browser previews a folder's
/// contents instead of drawing the same glyph on all of them.
///
/// Cached rather than read per frame: the mosaic is built from a recursive walk,
/// which is far too expensive to repeat for every visible folder every frame.
#[derive(Resource, Default)]
pub struct FolderPreviews {
    /// Folder → (up to [`FOLDER_PREVIEW_MAX`] image paths, when it was scanned).
    /// An empty vec means "scanned, no images" — the miss is cached too.
    entries: HashMap<PathBuf, (std::sync::Arc<Vec<PathBuf>>, f32)>,
    /// Bumped only when a scan actually changes a folder's images. The grid's
    /// dirty token folds this in, which is what makes tiles rebuild once their
    /// scan lands — without it the mosaic would sit invisible until something
    /// unrelated (a click, a resize) happened to re-snapshot the grid.
    version: u64,
}

impl FolderPreviews {
    /// The images to draw in `folder`'s tile. `None` until the scan lands.
    pub fn images(&self, folder: &PathBuf) -> Option<std::sync::Arc<Vec<PathBuf>>> {
        self.entries.get(folder).map(|(paths, _)| paths.clone())
    }

    /// Changes whenever any folder's images change. See [`FolderPreviews::version`]'s field docs.
    pub fn version(&self) -> u64 {
        self.version
    }
}

/// Walks the folders that currently have a tile on screen and records which
/// images each should show. Budgeted per frame, and each folder is re-walked at
/// most once per [`FOLDER_PREVIEW_TTL`].
pub(crate) fn scan_folder_previews(
    time: Res<Time>,
    tiles: Query<&crate::state::AssetTile>,
    mut previews: ResMut<FolderPreviews>,
) {
    let now = time.elapsed_secs();
    let mut budget = FOLDER_SCANS_PER_FRAME;
    for tile in &tiles {
        if budget == 0 {
            break;
        }
        if !tile.is_dir {
            continue;
        }
        let fresh = previews
            .entries
            .get(&tile.path)
            .is_some_and(|(_, at)| now - *at < FOLDER_PREVIEW_TTL);
        if fresh {
            continue;
        }
        let found = collect_folder_preview(&tile.path);
        budget -= 1;
        // An unchanged rescan only restamps the clock. Replacing the entry would
        // bump `version` and re-hash the whole grid every few seconds for a
        // picture that didn't change.
        if previews
            .entries
            .get(&tile.path)
            .is_some_and(|(paths, _)| **paths == found)
        {
            if let Some(slot) = previews.entries.get_mut(&tile.path) {
                slot.1 = now;
            }
            continue;
        }
        previews
            .entries
            .insert(tile.path.clone(), (std::sync::Arc::new(found), now));
        previews.version = previews.version.wrapping_add(1);
    }
}

/// The first [`FOLDER_PREVIEW_MAX`] images at or below `root`, breadth-first so
/// a folder's own images win over a subfolder's.
///
/// Each directory's entries are sorted by name before being taken. Filesystem
/// order isn't guaranteed stable, and an unstable result would rewrite the
/// tile's rebuild key on every rescan — the mosaic would visibly reshuffle
/// every few seconds.
#[cfg(not(target_arch = "wasm32"))]
fn collect_folder_preview(root: &Path) -> Vec<PathBuf> {
    let mut found: Vec<PathBuf> = Vec::new();
    let mut queue: std::collections::VecDeque<(PathBuf, usize)> = std::collections::VecDeque::new();
    queue.push_back((root.to_path_buf(), 0));
    let mut budget = FOLDER_SCAN_BUDGET;

    while let Some((dir, depth)) = queue.pop_front() {
        let Ok(rd) = std::fs::read_dir(&dir) else {
            continue;
        };
        let mut images: Vec<PathBuf> = Vec::new();
        let mut subdirs: Vec<PathBuf> = Vec::new();
        for e in rd.flatten() {
            if budget == 0 {
                break;
            }
            budget -= 1;
            let name = e.file_name().to_string_lossy().to_string();
            if name.starts_with('.') {
                continue;
            }
            // An unreadable `file_type` is skipped, same as an unsupported one.
            let Ok(ft) = e.file_type() else { continue };
            // The depth guard is deliberately *inside* the directory branch
            // rather than folded into it: a directory is never a thumbnail
            // whether or not we descend, and a combined condition would let a
            // folder named `textures.png` past the depth limit fall through to
            // the image branch below.
            if ft.is_dir() {
                if depth < FOLDER_SCAN_DEPTH {
                    subdirs.push(e.path());
                }
            } else if supports_thumbnail(&name) {
                images.push(e.path());
            }
        }
        images.sort();
        subdirs.sort();
        for image in images {
            found.push(image);
            if found.len() >= FOLDER_PREVIEW_MAX {
                return found;
            }
        }
        if budget == 0 {
            break;
        }
        queue.extend(subdirs.into_iter().map(|d| (d, depth + 1)));
    }
    found
}

/// Web: the browser's directory handle answers from a cache, and a miss returns
/// nothing rather than blocking. A folder whose subdirectories haven't been
/// read yet simply gets its images on a later rescan.
#[cfg(target_arch = "wasm32")]
fn collect_folder_preview(root: &Path) -> Vec<PathBuf> {
    let mut found: Vec<PathBuf> = Vec::new();
    let mut queue: std::collections::VecDeque<(PathBuf, usize)> = std::collections::VecDeque::new();
    queue.push_back((root.to_path_buf(), 0));

    while let Some((dir, depth)) = queue.pop_front() {
        let Some(list) = renzora_webfs::list_dir(&dir) else {
            continue;
        };
        let mut images: Vec<PathBuf> = Vec::new();
        let mut subdirs: Vec<PathBuf> = Vec::new();
        for e in list {
            if e.name.starts_with('.') {
                continue;
            }
            if e.is_dir {
                if depth < FOLDER_SCAN_DEPTH {
                    subdirs.push(dir.join(&e.name));
                }
            } else if supports_thumbnail(&e.name) {
                images.push(dir.join(&e.name));
            }
        }
        images.sort();
        subdirs.sort();
        for image in images {
            found.push(image);
            if found.len() >= FOLDER_PREVIEW_MAX {
                return found;
            }
        }
        queue.extend(subdirs.into_iter().map(|d| (d, depth + 1)));
    }
    found
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

/// Returns `true` if the file may have a scene thumbnail — a viewport snapshot
/// a previous save left in the thumbnail cache. Unlike the other kinds this is
/// only a *maybe*: nothing renders one on demand, so a scene that has never
/// been saved from this editor simply keeps its type glyph.
pub fn supports_scene_thumbnail(filename: &str) -> bool {
    let ext = filename.rsplit('.').next().unwrap_or("").to_lowercase();
    matches!(ext.as_str(), "bsn" | "ron")
}

/// Returns `true` if the file has a rendered thumbnail available through
/// the model thumbnail registry. GLBs and GLTFs are the supported model
/// formats — others (FBX, OBJ, USD) fall through to the generic icon
/// because Bevy doesn't load them.
pub fn supports_model_thumbnail(filename: &str) -> bool {
    let ext = filename.rsplit('.').next().unwrap_or("").to_lowercase();
    matches!(ext.as_str(), "glb" | "gltf")
}

/// System that checks loading state and, once a thumbnail's image lands,
/// persists a downscaled PNG to the on-disk cache. The `Handle<Image>` stays
/// published in `handles` for the native browser to display via `ImageNode`.
pub fn update_thumbnail_cache(
    asset_server: Res<AssetServer>,
    mut cache: ResMut<ThumbnailCache>,
    images: Res<Assets<Image>>,
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
                    // Persist a downscaled PNG once we know the bytes are
                    // stable so future sessions can hit the cache.
                    if cache.pending_disk_save.remove(&path) {
                        if let Some(p) = project.as_deref() {
                            if let Some(cache_path) = texture_thumb_path(&path, p) {
                                save_thumbnail_to_disk(image, &cache_path);
                            }
                        }
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

    // Catch handles that loaded before we got to check (race) and still owe a
    // disk save.
    let pending: Vec<PathBuf> = cache
        .pending_disk_save
        .iter()
        .filter(|p| !cache.loading.contains(*p) && !cache.failed.contains(*p))
        .cloned()
        .collect();

    for path in pending {
        if let Some(handle) = cache.handles.get(&path).cloned() {
            if let Some(image) = images.get(&handle) {
                cache.pending_disk_save.remove(&path);
                if let Some(p) = project.as_deref() {
                    if let Some(cache_path) = texture_thumb_path(&path, p) {
                        save_thumbnail_to_disk(image, &cache_path);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod folder_preview_tests {
    use super::*;

    /// The scan finds a folder's own images, and finds images in a subfolder
    /// when the folder itself holds none.
    #[test]
    fn scan_walks_into_subfolders() {
        let root = std::env::temp_dir().join("renzora_folder_preview_test");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("nested")).unwrap();
        for n in ["a.png", "b.jpg"] {
            std::fs::write(root.join("nested").join(n), b"x").unwrap();
        }
        std::fs::write(root.join("readme.txt"), b"x").unwrap();

        let found = collect_folder_preview(&root);
        assert_eq!(found.len(), 2, "expected the nested images, got {found:?}");

        std::fs::write(root.join("top.png"), b"x").unwrap();
        let found = collect_folder_preview(&root);
        assert_eq!(found.first().unwrap().file_name().unwrap(), "top.png");

        let _ = std::fs::remove_dir_all(&root);
    }

    /// The system, run in an app, populates the registry for a folder tile.
    #[test]
    fn system_populates_registry() {
        let root = std::env::temp_dir().join("renzora_folder_preview_sys_test");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("a.png"), b"x").unwrap();

        let mut app = App::new();
        app.init_resource::<Time>()
            .init_resource::<FolderPreviews>()
            .add_systems(Update, scan_folder_previews);
        app.world_mut().spawn(crate::state::AssetTile {
            path: root.clone(),
            is_dir: true,
        });
        app.update();

        let previews = app.world().resource::<FolderPreviews>();
        let images = previews.images(&root).expect("folder was never scanned");
        assert_eq!(images.len(), 1, "got {images:?}");

        let _ = std::fs::remove_dir_all(&root);
    }
}
