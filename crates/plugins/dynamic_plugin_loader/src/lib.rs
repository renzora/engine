//! Dynamic plugin loader for the Renzora engine.
//!
//! Plugins are `dylib` crates sharing `bevy_dylib` with the host.
//! Full `&mut App` access — same as built-in plugins.
//!
//! Plugins are loaded before `app.run()`. Restart to load new plugins.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use bevy::prelude::*;
use libloading::Library;

pub use dynamic_plugin_meta::PluginScope;

/// Type for plugin_create() -> *mut dyn Plugin
type CreatePluginFn = extern "C" fn() -> *mut dyn Plugin;

/// Type for plugin_scope() -> u8
type ScopePluginFn = extern "C" fn() -> u8;

/// Type for plugin_bevy_hash() -> [u64; 2]
type BevyHashFn = extern "C" fn() -> [u64; 2];

/// Get the engine's bevy hash for compatibility checking.
fn engine_bevy_hash() -> [u64; 2] {
    let id = std::any::TypeId::of::<bevy::ecs::world::World>();
    unsafe { std::mem::transmute(id) }
}

#[cfg(target_os = "windows")]
const DLL_EXT: &str = "dll";
#[cfg(target_os = "linux")]
const DLL_EXT: &str = "so";
#[cfg(target_os = "macos")]
const DLL_EXT: &str = "dylib";

#[derive(Debug, Clone)]
pub struct DynamicPluginInfo {
    pub id: String,
    pub path: PathBuf,
    pub scope: PluginScope,
}

#[derive(Debug, Clone)]
pub struct FailedPlugin {
    pub id: String,
    pub reason: String,
}

/// Keeps loaded plugin libraries alive.
#[derive(Resource, Default)]
pub struct DynamicPluginRegistry {
    pub plugins: Vec<DynamicPluginInfo>,
    pub failed: Vec<FailedPlugin>,
    _libraries: Vec<Library>,
}

/// Load plugins from a directory and add them to the app.
/// Call this before `app.run()`.
/// `is_editor`: true = load Editor + EditorAndRuntime, false = load Runtime + EditorAndRuntime.
pub fn load_plugins(app: &mut App, plugin_dir: &Path, is_editor: bool) {
    if !plugin_dir.exists() {
        return;
    }

    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            add_dll_search_dir(exe_dir);
        }
    }

    if !app.world().contains_resource::<DynamicPluginRegistry>() {
        app.init_resource::<DynamicPluginRegistry>();
    }

    let dll_paths = discover_dlls(plugin_dir);

    for path in dll_paths {
        let stem = path.file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string();

        let library = match unsafe { Library::new(&path) } {
            Ok(lib) => lib,
            Err(e) => {
                error!("[dynamic-plugin] Failed to load '{}': {e}", path.display());
                continue;
            }
        };

        // Check bevy compatibility — skip if plugin was built against a different bevy
        let compatible = unsafe {
            library.get::<BevyHashFn>(b"plugin_bevy_hash")
                .ok()
                .map(|f| (*f)() == engine_bevy_hash())
                .unwrap_or(false) // no hash export = incompatible
        };

        if !compatible {
            warn!(
                "[dynamic-plugin] Skipping '{}' — incompatible bevy version (rebuild plugin with current engine)",
                stem
            );
            if let Some(mut registry) = app.world_mut().get_resource_mut::<DynamicPluginRegistry>() {
                registry.failed.push(FailedPlugin {
                    id: stem,
                    reason: "Incompatible bevy version — rebuild plugin with current engine".into(),
                });
            }
            continue;
        }

        // Read scope — default to EditorAndRuntime if not exported
        let scope = unsafe {
            library.get::<ScopePluginFn>(b"plugin_scope")
                .ok()
                .map(|f| match (*f)() {
                    0 => PluginScope::Editor,
                    1 => PluginScope::Runtime,
                    _ => PluginScope::EditorAndRuntime,
                })
                .unwrap_or(PluginScope::EditorAndRuntime)
        };

        // Filter by scope
        let should_load = match scope {
            PluginScope::EditorAndRuntime => true,
            PluginScope::Editor => is_editor,
            PluginScope::Runtime => !is_editor,
        };

        if !should_load {
            info!("[dynamic-plugin] Skipping '{}' ({:?}, editor={})", stem, scope, is_editor);
            continue;
        }

        info!("[dynamic-plugin] Loading '{}' ({:?})", stem, scope);

        let create_fn: CreatePluginFn = match unsafe {
            library.get::<CreatePluginFn>(b"plugin_create")
        } {
            Ok(sym) => *sym,
            Err(e) => {
                error!("[dynamic-plugin] Missing plugin_create in '{}': {e}", path.display());
                continue;
            }
        };

        let plugin = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let raw = create_fn();
            unsafe { Box::from_raw(raw) }
        }));

        match plugin {
            Ok(plugin) => {
                plugin.build(app);
                info!("[dynamic-plugin] Registered '{}'", stem);
                let mut registry = app.world_mut().resource_mut::<DynamicPluginRegistry>();
                registry.plugins.push(DynamicPluginInfo {
                    id: stem,
                    path: path.clone(),
                    scope,
                });
                registry._libraries.push(library);
            }
            Err(_) => {
                error!("[dynamic-plugin] '{}' panicked during creation", path.display());
            }
        }
    }
}

