//! Embedded assets, plugins, and updater for single-binary distribution.
//!
//! In release builds, this module embeds the `assets/` and `plugins/` directories
//! into the binary at compile time using `include_dir!`, and the updater binary
//! via `include_bytes!`.
//!
//! In both debug and release builds, the custom `EmbeddedAssetReader` is registered
//! to support project-local asset overrides. When a project is open, assets are
//! looked up in the project's `assets/` directory first, then fall back to:
//! - Debug: the local `assets/` directory (filesystem)
//! - Release: the embedded `EMBEDDED_ASSETS` compiled into the binary
//!
//! - Assets are served through `EmbeddedAssetReader` (Bevy's `AssetReader` trait).
//! - Plugin DLLs are extracted to `%LOCALAPPDATA%/renzora/system_plugins/` on startup
//!   because `libloading` requires files on disk.
//! - The updater is extracted as `update.exe` next to the main binary on startup.

use bevy::prelude::*;
use bevy::asset::io::{AssetReader, AssetReaderError, AssetSourceBuilder, AssetSourceId, PathStream, Reader, VecReader};
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::{Arc, RwLock};
use std::task::{Context, Poll};

#[cfg(not(debug_assertions))]
use include_dir::{include_dir, Dir};

// ---------------------------------------------------------------------------
// Embedded data (only present in release builds)
// ---------------------------------------------------------------------------

#[cfg(not(debug_assertions))]
static EMBEDDED_ASSETS: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/assets");

#[cfg(not(debug_assertions))]
static EMBEDDED_PLUGINS: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/plugins");

#[cfg(not(debug_assertions))]
static EMBEDDED_UPDATER: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/renzora_updater.exe"));

// ---------------------------------------------------------------------------
// Project asset override path (shared between reader and systems)
// ---------------------------------------------------------------------------

/// Shared project path that the asset reader checks for local overrides.
///
/// When a project is open, this holds the project directory path.
/// The asset reader checks `project_path/assets/{path}` before falling back
/// to engine-bundled assets.
#[derive(Resource, Clone)]
pub struct ProjectAssetOverridePath(pub Arc<RwLock<Option<PathBuf>>>);

impl Default for ProjectAssetOverridePath {
    fn default() -> Self {
        Self(Arc::new(RwLock::new(None)))
    }
}

// ---------------------------------------------------------------------------
// EmbeddedAssetReader — serves assets with project-local override support
// ---------------------------------------------------------------------------

/// Simple stream that yields paths from a Vec.
struct VecPathStream {
    entries: Vec<PathBuf>,
    index: usize,
}

impl futures_lite::Stream for VecPathStream {
    type Item = PathBuf;

    fn poll_next(mut self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if self.index < self.entries.len() {
            let item = self.entries[self.index].clone();
            self.index += 1;
            Poll::Ready(Some(item))
        } else {
            Poll::Ready(None)
        }
    }
}

impl Unpin for VecPathStream {}

struct EmbeddedAssetReader {
    project_path: Arc<RwLock<Option<PathBuf>>>,
}

impl EmbeddedAssetReader {
    fn normalize(path: &Path) -> String {
        let s = path.to_string_lossy();
        // Strip leading "assets/" if present (Bevy passes relative-to-source paths)
        let s = s.strip_prefix("assets/").unwrap_or(&s);
        s.replace('\\', "/")
    }

    /// Try reading a file from the project's assets directory.
    fn try_read_from_project(&self, normalized: &str) -> Option<Vec<u8>> {
        let lock = self.project_path.read().ok()?;
        let project_path = lock.as_ref()?;
        let full_path = project_path.join("assets").join(normalized);
        std::fs::read(&full_path).ok()
    }
}

