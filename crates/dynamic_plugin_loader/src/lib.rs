//! Dynamic plugin loader for the Renzora engine.
//!
//! Plugins are `dylib` crates sharing `bevy_dylib` with the host.
//! Full `&mut App` access — same as built-in plugins.
//!
//! Plugins are loaded before `app.run()`. Restart to load new plugins.
//!
//! On platforms that don't support dynamic linking (WASM, mobile),
//! all functions are no-ops.

use bevy::prelude::*;
use std::path::{Path, PathBuf};

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
    use libloading::Library;
    use std::ffi::OsStr;

    // `*mut dyn Plugin` is a fat pointer (data + vtable). Passing it across
    // an FFI boundary is technically not C-ABI-safe, but Renzora's plugin
    // loader and the `add!` macro both produce/consume the same fat pointer
    // shape from Rust code compiled against the same bevy version, so the
    // ABI mismatch the compiler warns about can't actually occur in practice.
    #[allow(improper_ctypes_definitions)]
    type CreatePluginFn = extern "C" fn() -> *mut dyn Plugin;
    type ScopePluginFn = extern "C" fn() -> u8;
    type BevyHashFn = extern "C" fn() -> [u64; 2];
    // Bundle entry point: one cdylib, N plugins. `*mut App` is a thin pointer
    // (FFI-safe in practice; only the address crosses — both sides agree on
    // `App`'s layout via the shared `bevy_dylib`, enforced by `plugin_bevy_hash`).
    // `host_scope` is the `PluginScope` discriminant as `u8`; returns the count
    // of plugins that panicked during install (the bundle catches per-plugin, so
    // nothing unwinds across this boundary).
    #[allow(improper_ctypes_definitions)]
    type InstallScopeFn = extern "C" fn(*mut App, u8) -> u32;

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
        load_plugins_impl(app, plugin_dir, is_editor, false);
    }

    /// Load plugins while recursively walking `plugin_dir`. Use for game
    /// projects where dylibs may live alongside assets anywhere in the tree.
    pub fn load_plugins_recursive(app: &mut App, plugin_dir: &Path, is_editor: bool) {
        load_plugins_impl(app, plugin_dir, is_editor, true);
    }

    fn load_plugins_impl(app: &mut App, plugin_dir: &Path, is_editor: bool, recursive: bool) {
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

        let dll_paths = if recursive {
            discover_dlls_recursive(plugin_dir)
        } else {
            discover_dlls(plugin_dir)
        };

        for path in dll_paths {
            let stem = path
                .file_stem()
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
                library
                    .get::<BevyHashFn>(b"plugin_bevy_hash")
                    .ok()
                    .map(|f| (*f)() == engine_bevy_hash())
                    .unwrap_or(false)
            };

            if !compatible {
                let engine_hash = engine_bevy_hash();
                let plugin_hash = unsafe {
                    library
                        .get::<BevyHashFn>(b"plugin_bevy_hash")
                        .ok()
                        .map(|f| (*f)())
                };
                warn!(
                    "[dynamic-plugin] Skipping '{}' — incompatible bevy version (engine: {:?}, plugin: {:?})",
                    stem, engine_hash, plugin_hash
                );
                if let Some(mut registry) =
                    app.world_mut().get_resource_mut::<DynamicPluginRegistry>()
                {
                    registry.failed.push(FailedPlugin {
                        id: stem,
                        reason: "Incompatible bevy version — rebuild plugin with current engine"
                            .into(),
                    });
                }
                continue;
            }

            // ── Editor bundle found in plugins/ → SKIP ─────────────────────
            // A bundle cdylib exports `plugin_install_scope`. The editor bundle
            // (`renzora_editor.dll`) belongs BESIDE the exe and is loaded exactly
            // once via `load_bundle` — it must NOT be loaded from plugins/ too.
            // A bundle found here is a stale/misplaced artifact (e.g. an old
            // `renzora_editor_bundle.dll` left in the cargo cache and swept in by
            // packaging). Loading it would be a SECOND bundle install on top of
            // the beside-exe one, double-adding every editor plugin and panicking
            // Bevy ("plugin was already added"). Skip it. Community single
            // plugins fall through to the `plugin_create` path below.
            if unsafe { library.get::<InstallScopeFn>(b"plugin_install_scope") }.is_ok() {
                info!(
                    "[dynamic-plugin] Skipping '{}' in plugins/ — editor bundles load from \
                     beside the exe, not plugins/ (stale artifact?)",
                    stem
                );
                continue;
            }

            let scope = unsafe {
                library
                    .get::<ScopePluginFn>(b"plugin_scope")
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
                info!(
                    "[dynamic-plugin] Skipping '{}' ({:?}, editor={})",
                    stem, scope, is_editor
                );
                continue;
            }

            info!("[dynamic-plugin] Loading '{}' ({:?})", stem, scope);

            let create_fn: CreatePluginFn =
                match unsafe { library.get::<CreatePluginFn>(b"plugin_create") } {
                    Ok(sym) => *sym,
                    Err(e) => {
                        error!(
                            "[dynamic-plugin] Missing plugin_create in '{}': {e}",
                            path.display()
                        );
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
                    error!(
                        "[dynamic-plugin] '{}' panicked during creation",
                        path.display()
                    );
                }
            }
        }
    }

    /// Load exactly ONE bundle cdylib by path — the editor bundle that ships
    /// beside the exe (not in `plugins/`). Reuses the same ABI gate + the
    /// `plugin_install_scope` branch as `load_plugins_impl`, but does NOT
    /// directory-scan, so the host's own SDK dylibs (`renzora`,
    /// `renzora_editor_framework`, `bevy_dylib`) sitting next to the exe are never
    /// dlopened as plugins. Call AFTER `add_engine_plugins` so the runtime
    /// foundation + Runtime/EditorAndRuntime plugins already exist; the bundle
    /// then layers its Editor-scope plugins on top (host_scope = Editor),
    /// reproducing the old `add_editor_plugins` ordering.
    pub fn load_bundle(app: &mut App, bundle_path: &Path, is_editor: bool) {
        if !bundle_path.exists() {
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

        let stem = bundle_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string();

        let library = match unsafe { Library::new(bundle_path) } {
            Ok(lib) => lib,
            Err(e) => {
                error!(
                    "[dynamic-plugin] editor bundle load failed '{}': {e}",
                    bundle_path.display()
                );
                return;
            }
        };

        // Same ABI gate as the directory loader: the bundle must import the
        // exact `bevy_dylib` the host imports (one `--workspace` build) or its
        // `World` TypeId differs and every component/resource crossing the
        // boundary would be a distinct type. Reject on mismatch.
        let compatible = unsafe {
            library
                .get::<BevyHashFn>(b"plugin_bevy_hash")
                .ok()
                .map(|f| (*f)() == engine_bevy_hash())
                .unwrap_or(false)
        };
        if !compatible {
            warn!(
                "[dynamic-plugin] editor bundle '{}' — incompatible bevy version, skipped \
                 (rebuild the bundle in the same `--workspace` build as the host)",
                stem
            );
            return;
        }

        let install_fn: InstallScopeFn =
            match unsafe { library.get::<InstallScopeFn>(b"plugin_install_scope") } {
                Ok(sym) => *sym,
                Err(_) => {
                    error!(
                        "[dynamic-plugin] '{}' is not a bundle (no plugin_install_scope)",
                        stem
                    );
                    return;
                }
            };

        let host_scope: u8 = if is_editor {
            PluginScope::Editor as u8
        } else {
            PluginScope::Runtime as u8
        };
        info!(
            "[dynamic-plugin] Loading editor bundle '{}' (host_scope={})",
            stem, host_scope
        );
        // The bundle catches panics per-plugin internally — nothing unwinds
        // across this `extern "C"` boundary; it returns how many failed.
        let failures = install_fn(app, host_scope);
        if failures > 0 {
            warn!(
                "[dynamic-plugin] editor bundle '{}' — {} plugin(s) panicked during install",
                stem, failures
            );
        }
        info!("[dynamic-plugin] Registered editor bundle '{}'", stem);

        let mut registry = app.world_mut().resource_mut::<DynamicPluginRegistry>();
        registry.plugins.push(DynamicPluginInfo {
            id: stem,
            path: bundle_path.to_path_buf(),
            scope: if is_editor {
                PluginScope::Editor
            } else {
                PluginScope::EditorAndRuntime
            },
        });
        registry._libraries.push(library);
    }

    pub fn scan_plugins(plugin_dir: &Path) -> Vec<DynamicPluginInfo> {
        if !plugin_dir.exists() {
            return Vec::new();
        }

        let mut result = Vec::new();

        for path in discover_dlls(plugin_dir) {
            let stem = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_string();

            let library = match unsafe { Library::new(&path) } {
                Ok(lib) => lib,
                Err(_) => continue,
            };

            // Skip editor BUNDLE cdylibs (they export `plugin_install_scope`,
            // not the single-plugin `plugin_scope`/`plugin_create` trio). A
            // bundle is the removable editor, not a shippable game plugin —
            // without this it'd fall through to the `EditorAndRuntime` default
            // below and the export UI would offer to ship the editor in a game.
            if unsafe { library.get::<InstallScopeFn>(b"plugin_install_scope") }.is_ok() {
                continue;
            }

            let scope = unsafe {
                library
                    .get::<ScopePluginFn>(b"plugin_scope")
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

    /// Collect dylibs directly inside `dir` (non-recursive). Used for the
    /// engine's own `plugins/` folder next to the executable.
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

    /// Recursively walk `dir` for dylibs. Used for game projects where plugins
    /// may live alongside prefabs/assets anywhere in the tree.
    fn discover_dlls_recursive(dir: &Path) -> Vec<PathBuf> {
        let mut paths: Vec<PathBuf> = Vec::new();
        let mut stack: Vec<PathBuf> = vec![dir.to_path_buf()];
        while let Some(current) = stack.pop() {
            let Ok(entries) = std::fs::read_dir(&current) else {
                continue;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    stack.push(path);
                    continue;
                }
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
        let wide: Vec<u16> = dir
            .as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
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
    pub fn load_plugins_recursive(_app: &mut App, _plugin_dir: &Path, _is_editor: bool) {}
    pub fn load_bundle(_app: &mut App, _bundle_path: &Path, _is_editor: bool) {}
    pub fn scan_plugins(_plugin_dir: &Path) -> Vec<DynamicPluginInfo> {
        Vec::new()
    }
}

pub use platform::*;
