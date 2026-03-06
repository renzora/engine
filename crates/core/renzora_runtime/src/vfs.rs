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
        // Android: load rpak from APK assets
        #[cfg(target_os = "android")]
        {
            if let Some(vfs) = Self::detect_android() {
                return vfs;
            }
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
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
        }

        // No rpak found (or WASM) — normal filesystem mode
        Self {
            archive: None,
            project_root: None,
        }
    }

    /// On Android, try to load game.rpak from APK assets or internal storage.
    #[cfg(target_os = "android")]
    fn detect_android() -> Option<Self> {
        // 1. Check if already extracted to internal storage
        let candidates = [
            "/data/data/com.renzora.runtime/files/game.rpak",
            "/data/data/com.renzora.runtime/game.rpak",
        ];

        for path_str in &candidates {
            let path = std::path::Path::new(path_str);
            if path.exists() {
                match RpakArchive::from_file(path) {
                    Ok(archive) => {
                        info!("Loaded Android rpak from {} ({} files)", path_str, archive.len());
                        return Some(Self {
                            archive: Some(archive),
                            project_root: None,
                        });
                    }
                    Err(e) => {
                        warn!("Failed to load Android rpak at {}: {}", path_str, e);
                    }
                }
            }
        }

        // 2. Read assets/game.rpak from the APK via Android AssetManager
        if let Some(vfs) = Self::load_from_apk_assets() {
            return Some(vfs);
        }

        warn!("No rpak found on Android — running in filesystem mode");
        None
    }

    /// Read game.rpak from APK assets/ using the Android AssetManager NDK API.
    #[cfg(target_os = "android")]
    fn load_from_apk_assets() -> Option<Self> {
        use std::ffi::CString;

        let android_app = bevy::android::ANDROID_APP.get()?;
        let asset_manager = android_app.asset_manager();

        let filename = CString::new("game.rpak").ok()?;
        let mut asset = asset_manager.open(&filename)?;
        let data = match asset.buffer() {
            Ok(buf) => buf.to_vec(),
            Err(e) => {
                error!("Failed to read game.rpak from APK assets: {}", e);
                return None;
            }
        };

        info!("Read game.rpak from APK assets ({} bytes)", data.len());

        match RpakArchive::from_bytes(&data) {
            Ok(archive) => {
                info!("Loaded rpak from APK assets ({} files)", archive.len());
                Some(Self {
                    archive: Some(archive),
                    project_root: None,
                })
            }
            Err(e) => {
                error!("Failed to parse rpak from APK assets: {}", e);
                None
            }
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
    #[cfg(not(target_arch = "wasm32"))]
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