impl AssetReader for EmbeddedAssetReader {
    async fn read<'a>(&'a self, path: &'a Path) -> Result<impl Reader + 'a, AssetReaderError> {
        // 1. Absolute paths — used for project assets (drag-and-drop, scene loads, etc.)
        if path.is_absolute() {
            if let Ok(bytes) = std::fs::read(path) {
                return Ok(VecReader::new(bytes));
            }
        }

        let normalized = Self::normalize(path);

        // 2. Project-local override (checked when a project is open)
        if let Some(bytes) = self.try_read_from_project(&normalized) {
            return Ok(VecReader::new(bytes));
        }

        // 3. Fall back to engine assets
        #[cfg(not(debug_assertions))]
        {
            if let Some(file) = EMBEDDED_ASSETS.get_file(&normalized) {
                return Ok(VecReader::new(file.contents().to_vec()));
            }
        }

        #[cfg(debug_assertions)]
        {
            let local_path = PathBuf::from("assets").join(&normalized);
            if let Ok(bytes) = std::fs::read(&local_path) {
                return Ok(VecReader::new(bytes));
            }
        }

        Err(AssetReaderError::NotFound(path.to_path_buf()))
    }

    async fn read_meta<'a>(&'a self, path: &'a Path) -> Result<impl Reader + 'a, AssetReaderError> {
        Err::<VecReader, _>(AssetReaderError::NotFound(path.to_path_buf()))
    }

    async fn read_directory<'a>(
        &'a self,
        path: &'a Path,
    ) -> Result<Box<PathStream>, AssetReaderError> {
        let normalized = Self::normalize(path);

        #[cfg(not(debug_assertions))]
        {
            let dir = if normalized.is_empty() || normalized == "." {
                Some(&EMBEDDED_ASSETS)
            } else {
                EMBEDDED_ASSETS.get_dir(&normalized)
            };

            if let Some(dir) = dir {
                let entries: Vec<PathBuf> = dir
                    .entries()
                    .iter()
                    .map(|e| PathBuf::from(e.path()))
                    .collect();
                return Ok(Box::new(VecPathStream { entries, index: 0 }));
            }
        }

        #[cfg(debug_assertions)]
        {
            let local_path = PathBuf::from("assets").join(&normalized);
            if local_path.is_dir() {
                let entries: Vec<PathBuf> = std::fs::read_dir(&local_path)
                    .map_err(|_| AssetReaderError::NotFound(path.to_path_buf()))?
                    .filter_map(|e| e.ok())
                    .map(|e| e.path())
                    .collect();
                return Ok(Box::new(VecPathStream { entries, index: 0 }));
            }
        }

        Err(AssetReaderError::NotFound(path.to_path_buf()))
    }

    async fn is_directory<'a>(&'a self, path: &'a Path) -> Result<bool, AssetReaderError> {
        let normalized = Self::normalize(path);

        #[cfg(not(debug_assertions))]
        {
            if normalized.is_empty() || normalized == "." {
                return Ok(true);
            }
            return Ok(EMBEDDED_ASSETS.get_dir(&normalized).is_some());
        }

        #[cfg(debug_assertions)]
        {
            if normalized.is_empty() || normalized == "." {
                return Ok(true);
            }
            let local_path = PathBuf::from("assets").join(&normalized);
            return Ok(local_path.is_dir());
        }
    }
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Register the custom asset reader on the Bevy `App`.
///
/// Must be called **before** `DefaultPlugins` are added so that `AssetPlugin`
/// picks up our custom reader instead of the default filesystem reader.
///
/// In both debug and release builds, this registers the `EmbeddedAssetReader`
/// which supports project-local asset overrides.
pub fn setup_embedded_assets(app: &mut App) {
    let override_path = ProjectAssetOverridePath::default();
    let reader_path = override_path.0.clone();
    app.insert_resource(override_path);
    app.register_asset_source(
        AssetSourceId::Default,
        AssetSourceBuilder::new(move || Box::new(EmbeddedAssetReader {
            project_path: reader_path.clone(),
        })),
    );
}

