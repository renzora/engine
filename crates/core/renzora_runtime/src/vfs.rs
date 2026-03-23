//! Virtual filesystem — transparently reads from `.rpak` or disk.
//!
//! At startup the runtime checks:
//! 1. Embedded rpak in the current executable (self-contained mode)
//! 2. Adjacent `.rpak` file next to the executable
//! 3. Falls back to raw filesystem (development / `--project` mode)

use bevy::prelude::*;
use renzora_rpak::RpakArchive;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

/// Static storage for rpak bytes injected from JS before app startup (WASM only).
static WASM_RPAK_BYTES: OnceLock<Vec<u8>> = OnceLock::new();

/// Called from JavaScript to provide the game.rpak bytes before init().
/// Must be called before the Bevy app starts.
pub fn set_wasm_rpak(bytes: Vec<u8>) {
    let _ = WASM_RPAK_BYTES.set(bytes);
}

/// Bevy resource providing a virtual filesystem backed by an `.rpak` archive.
///
/// When no archive is loaded, all reads go through the normal filesystem.
/// The archive is wrapped in `Arc` so it can be shared with the asset reader.
#[derive(Resource, Clone, Default)]
pub struct Vfs {
    archive: Option<Arc<RpakArchive>>,
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

        // iOS / tvOS: load rpak from app bundle
        #[cfg(any(target_os = "ios", target_os = "tvos"))]
        {
            if let Some(vfs) = Self::detect_ios() {
                return vfs;
            }
        }

        // WASM: check for rpak bytes injected from JavaScript
        #[cfg(target_arch = "wasm32")]
        {
            if let Some(bytes) = WASM_RPAK_BYTES.get() {
                match RpakArchive::from_bytes(bytes) {
                    Ok(archive) => {
                        info!("Loaded rpak from WASM ({} files)", archive.len());
                        return Self {
                            archive: Some(Arc::new(archive)),
                            project_root: None,
                        };
                    }
                    Err(e) => {
                        error!("Failed to parse WASM rpak: {}", e);
                    }
                }
            }
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
            // 1. Check for embedded rpak in current exe
            match RpakArchive::from_current_exe() {
                Ok(Some(archive)) => {
                    info!("Loaded embedded .rpak ({} files)", archive.len());
                    return Self {
                        archive: Some(Arc::new(archive)),
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
                                    archive: Some(Arc::new(archive)),
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
                            archive: Some(Arc::new(archive)),
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
                    archive: Some(Arc::new(archive)),
                    project_root: None,
                })
            }
            Err(e) => {
                error!("Failed to parse rpak from APK assets: {}", e);
                None
            }
        }
    }

    /// On iOS/tvOS, load game.rpak from the app bundle's resource directory.
    #[cfg(any(target_os = "ios", target_os = "tvos"))]
    fn detect_ios() -> Option<Self> {
        use std::ffi::{CStr, c_char};

        extern "C" {
            fn CFBundleGetMainBundle() -> *const std::ffi::c_void;
            fn CFBundleCopyResourceURL(
                bundle: *const std::ffi::c_void,
                resource_name: *const std::ffi::c_void,
                resource_type: *const std::ffi::c_void,
                sub_dir_name: *const std::ffi::c_void,
            ) -> *const std::ffi::c_void;
            fn CFURLGetFileSystemRepresentation(
                url: *const std::ffi::c_void,
                resolve_against_base: bool,
                buffer: *mut u8,
                max_buf_len: isize,
            ) -> bool;
            fn CFRelease(cf: *const std::ffi::c_void);
        }

        // Helper to create a CFString from a Rust &str
        fn cfstring(s: &str) -> *const std::ffi::c_void {
            extern "C" {
                fn CFStringCreateWithBytes(
                    alloc: *const std::ffi::c_void,
                    bytes: *const u8,
                    num_bytes: isize,
                    encoding: u32,
                    is_external: bool,
                ) -> *const std::ffi::c_void;
            }
            const K_CF_STRING_ENCODING_UTF8: u32 = 0x08000100;
            unsafe {
                CFStringCreateWithBytes(
                    std::ptr::null(),
                    s.as_ptr(),
                    s.len() as isize,
                    K_CF_STRING_ENCODING_UTF8,
                    false,
                )
            }
        }

        unsafe {
            let bundle = CFBundleGetMainBundle();
            if bundle.is_null() {
                warn!("iOS: could not get main bundle");
                return None;
            }

            let name = cfstring("game");
            let ext = cfstring("rpak");
            let url = CFBundleCopyResourceURL(bundle, name, ext, std::ptr::null());
            CFRelease(name);
            CFRelease(ext);

            if url.is_null() {
                warn!("iOS: game.rpak not found in app bundle");
                return None;
            }

            let mut buf = [0u8; 1024];
            let ok = CFURLGetFileSystemRepresentation(url, true, buf.as_mut_ptr(), buf.len() as isize);
            CFRelease(url);

            if !ok {
                warn!("iOS: could not get filesystem path for game.rpak");
                return None;
            }

            let c_path = CStr::from_ptr(buf.as_ptr() as *const c_char);
            let path_str = c_path.to_str().ok()?;
            let path = Path::new(path_str);

            match RpakArchive::from_file(path) {
                Ok(archive) => {
                    info!("Loaded iOS bundle rpak ({} files)", archive.len());
                    Some(Self {
                        archive: Some(Arc::new(archive)),
                        project_root: None,
                    })
                }
                Err(e) => {
                    error!("Failed to load iOS bundle rpak: {}", e);
                    None
                }
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
        self.archive.as_deref()
    }

    /// Get a shared handle to the archive (for passing to the asset reader).
    pub fn archive_arc(&self) -> Option<Arc<RpakArchive>> {
        self.archive.clone()
    }

    /// Load a VFS from raw rpak bytes (used by wasm fetch).
    pub fn from_rpak_bytes(bytes: &[u8]) -> Result<Self, String> {
        let archive = RpakArchive::from_bytes(bytes)
            .map_err(|e| format!("Failed to load rpak: {}", e))?;
        info!("Loaded rpak from bytes ({} files)", archive.len());
        Ok(Self {
            archive: Some(Arc::new(archive)),
            project_root: None,
        })
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
