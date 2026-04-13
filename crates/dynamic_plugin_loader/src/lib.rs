//! Dynamic plugin loader for the Renzora engine.
//!
//! Plugins are `dylib` crates sharing `bevy_dylib` with the host.
//! Full `&mut App` access — same as built-in plugins.
//!
//! Plugins are loaded before `app.run()`. Restart to load new plugins.
//!
//! On platforms that don't support dynamic linking (WASM, mobile),
//! all functions are no-ops.

use std::path::{Path, PathBuf};
use bevy::prelude::*;

pub use renzora::PluginScope;

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

// ── Desktop: full dynamic loading ────────────────────────────────────────

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
mod platform {
    use super::*;
    use std::ffi::OsStr;
    use libloading::Library;

    // `*mut dyn Plugin` is a fat pointer (data + vtable). Passing it across
    // an FFI boundary is technically not C-ABI-safe, but Renzora's plugin
    // loader and the `add!` macro both produce/consume the same fat pointer
    // shape from Rust code compiled against the same bevy version, so the
    // ABI mismatch the compiler warns about can't actually occur in practice.
    #[allow(improper_ctypes_definitions)]
    type CreatePluginFn = extern "C" fn() -> *mut dyn Plugin;
    type ScopePluginFn = extern "C" fn() -> u8;
    type BevyHashFn = extern "C" fn() -> [u64; 2];

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

    #[derive(Resource, Default)]
    pub struct DynamicPluginRegistry {
        pub plugins: Vec<DynamicPluginInfo>,
        pub failed: Vec<FailedPlugin>,
        _libraries: Vec<Library>,
    }

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

            // Skip if already loaded (e.g. from dist/plugins/ when also in project/plugins/)
            if let Some(registry) = app.world().get_resource::<DynamicPluginRegistry>() {
                if registry.plugins.iter().any(|p| p.id == stem) {
                    info!("[dynamic-plugin] Skipping '{}' — already loaded", stem);
                    continue;
                }
            }

            let library = match unsafe { Library::new(&path) } {
                Ok(lib) => lib,
                Err(e) => {
                    error!("[dynamic-plugin] Failed to load '{}': {e}", path.display());
                    continue;
                }
            };

            let compatible = unsafe {
                library.get::<BevyHashFn>(b"plugin_bevy_hash")
                    .ok()
                    .map(|f| (*f)() == engine_bevy_hash())
                    .unwrap_or(false)
            };

            if !compatible {
                let engine_hash = engine_bevy_hash();
                let plugin_hash = unsafe {
                    library.get::<BevyHashFn>(b"plugin_bevy_hash")
                        .ok()
                        .map(|f| (*f)())
                };
                warn!(
                    "[dynamic-plugin] Skipping '{}' — incompatible bevy version (engine: {:?}, plugin: {:?})",
                    stem, engine_hash, plugin_hash
                );
                if let Some(mut registry) = app.world_mut().get_resource_mut::<DynamicPluginRegistry>() {
                    registry.failed.push(FailedPlugin {
                        id: stem,
                        reason: "Incompatible bevy version — rebuild plugin with current engine".into(),
                    });
                }
                continue;
            }

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
}

// ── Non-desktop: no-op stubs ─────────────────────────────────────────────

#[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
mod platform {
    use super::*;

    #[derive(Resource, Default)]
    pub struct DynamicPluginRegistry {
        pub plugins: Vec<DynamicPluginInfo>,
        pub failed: Vec<FailedPlugin>,
    }

    pub fn load_plugins(_app: &mut App, _plugin_dir: &Path, _is_editor: bool) {}
    pub fn scan_plugins(_plugin_dir: &Path) -> Vec<DynamicPluginInfo> { Vec::new() }
}

pub use platform::*;
