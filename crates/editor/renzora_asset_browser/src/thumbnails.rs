//! Asset thumbnail cache — loads image files as Bevy images and registers them
//! with egui so the asset browser grid can display visual previews.
//!
//! Supported formats: PNG, JPG, JPEG, BMP, TGA, WebP.
//! HDR/EXR are excluded (often use texture formats incompatible with egui).

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use bevy::asset::LoadState;
use bevy::prelude::*;
use bevy_egui::egui;
use bevy_egui::{EguiTextureHandle, EguiUserTextures};
use renzora_core::CurrentProject;

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
pub fn supports_thumbnail(filename: &str) -> bool {
    let ext = filename.rsplit('.').next().unwrap_or("").to_lowercase();
    matches!(
        ext.as_str(),
        "png" | "jpg" | "jpeg" | "bmp" | "tga" | "webp"
    )
}

/// System that checks loading state and registers completed thumbnails with egui.
pub fn update_thumbnail_cache(
    asset_server: Res<AssetServer>,
    mut cache: ResMut<ThumbnailCache>,
    mut user_textures: ResMut<EguiUserTextures>,
    images: Res<Assets<Image>>,
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
                // Register with egui
                if images.contains(&handle) {
                    user_textures
                        .add_image(EguiTextureHandle::Strong(handle.clone()));
                    if let Some(id) = user_textures.image_id(handle.id()) {
                        cache.texture_ids.insert(path, id);
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
        if let Some(handle) = cache.handles.get(&path) {
            if images.contains(handle) {
                user_textures
                    .add_image(EguiTextureHandle::Strong(handle.clone()));
                if let Some(id) = user_textures.image_id(handle.id()) {
                    cache.texture_ids.insert(path, id);
                }
            }
        }
    }
}