/// Scan a plugins directory and return info about each plugin without loading them.
/// Use this in the export overlay to show which plugins are available to ship.
pub fn scan_plugins(plugin_dir: &Path) -> Vec<DynamicPluginInfo> {
    if !plugin_dir.exists() {
        return Vec::new();
    }

    let mut result = Vec::new();

    for path in discover_dlls(plugin_dir) {
        let stem = path.file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string();

        let library = match unsafe { Library::new(&path) } {
            Ok(lib) => lib,
            Err(_) => continue,
        };

        let scope = unsafe {
            library.get::<ScopePluginFn>(b"plugin_scope")
                .ok()
                .map(|f| match (*f)() {
                    0 => PluginScope::Editor,
                    1 => PluginScope::Runtime,
                    _ => PluginScope::EditorAndRuntime,
                })
                .unwrap_or(PluginScope::EditorAndRuntime)
        };

        // Only include runtime-compatible plugins
        if matches!(scope, PluginScope::Runtime | PluginScope::EditorAndRuntime) {
            result.push(DynamicPluginInfo {
                id: stem,
                path: path.clone(),
                scope,
            });
        }
    }

    result
}

/// Type for xr_init_rendering(app: *mut c_void)
type XrInitRenderingFn = unsafe extern "C" fn(*mut std::ffi::c_void);

/// Pre-scan the plugins directory for an XR plugin.
/// If found, calls its `xr_init_rendering` function which replaces DefaultPlugins
/// with OpenXR stereo rendering.
///
/// Returns `true` if XR rendering was initialized (caller should NOT add DefaultPlugins).
/// The library is kept alive in the DynamicPluginRegistry.
pub fn try_init_xr_rendering(app: &mut App, plugin_dir: &Path) -> bool {
    if !plugin_dir.exists() {
        return false;
    }

    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            add_dll_search_dir(exe_dir);
        }
    }

    let xr_dll = find_xr_plugin(plugin_dir);
    let Some(path) = xr_dll else {
        return false;
    };

    info!("[dynamic-plugin] Found XR plugin: {}", path.display());

    let library = match unsafe { Library::new(&path) } {
        Ok(lib) => lib,
        Err(e) => {
            error!("[dynamic-plugin] Failed to load XR plugin: {e}");
            return false;
        }
    };

    // Compatibility check
    let compatible = unsafe {
        library.get::<BevyHashFn>(b"plugin_bevy_hash")
            .ok()
            .map(|f| (*f)() == engine_bevy_hash())
            .unwrap_or(false)
    };

    if !compatible {
        warn!("[dynamic-plugin] XR plugin incompatible — rebuild with current engine");
        return false;
    }

    // Call xr_init_rendering
    let init_fn = match unsafe { library.get::<XrInitRenderingFn>(b"xr_init_rendering") } {
        Ok(sym) => *sym,
        Err(e) => {
            warn!("[dynamic-plugin] XR plugin missing xr_init_rendering: {e}");
            return false;
        }
    };

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        unsafe { init_fn(app as *mut App as *mut std::ffi::c_void) };
    }));

    match result {
        Ok(()) => {
            info!("[dynamic-plugin] XR rendering initialized");
            // Keep library alive
            if !app.world().contains_resource::<DynamicPluginRegistry>() {
                app.init_resource::<DynamicPluginRegistry>();
            }
            app.world_mut()
                .resource_mut::<DynamicPluginRegistry>()
                ._libraries
                .push(library);
            true
        }
        Err(_) => {
            error!("[dynamic-plugin] XR rendering init panicked");
            false
        }
    }
}

/// Look for the XR plugin DLL in the plugins directory.
fn find_xr_plugin(dir: &Path) -> Option<PathBuf> {
    let candidates = ["renzora_xr"];
    for name in candidates {
        let path = dir.join(format!("{name}.{DLL_EXT}"));
        if path.exists() {
            return Some(path);
        }
        // Linux: libname.so
        let path = dir.join(format!("lib{name}.{DLL_EXT}"));
        if path.exists() {
            return Some(path);
        }
    }
    None
}

fn discover_dlls(dir: &Path) -> Vec<PathBuf> {
    let mut paths: Vec<PathBuf> = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(OsStr::to_str) != Some(DLL_EXT) {
                continue;
            }
            let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
            if stem.starts_with("bevy_dylib") || stem.starts_with("std-") {
                continue;
            }
            paths.push(path);
        }
    }
    paths.sort();
    paths
}

#[cfg(target_os = "windows")]
fn add_dll_search_dir(dir: &Path) {
    use std::os::windows::ffi::OsStrExt;
    let wide: Vec<u16> = dir.as_os_str().encode_wide().chain(std::iter::once(0)).collect();
    unsafe {
        #[link(name = "kernel32")]
        extern "system" {
            fn SetDllDirectoryW(path: *const u16) -> i32;
        }
        SetDllDirectoryW(wide.as_ptr());
    }
}

#[cfg(not(target_os = "windows"))]
fn add_dll_search_dir(_dir: &Path) {}
