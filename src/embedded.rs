//! Embedded assets, plugins, and updater for single-binary distribution.
//!
//! In release builds, this module embeds the `assets/` and `plugins/` directories
//! into the binary at compile time using `include_dir!`, and the updater binary
//! via `include_bytes!`.
//!
//! In debug builds, all functions are no-ops — assets load from the filesystem
//! and plugins are copied to the target directory by build.rs.
//!
//! - Assets are served through `EmbeddedAssetReader` (Bevy's `AssetReader` trait).
//! - Plugin DLLs are extracted to `%LOCALAPPDATA%/renzora/system_plugins/` on startup
//!   because `libloading` requires files on disk.
//! - The updater is extracted as `update.exe` next to the main binary on startup.

use bevy::prelude::*;

#[cfg(not(debug_assertions))]
use bevy::asset::io::{AssetReader, AssetReaderError, AssetSourceBuilder, AssetSourceId, PathStream, Reader, VecReader};
#[cfg(not(debug_assertions))]
use include_dir::{include_dir, Dir};
#[cfg(not(debug_assertions))]
use std::path::Path;
#[cfg(not(debug_assertions))]
use std::pin::Pin;
#[cfg(not(debug_assertions))]
use std::task::{Context, Poll};

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
// EmbeddedAssetReader — serves assets from the embedded directory
// ---------------------------------------------------------------------------

/// Simple stream that yields paths from a Vec.
#[cfg(not(debug_assertions))]
struct VecPathStream {
    entries: Vec<std::path::PathBuf>,
    index: usize,
}

#[cfg(not(debug_assertions))]
impl futures_lite::Stream for VecPathStream {
    type Item = std::path::PathBuf;

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

#[cfg(not(debug_assertions))]
impl Unpin for VecPathStream {}

#[cfg(not(debug_assertions))]
struct EmbeddedAssetReader;

#[cfg(not(debug_assertions))]
impl EmbeddedAssetReader {
    fn normalize(path: &Path) -> String {
        let s = path.to_string_lossy();
        // Strip leading "assets/" if present (Bevy passes relative-to-source paths)
        let s = s.strip_prefix("assets/").unwrap_or(&s);
        s.replace('\\', "/")
    }

    /// Try reading a file from the real filesystem (for user project assets).
    fn read_from_filesystem(path: &Path) -> Result<Vec<u8>, AssetReaderError> {
        // Absolute paths are used for project assets (drag-and-drop, scene loads, etc.)
        if path.is_absolute() {
            return std::fs::read(path)
                .map_err(|_| AssetReaderError::NotFound(path.to_path_buf()));
        }
        Err(AssetReaderError::NotFound(path.to_path_buf()))
    }
}

#[cfg(not(debug_assertions))]
impl AssetReader for EmbeddedAssetReader {
    async fn read<'a>(&'a self, path: &'a Path) -> Result<impl Reader + 'a, AssetReaderError> {
        // Filesystem first — most loads are user project assets
        if let Ok(bytes) = Self::read_from_filesystem(path) {
            return Ok(VecReader::new(bytes));
        }
        // Fall back to embedded data (engine built-in assets)
        let normalized = Self::normalize(path);
        if let Some(file) = EMBEDDED_ASSETS.get_file(&normalized) {
            Ok(VecReader::new(file.contents().to_vec()))
        } else {
            Err(AssetReaderError::NotFound(path.to_path_buf()))
        }
    }

    async fn read_meta<'a>(&'a self, path: &'a Path) -> Result<impl Reader + 'a, AssetReaderError> {
        Err::<VecReader, _>(AssetReaderError::NotFound(path.to_path_buf()))
    }

    async fn read_directory<'a>(
        &'a self,
        path: &'a Path,
    ) -> Result<Box<PathStream>, AssetReaderError> {
        let normalized = Self::normalize(path);

        // Root directory listing
        let dir = if normalized.is_empty() || normalized == "." {
            Some(&EMBEDDED_ASSETS)
        } else {
            EMBEDDED_ASSETS.get_dir(&normalized)
        };

        if let Some(dir) = dir {
            let entries: Vec<std::path::PathBuf> = dir
                .entries()
                .iter()
                .map(|e| std::path::PathBuf::from(e.path()))
                .collect();
            Ok(Box::new(VecPathStream { entries, index: 0 }))
        } else {
            Err(AssetReaderError::NotFound(path.to_path_buf()))
        }
    }

    async fn is_directory<'a>(&'a self, path: &'a Path) -> Result<bool, AssetReaderError> {
        let normalized = Self::normalize(path);
        if normalized.is_empty() || normalized == "." {
            Ok(true)
        } else {
            Ok(EMBEDDED_ASSETS.get_dir(&normalized).is_some())
        }
    }
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Register the embedded asset source on the Bevy `App`.
///
/// Must be called **before** `DefaultPlugins` are added so that `AssetPlugin`
/// picks up our custom reader instead of the default filesystem reader.
#[cfg(not(debug_assertions))]
pub fn setup_embedded_assets(app: &mut App) {
    app.register_asset_source(
        AssetSourceId::Default,
        AssetSourceBuilder::new(move || Box::new(EmbeddedAssetReader)),
    );
}

#[cfg(debug_assertions)]
pub fn setup_embedded_assets(_app: &mut App) {
    // No-op: assets loaded from filesystem in debug builds
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
