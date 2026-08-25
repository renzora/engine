//! Cross-crate contract for scene file thumbnails (`.bsn` / `.ron`).
//!
//! Materials and models can be thumbnailed on demand — the renderer loads the
//! one asset, snaps it against a neutral backdrop, and caches the PNG. A scene
//! cannot: reproducing its picture means loading the entire scene, which is the
//! expensive thing the browser is trying to spare the user in the first place.
//!
//! So a scene's picture is taken at the one moment it costs nothing — when the
//! user saves. The viewport is already showing the scene, so `renzora_scene`
//! grabs that frame, downscales it, and writes
//! `<project>/.cache/thumbnails/scenes/<rel>.png`.
//!
//! This registry is therefore **load-only**: [`request`](SceneThumbnailRegistry::request)
//! never enqueues a capture, it only picks up a PNG a previous save left behind.
//! A path with nothing on disk is remembered in `missing` so the browser's
//! per-frame sweep doesn't stat the filesystem once per visible tile per frame.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use bevy::prelude::*;

use renzora::core::CurrentProject;

#[derive(Resource, Default)]
pub struct SceneThumbnailRegistry {
    /// Scene path → the loaded (or freshly captured) `Handle<Image>`.
    handles: HashMap<PathBuf, Handle<Image>>,
    /// Scene paths we've already looked for and found no PNG for. Cleared for a
    /// path as soon as a save publishes one through [`SceneThumbnailRegistry::complete`].
    missing: HashSet<PathBuf>,
}

impl SceneThumbnailRegistry {
    /// The thumbnail's image handle, if one is available.
    pub fn handle(&self, path: &PathBuf) -> Option<Handle<Image>> {
        self.handles.get(path).cloned()
    }

    /// Pick up the cached PNG a previous save wrote for `path`, if there is one.
    ///
    /// Cheap to call every frame: a hit and a known-miss both short-circuit
    /// before touching the filesystem. Deliberately does *not* compare mtimes
    /// against the scene file — a thumbnail only ever comes from a save, so a
    /// scene edited outside the editor would lose its preview permanently
    /// rather than merely showing a slightly old one.
    pub fn request(
        &mut self,
        path: PathBuf,
        asset_server: &AssetServer,
        project: Option<&CurrentProject>,
    ) {
        if self.handles.contains_key(&path) || self.missing.contains(&path) {
            return;
        }
        // Without a project there's no cache root to look in — and no `missing`
        // entry either, so the lookup retries once a project is open.
        let Some(project) = project else {
            return;
        };
        let thumb = scene_thumb_path(&path, project);
        if !thumb.is_file() {
            self.missing.insert(path);
            return;
        }
        // The asset root is the project root, so the cache PNG is loadable by
        // the asset server as `.cache/thumbnails/scenes/...`. A path that
        // didn't relativize is outside the project and can't be loaded.
        let rel = project.make_asset_relative(&thumb);
        if Path::new(&rel).is_absolute() {
            self.missing.insert(path);
            return;
        }
        self.handles.insert(path, asset_server.load(rel));
    }

    /// Publish a thumbnail for `path`. Called by the save-time capture with the
    /// image it just wrote to disk — handing over the in-memory copy rather
    /// than re-loading the file, because the asset server would serve the
    /// previous save's bytes for that unchanged path.
    pub fn complete(&mut self, path: PathBuf, handle: Handle<Image>) {
        self.missing.remove(&path);
        self.handles.insert(path, handle);
    }

    /// Drop a cached entry so the next `request` looks at disk again. Call when
    /// the scene file moves or is deleted.
    pub fn invalidate(&mut self, path: &PathBuf) {
        self.handles.remove(path);
        self.missing.remove(path);
    }

    /// Forget everything. Called when a project is re-opened from inside the
    /// editor, so the previous project's hits and known-misses don't
    /// short-circuit the new project's lookups.
    pub fn reset(&mut self) {
        self.handles.clear();
        self.missing.clear();
    }
}

/// Path on disk where the cached PNG thumbnail for a scene file lives.
///
/// Example: `<project>/assets/scenes/level.bsn` →
/// `<project>/.cache/thumbnails/scenes/scenes/level.bsn.png`. If the scene
/// path isn't under the project, falls back to a flattened name.
///
/// The extension is **appended**, not replaced (which is what
/// [`crate::model_thumb_path`] does): a project mid-BSN-migration can hold
/// `level.bsn` and `level.ron` side by side, and `set_extension` would collapse
/// both onto one `level.png` so each save would overwrite the other's preview.
pub fn scene_thumb_path(scene_abs: &Path, project: &CurrentProject) -> PathBuf {
    let rel = project.make_relative(scene_abs).unwrap_or_else(|| {
        scene_abs
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default()
    });
    let rel = rel.strip_prefix("assets/").unwrap_or(&rel);
    crate::thumbnail_cache_dir(project, "scenes").join(format!("{rel}.png"))
}
