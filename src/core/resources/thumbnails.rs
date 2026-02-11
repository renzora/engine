#![allow(dead_code)]

use bevy::prelude::*;
use bevy_egui::egui::{TextureHandle, TextureId};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

/// Maximum dimension for thumbnail images (will be scaled down if larger)
pub const THUMBNAIL_MAX_SIZE: u32 = 128;

/// Resource that caches image preview textures for the image preview panel
#[derive(Resource, Default)]
pub struct ImagePreviewTextures {
    /// Map of file paths to their egui texture handles
    pub textures: HashMap<PathBuf, TextureHandle>,
}

/// Resource that caches asset thumbnails for the asset browser
#[derive(Resource, Default)]
pub struct ThumbnailCache {
    /// Map of file paths to their loaded image handles
    pub image_handles: HashMap<PathBuf, Handle<Image>>,
    /// Map of file paths to their egui texture IDs (once registered)
    pub texture_ids: HashMap<PathBuf, TextureId>,
    /// Set of paths currently being loaded (to avoid duplicate loads)
    pub loading: HashSet<PathBuf>,
    /// Paths that failed to load (to avoid retrying)
    pub failed: HashSet<PathBuf>,
    /// Current folder being viewed (to invalidate cache on folder change)
    pub current_folder: Option<PathBuf>,
}

impl ThumbnailCache {
    /// Check if a thumbnail is ready to display
    pub fn get_texture_id(&self, path: &PathBuf) -> Option<TextureId> {
        self.texture_ids.get(path).copied()
    }

    /// Check if a path is currently loading
    pub fn is_loading(&self, path: &PathBuf) -> bool {
        self.loading.contains(path)
    }

    /// Check if loading failed for a path
    pub fn has_failed(&self, path: &PathBuf) -> bool {
        self.failed.contains(path)
    }

    /// Request loading a thumbnail (returns true if load was started)
    pub fn request_load(&mut self, path: PathBuf) -> bool {
        if self.texture_ids.contains_key(&path)
            || self.loading.contains(&path)
            || self.failed.contains(&path)
        {
            return false;
        }
        self.loading.insert(path);
        true
    }

    /// Mark a thumbnail as loaded with its handle
    pub fn mark_loaded(&mut self, path: PathBuf, handle: Handle<Image>) {
        self.loading.remove(&path);
        self.image_handles.insert(path, handle);
    }

    /// Mark a thumbnail as failed
    pub fn mark_failed(&mut self, path: PathBuf) {
        self.loading.remove(&path);
        self.failed.insert(path);
    }

    /// Register a texture ID for a path
    pub fn register_texture_id(&mut self, path: PathBuf, id: TextureId) {
        self.texture_ids.insert(path, id);
    }

    /// Clear cache when folder changes
    pub fn clear_for_folder_change(&mut self, new_folder: Option<PathBuf>) {
        if self.current_folder != new_folder {
            // Keep handles but clear texture IDs (egui manages those)
            // Only clear if we've moved to a different folder tree
            self.current_folder = new_folder;
        }
    }

    /// Check if we have a handle ready to register
    pub fn get_pending_handle(&self, path: &PathBuf) -> Option<&Handle<Image>> {
        if self.texture_ids.contains_key(path) {
            return None; // Already registered
        }
        self.image_handles.get(path)
    }
}

/// Identifies which asset types support image thumbnails (loaded directly as images)
/// Note: HDR and EXR are excluded as they often use texture formats incompatible with egui
pub fn supports_thumbnail(filename: &str) -> bool {
    let ext = filename.rsplit('.').next().unwrap_or("").to_lowercase();
    matches!(ext.as_str(), "png" | "jpg" | "jpeg" | "bmp" | "tga" | "webp")
}

/// Identifies model files that support 3D preview thumbnails
pub fn supports_model_preview(filename: &str) -> bool {
    let ext = filename.rsplit('.').next().unwrap_or("").to_lowercase();
    matches!(ext.as_str(), "glb" | "gltf")
}

/// Identifies model files that could have generated previews (future)
pub fn is_model_file_for_preview(filename: &str) -> bool {
    let ext = filename.rsplit('.').next().unwrap_or("").to_lowercase();
    matches!(ext.as_str(), "glb" | "gltf" | "obj" | "fbx" | "usd" | "usdz")
}

/// Identifies WGSL shader files that support rendered shader thumbnails
pub fn supports_shader_thumbnail(filename: &str) -> bool {
    let ext = filename.rsplit('.').next().unwrap_or("").to_lowercase();
    ext == "wgsl"
}
