//! Cross-crate contract for rendered `.material` file thumbnails.
//!
//! The asset browser requests a thumbnail for a `.material` path via
//! [`MaterialThumbnailRegistry::request`]. The material thumbnail renderer
//! (in `renzora_material_editor`) drains `incoming_requests`, captures a
//! one-shot render of a sphere with the compiled material, writes a PNG to
//! `<project>/.cache/thumbnails/materials/<rel>.png`, and publishes the resulting egui
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

/// Root directory for persistent thumbnail caches inside a project.
///
/// `<project>/.cache/thumbnails/<kind>/...`. Each asset kind (materials,
/// textures, models, …) gets its own subdirectory so a thumbnail's path
/// is recoverable from `(kind, asset-relative path)` alone.
///
/// Sits under `.cache/` rather than the legacy `.thumbs/` so future
/// editor caches (compiled shaders, asset import state, etc.) can share
/// the same gitignored umbrella without colliding with thumbnails.
pub fn thumbnail_cache_dir(project: &CurrentProject, kind: &str) -> PathBuf {
    project.path.join(".cache").join("thumbnails").join(kind)
}

/// Best-effort migration from the legacy `<project>/.thumbs/` directory
/// to `<project>/.cache/thumbnails/`. Skipped if the new directory
/// already exists or the legacy one is absent.
///
/// On rename failure (e.g. cross-volume on platforms where rename is
/// volume-local), leaves both alone — regeneration under the new path
/// kicks in and the user can clean up the legacy directory manually.
pub fn migrate_legacy_thumbnail_cache(project: &CurrentProject) {
    let legacy = project.path.join(".thumbs");
    let new_root = project.path.join(".cache").join("thumbnails");
    if !legacy.exists() || new_root.exists() {
        return;
    }
    if let Some(parent) = new_root.parent() {
        if let Err(e) = std::fs::create_dir_all(parent) {
            bevy::log::warn!(
                "[thumbnails] couldn't create {} for migration: {}",
                parent.display(),
                e
            );
            return;
        }
    }
    match std::fs::rename(&legacy, &new_root) {
        Ok(_) => bevy::log::info!(
            "[thumbnails] migrated {} → {}",
            legacy.display(),
            new_root.display()
        ),
        Err(e) => bevy::log::warn!(
            "[thumbnails] couldn't migrate {} → {} ({}); will regenerate",
            legacy.display(),
            new_root.display(),
            e
        ),
    }
}

/// Path on disk where the cached PNG thumbnail for a `.material` file lives.
///
/// Example: `<project>/assets/shaders/rock.material` → `<project>/.cache/thumbnails/materials/shaders/rock.png`.
/// If the material path isn't under the project, falls back to a flattened name.
pub fn material_thumb_path(material_abs: &Path, project: &CurrentProject) -> PathBuf {
    let rel = project
        .make_relative(material_abs)
        .unwrap_or_else(|| material_abs.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default());
    let rel = rel.strip_prefix("assets/").unwrap_or(&rel);
    let mut out = thumbnail_cache_dir(project, "materials").join(rel);
    out.set_extension("png");
    out
}
