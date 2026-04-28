//! Cross-crate contract for rendered `.material` file thumbnails.
//!
//! The asset browser requests a thumbnail for a `.material` path via
//! [`MaterialThumbnailRegistry::request`]. The material thumbnail renderer
//! (in `renzora_material_editor`) drains `incoming_requests`, captures a
//! one-shot render of a sphere with the compiled material, writes a PNG to
//! `<project>/.thumbs/materials/<rel>.png`, and publishes the resulting egui
//! `TextureId` via [`MaterialThumbnailRegistry::complete`].
//!
//! Thumbnails persist across sessions: when a request hits and the PNG is
//! already on disk, the renderer skips the capture and simply reloads the
//! file. Invalidation is expected when the material is saved.

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};

use bevy::prelude::*;
use bevy_egui::egui;

use renzora::core::CurrentProject;

#[derive(Resource, Default)]
pub struct MaterialThumbnailRegistry {
    entries: HashMap<PathBuf, egui::TextureId>,
    in_flight: HashSet<PathBuf>,
    pub incoming_requests: VecDeque<PathBuf>,
}

impl MaterialThumbnailRegistry {
    pub fn get(&self, path: &PathBuf) -> Option<egui::TextureId> {
        self.entries.get(path).copied()
    }

    pub fn entries(&self) -> &HashMap<PathBuf, egui::TextureId> {
        &self.entries
    }

    /// Non-blocking request. If the thumbnail is already cached or a capture
    /// is already queued for this path, this is a no-op.
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

    /// Called by the renderer when a capture failed (parse error, unsupported
    /// domain, IO error, etc.) — clears the in-flight flag so the request
    /// can be retried later.
    pub fn cancel(&mut self, path: &PathBuf) {
        self.in_flight.remove(path);
    }

    /// Forces a re-capture next time this file is viewed. Call on save.
    pub fn invalidate(&mut self, path: &PathBuf) {
        self.entries.remove(path);
        self.in_flight.remove(path);
    }

    /// Clear every cached thumbnail entry, every in-flight marker, and
    /// every pending request. Called when re-opening a project from
    /// inside the editor — without this, [`request`](Self::request)
    /// short-circuits on every path the previous session had thumbnailed,
    /// no requests enqueue, and the splash's "Material thumbnails" task
    /// is left stuck at 0/N forever.
    ///
    /// Doesn't touch the on-disk PNG cache — those are still valid
    /// across sessions and will be reloaded by the first re-request.
    pub fn reset(&mut self) {
        self.entries.clear();
        self.in_flight.clear();
        self.incoming_requests.clear();
    }
}

/// Path on disk where the cached PNG thumbnail for a `.material` file lives.
///
/// Example: `<project>/assets/shaders/rock.material` → `<project>/.thumbs/materials/shaders/rock.png`.
/// If the material path isn't under the project, falls back to a flattened name.
pub fn material_thumb_path(material_abs: &Path, project: &CurrentProject) -> PathBuf {
    let rel = project
        .make_relative(material_abs)
        .unwrap_or_else(|| material_abs.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default());
    let rel = rel.strip_prefix("assets/").unwrap_or(&rel);
    let mut out = project.path.join(".thumbs").join("materials").join(rel);
    out.set_extension("png");
    out
}