/// Copy an engine asset into the project's assets directory.
///
/// `project_path` is the project root directory.
/// `asset_path` is the path relative to `assets/`, e.g. `"shaders/clouds.wgsl"`.
///
/// In release builds, reads from the embedded `EMBEDDED_ASSETS`.
/// In debug builds, reads from the local `assets/` directory.
///
/// Does nothing if the file already exists in the project.
/// Returns `true` on success or if the file already existed.
pub fn copy_engine_asset_to_project(project_path: &Path, asset_path: &str) -> bool {
    let normalized = asset_path.replace('\\', "/");
    let dest = project_path.join("assets").join(&normalized);

    // Already exists — don't overwrite user customizations
    if dest.exists() {
        return true;
    }

    // Read from engine source
    let source_bytes: Option<Vec<u8>> = {
        #[cfg(not(debug_assertions))]
        {
            EMBEDDED_ASSETS.get_file(&normalized).map(|f| f.contents().to_vec())
        }

        #[cfg(debug_assertions)]
        {
            let local_path = PathBuf::from("assets").join(&normalized);
            std::fs::read(&local_path).ok()
        }
    };

    let Some(bytes) = source_bytes else {
        warn!("Engine asset not found for copy: {}", asset_path);
        return false;
    };

    // Create parent directories
    if let Some(parent) = dest.parent() {
        if let Err(e) = std::fs::create_dir_all(parent) {
            error!("Failed to create directory {}: {}", parent.display(), e);
            return false;
        }
    }

    match std::fs::write(&dest, &bytes) {
        Ok(_) => {
            info!("Copied engine asset to project: {}", dest.display());
            true
        }
        Err(e) => {
            error!("Failed to copy engine asset to project: {}", e);
            false
        }
    }
}

/// Extract embedded plugin DLLs to `%LOCALAPPDATA%/renzora/system_plugins/`.
///
/// Returns the extraction directory path on success.
#[cfg(not(debug_assertions))]
pub fn extract_embedded_plugins() -> Option<std::path::PathBuf> {
    let dir = dirs::data_local_dir()?.join("renzora").join("system_plugins");

    if let Err(e) = std::fs::create_dir_all(&dir) {
        error!("Failed to create system_plugins dir: {}", e);
        return None;
    }

    for file in EMBEDDED_PLUGINS.files() {
        let dest = dir.join(file.path().file_name().unwrap_or(file.path().as_os_str()));
        // Only overwrite if sizes differ (cheap freshness check)
        let needs_write = match std::fs::metadata(&dest) {
            Ok(meta) => meta.len() != file.contents().len() as u64,
            Err(_) => true,
        };
        if needs_write {
            if let Err(e) = std::fs::write(&dest, file.contents()) {
                error!("Failed to extract plugin {:?}: {}", dest, e);
            }
        }
    }

    Some(dir)
}

#[cfg(debug_assertions)]
pub fn extract_embedded_plugins() -> Option<std::path::PathBuf> {
    None // No embedded plugins in debug builds
}

/// Extract the embedded updater as `update.exe` next to the main binary.
#[cfg(not(debug_assertions))]
pub fn extract_embedded_updater() {
    let Some(exe_path) = std::env::current_exe().ok() else {
        error!("Could not determine current exe path for updater extraction");
        return;
    };
    let Some(exe_dir) = exe_path.parent() else {
        error!("Current exe has no parent directory");
        return;
    };
    let updater_path = exe_dir.join("update.exe");

    // Only overwrite if sizes differ
    let needs_write = match std::fs::metadata(&updater_path) {
        Ok(meta) => meta.len() != EMBEDDED_UPDATER.len() as u64,
        Err(_) => true,
    };
    if needs_write {
        if let Err(e) = std::fs::write(&updater_path, EMBEDDED_UPDATER) {
            error!("Failed to extract updater: {}", e);
        }
    }
}

#[cfg(debug_assertions)]
pub fn extract_embedded_updater() {
    // No-op: updater is a separate binary in debug builds
}

/// Returns the system plugin directory for release builds.
///
/// In release builds, plugins are extracted to a known location under
/// `%LOCALAPPDATA%` rather than sitting next to the exe.
#[cfg(not(debug_assertions))]
pub fn packed_system_plugin_dir() -> Option<std::path::PathBuf> {
    dirs::data_local_dir().map(|p| p.join("renzora").join("system_plugins"))
}

#[cfg(debug_assertions)]
pub fn packed_system_plugin_dir() -> Option<std::path::PathBuf> {
    None
}
