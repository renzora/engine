//! Virtual filesystem — transparently reads from `.rpak` or disk.
//!
//! At startup the runtime checks:
//! 1. Embedded rpak in the current executable (self-contained mode)
//! 2. Adjacent `.rpak` file next to the executable
//! 3. Falls back to raw filesystem (development / `--project` mode)

use bevy::prelude::*;
use renzora_rpak::RpakArchive;
use std::path::{Path, PathBuf};

/// Bevy resource providing a virtual filesystem backed by an `.rpak` archive.
///
/// When no archive is loaded, all reads go through the normal filesystem.
#[derive(Resource, Default)]
pub struct Vfs {
    archive: Option<RpakArchive>,
    /// If archive was loaded, this is the "virtual project root" (temp dir where
    /// project.toml etc. are extracted, or empty if we read directly from archive).
    project_root: Option<PathBuf>,
}

impl Vfs {
    /// Try to initialize the VFS from an embedded or adjacent rpak.
    pub fn detect() -> Self {
        // 1. Check for embedded rpak in current exe
        match RpakArchive::from_current_exe() {
            Ok(Some(archive)) => {
                info!("Loaded embedded .rpak ({} files)", archive.len());
                return Self {
                    archive: Some(archive),
                    project_root: None,
                };
            }
            Ok(None) => {}
            Err(e) => {
                warn!("Failed to check exe for embedded rpak: {}", e);
            }
        }

        // 2. Check for adjacent .rpak file
        if let Ok(exe_path) = std::env::current_exe() {
            if let Some(exe_dir) = exe_path.parent() {
                let stem = exe_path.file_stem().unwrap_or_default();
                let rpak_path = exe_dir.join(format!("{}.rpak", stem.to_string_lossy()));
                if rpak_path.exists() {
                    match RpakArchive::from_file(&rpak_path) {
                        Ok(archive) => {
                            info!(
                                "Loaded adjacent .rpak: {} ({} files)",
                                rpak_path.display(),
                                archive.len()
                            );
                            return Self {
                                archive: Some(archive),
                                project_root: None,
                            };
                        }
                        Err(e) => {
                            error!("Failed to load {}: {}", rpak_path.display(), e);
                        }
                    }
                }
            }
        }

        // 3. No rpak found — normal filesystem mode
        Self {
            archive: None,
            project_root: None,
        }
    }

    /// Whether we have an rpak archive loaded.
    pub fn has_archive(&self) -> bool {
        self.archive.is_some()
    }

    /// Read a file by archive-relative path (or absolute/relative disk path as fallback).
    pub fn read(&self, path: &str) -> Option<Vec<u8>> {
        if let Some(ref archive) = self.archive {
            if let Some(data) = archive.get(path) {
                return Some(data.to_vec());
            }
        }
        // Fallback to filesystem
        std::fs::read(path).ok()
    }

    /// Read a file as UTF-8 string.
    pub fn read_string(&self, path: &str) -> Option<String> {
        self.read(path).and_then(|bytes| String::from_utf8(bytes).ok())
    }

    /// Check if a file exists (in archive or on disk).
    pub fn exists(&self, path: &str) -> bool {
        if let Some(ref archive) = self.archive {
            if archive.contains(path) {
                return true;
            }
        }
        Path::new(path).exists()
    }

    /// Get the underlying archive, if any.
    pub fn archive(&self) -> Option<&RpakArchive> {
        self.archive.as_ref()
    }

    /// Extract the entire archive to a temporary directory and return the path.
    /// Useful for systems that need filesystem paths (e.g., Bevy asset server).
    pub fn extract_to_temp(&self) -> Option<PathBuf> {
        let archive = self.archive.as_ref()?;
        let temp_dir = std::env::temp_dir().join("renzora_runtime_vfs");
        if let Err(e) = archive.extract_to(&temp_dir) {
            error!("Failed to extract rpak to temp: {}", e);
            return None;
        }
        Some(temp_dir)
    }
}
