//! Cross-crate contract for rendered model file thumbnails (`.glb`, `.gltf`).
//!
//! Mirrors [`MaterialThumbnailRegistry`](crate::MaterialThumbnailRegistry):
//! the asset browser requests a thumbnail for a model path via
//! [`ModelThumbnailRegistry::request`], the renderer (in
//! `renzora_asset_browser::model_thumbnails`) drains the queue, performs a
//! one-shot offscreen capture of the loaded GLB scene, writes a PNG to
//! `<project>/.cache/thumbnails/models/<rel>.png`, and publishes the
//! resulting egui `TextureId` via [`ModelThumbnailRegistry::complete`].
//!
//! Lives in `renzora_editor` rather than the asset browser so other panels
//! (inspector, asset preview, drag preview) can read the registry without
//! pulling in the renderer.

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};

use bevy::prelude::*;
use bevy_egui::egui;

use renzora::core::CurrentProject;

#[derive(Resource, Default)]
pub struct ModelThumbnailRegistry {
    entries: HashMap<PathBuf, egui::TextureId>,
    in_flight: HashSet<PathBuf>,
    pub incoming_requests: VecDeque<PathBuf>,
}

impl ModelThumbnailRegistry {
    pub fn get(&self, path: &PathBuf) -> Option<egui::TextureId> {
        self.entries.get(path).copied()
    }

    pub fn entries(&self) -> &HashMap<PathBuf, egui::TextureId> {
        &self.entries
    }

    /// Non-blocking request. If the thumbnail is already cached or a
    /// capture is already queued for this path, this is a no-op.
    pub fn request(&mut self, path: PathBuf) {
        if self.entries.contains_key(&path) || self.in_flight.contains(&path) {
            return;
        }
        self.in_flight.insert(path.clone());
        self.incoming_requests.push_back(path);
    }

    /// Called by the renderer when a thumbnail becomes available (either
    /// fresh capture or disk-cache reload).
    pub fn complete(&mut self, path: PathBuf, id: egui::TextureId) {
        self.in_flight.remove(&path);
        self.entries.insert(path, id);
    }

    /// Called by the renderer when a capture failed (asset load timeout,
    /// missing scene, etc.) — clears the in-flight flag so the request
    /// can be retried later.
    pub fn cancel(&mut self, path: &PathBuf) {
        self.in_flight.remove(path);
    }

    /// Forces a re-capture next time this model is viewed. Call when
    /// the source file changes on disk.
    pub fn invalidate(&mut self, path: &PathBuf) {
        self.entries.remove(path);
        self.in_flight.remove(path);
    }

    /// Drop every cached entry, every in-flight marker, and every
    /// pending request. Mirrors `MaterialThumbnailRegistry::reset` —
    /// called when re-opening a project from inside the editor so
    /// `request` doesn't short-circuit on every path the previous
    /// session had thumbnailed.
    pub fn reset(&mut self) {
        self.entries.clear();
        self.in_flight.clear();
        self.incoming_requests.clear();
    }
}

/// Path on disk where the cached PNG thumbnail for a model file lives.
///
/// Example: `<project>/assets/models/audi.glb` →
/// `<project>/.cache/thumbnails/models/models/audi.png`. If the model
/// path isn't under the project, falls back to a flattened name.
pub fn model_thumb_path(model_abs: &Path, project: &CurrentProject) -> PathBuf {
    let rel = project
        .make_relative(model_abs)
        .unwrap_or_else(|| {
            model_abs
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default()
        });
    let rel = rel.strip_prefix("assets/").unwrap_or(&rel);
    let mut out = crate::thumbnail_cache_dir(project, "models").join(rel);
    out.set_extension("png");
    out
}
