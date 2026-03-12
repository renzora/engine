//! Custom asset reader with project-local override support.
//!
//! Lookup order:
//! 1. Absolute paths — pass through directly
//! 2. Project-local `assets/` override (when a project is open)
//! 3. Exe-adjacent `assets/` directory (exported runtime builds)
//! 4. CWD `assets/` directory (development fallback)
//!
//! Must be registered **before** `DefaultPlugins` via [`setup_asset_reader`].

use bevy::asset::io::{
    AssetReader, AssetReaderError, AssetSourceBuilder, AssetSourceId, PathStream, Reader,
    VecReader,
};
use bevy::prelude::*;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::{Arc, RwLock};
use std::task::{Context, Poll};

// ---------------------------------------------------------------------------
// Project asset override path (shared between reader and systems)
// ---------------------------------------------------------------------------

/// Shared project path that the asset reader checks for local overrides.
///
/// When a project is open, this holds the project directory path.
/// The asset reader checks `project_path/assets/{path}` before falling back
/// to engine-bundled assets.
#[derive(Resource, Clone)]
pub struct ProjectAssetPath(pub Arc<RwLock<Option<PathBuf>>>);

impl Default for ProjectAssetPath {
    fn default() -> Self {
        Self(Arc::new(RwLock::new(None)))
    }
}

impl ProjectAssetPath {
    pub fn set(&self, path: PathBuf) {
        if let Ok(mut lock) = self.0.write() {
            *lock = Some(path);
        }
    }
}

// ---------------------------------------------------------------------------
// Path stream helper
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// EmbeddedAssetReader
// ---------------------------------------------------------------------------

struct EmbeddedAssetReader {
    project_path: Arc<RwLock<Option<PathBuf>>>,
    /// Directory containing the executable (cached at startup).
    exe_dir: Option<PathBuf>,
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

    /// Try reading a file from the exe-adjacent assets directory.
    fn try_read_from_exe(&self, normalized: &str) -> Option<Vec<u8>> {
        let exe_dir = self.exe_dir.as_ref()?;
        let full_path = exe_dir.join("assets").join(normalized);
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

        // 3. Exe-adjacent assets (exported runtime builds)
        if let Some(bytes) = self.try_read_from_exe(&normalized) {
            return Ok(VecReader::new(bytes));
        }

        // 4. Fall back to CWD assets (development)
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

        // Project assets
        if let Ok(lock) = self.project_path.read() {
            if let Some(project_path) = lock.as_ref() {
                let dir = project_path.join("assets").join(&normalized);
                if dir.is_dir() {
                    let entries: Vec<PathBuf> = std::fs::read_dir(&dir)
                        .map_err(|_| AssetReaderError::NotFound(path.to_path_buf()))?
                        .filter_map(|e| e.ok())
                        .map(|e| e.path())
                        .collect();
                    return Ok(Box::new(VecPathStream { entries, index: 0 }));
                }
            }
        }

        // Exe-adjacent assets
        if let Some(exe_dir) = &self.exe_dir {
            let dir = exe_dir.join("assets").join(&normalized);
            if dir.is_dir() {
                let entries: Vec<PathBuf> = std::fs::read_dir(&dir)
                    .map_err(|_| AssetReaderError::NotFound(path.to_path_buf()))?
                    .filter_map(|e| e.ok())
                    .map(|e| e.path())
                    .collect();
                return Ok(Box::new(VecPathStream { entries, index: 0 }));
            }
        }

        // CWD assets
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

        if normalized.is_empty() || normalized == "." {
            return Ok(true);
        }

        // Project assets
        if let Ok(lock) = self.project_path.read() {
            if let Some(project_path) = lock.as_ref() {
                if project_path.join("assets").join(&normalized).is_dir() {
                    return Ok(true);
                }
            }
        }

        // Exe-adjacent assets
        if let Some(exe_dir) = &self.exe_dir {
            if exe_dir.join("assets").join(&normalized).is_dir() {
                return Ok(true);
            }
        }

        // CWD assets
        let local_path = PathBuf::from("assets").join(&normalized);
        Ok(local_path.is_dir())
    }
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Register the custom asset reader on the Bevy `App`.
///
/// Must be called **before** `DefaultPlugins` are added so that `AssetPlugin`
/// picks up our custom reader instead of the default filesystem reader.
pub fn setup_asset_reader(app: &mut App) -> ProjectAssetPath {
    let project_asset_path = ProjectAssetPath::default();
    let reader_path = project_asset_path.0.clone();

    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()));

    app.insert_resource(project_asset_path.clone());
    app.register_asset_source(
        AssetSourceId::Default,
        AssetSourceBuilder::new(move || Box::new(EmbeddedAssetReader {
            project_path: reader_path.clone(),
            exe_dir: exe_dir.clone(),
        })),
    );
    project_asset_path
}
