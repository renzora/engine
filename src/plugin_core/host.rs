//! Plugin host for discovering, loading, and managing plugins.

use std::collections::HashMap;
use std::ffi::{c_char, c_void, CStr, OsStr};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver};
use std::sync::Mutex;

use bevy::prelude::*;
use libloading::Library;
use notify::{Config, Event, RecommendedWatcher, RecursiveMode, Watcher};

use super::abi::{PluginError, PluginManifest, EDITOR_API_VERSION};
use super::api::EditorApiImpl;
use super::dependency::DependencyGraph;
use super::traits::EditorEvent;
use editor_plugin_api::ffi::{PluginExport, PluginVTable, PluginHandle, FfiStatusBarItem, HostApi, FFI_API_VERSION};
use crate::plugin_core::{StatusBarAlign, StatusBarItem};
use crate::core::resources::console::{console_log, LogLevel};

/// Type for the FFI create_plugin function
type CreatePluginFn = unsafe extern "C" fn() -> PluginExport;

// ============================================================================
// Host callback implementations - these are called by plugins via FFI
// ============================================================================

unsafe extern "C" fn host_log_info(ctx: *mut c_void, message: *const c_char) {
    if message.is_null() { return; }
    let msg = CStr::from_ptr(message).to_string_lossy();
    info!("[Plugin] {}", msg);
    let _ = ctx; // ctx is EditorApiImpl but we don't need it for logging
}

unsafe extern "C" fn host_log_warn(ctx: *mut c_void, message: *const c_char) {
    if message.is_null() { return; }
    let msg = CStr::from_ptr(message).to_string_lossy();
    warn!("[Plugin] {}", msg);
    let _ = ctx;
}

unsafe extern "C" fn host_log_error(ctx: *mut c_void, message: *const c_char) {
    if message.is_null() { return; }
    let msg = CStr::from_ptr(message).to_string_lossy();
    error!("[Plugin] {}", msg);
    let _ = ctx;
}

unsafe extern "C" fn host_set_status_item(ctx: *mut c_void, item: *const FfiStatusBarItem) {
    if ctx.is_null() || item.is_null() { return; }
    let api_impl = &mut *(ctx as *mut EditorApiImpl);
    let ffi_item = &*item;

    let id = ffi_item.id.to_string();
    let text = ffi_item.text.to_string();
    let icon = if ffi_item.icon.ptr.is_null() { None } else { Some(ffi_item.icon.to_string()) };
    let tooltip = if ffi_item.tooltip.ptr.is_null() { None } else { Some(ffi_item.tooltip.to_string()) };

    let status_item = StatusBarItem {
        id: id.clone(),
        icon,
        text,
        tooltip,
        align: if ffi_item.align_right { StatusBarAlign::Right } else { StatusBarAlign::Left },
        priority: ffi_item.priority,
    };

    // Store in API - use current plugin ID if available
    let plugin_id = api_impl.current_plugin_id.clone().unwrap_or_default();
    api_impl.status_bar_items.insert(id, (status_item, plugin_id));
}

unsafe extern "C" fn host_remove_status_item(ctx: *mut c_void, id: *const c_char) {
    if ctx.is_null() || id.is_null() { return; }
    let api_impl = &mut *(ctx as *mut EditorApiImpl);
    let id_str = CStr::from_ptr(id).to_string_lossy().into_owned();
    api_impl.status_bar_items.remove(&id_str);
}

unsafe extern "C" fn host_undo(ctx: *mut c_void) -> bool {
    if ctx.is_null() { return false; }
    let api_impl = &mut *(ctx as *mut EditorApiImpl);
    if api_impl.can_undo {
        api_impl.pending_undo = true;
        true
    } else {
        false
    }
}

unsafe extern "C" fn host_redo(ctx: *mut c_void) -> bool {
    if ctx.is_null() { return false; }
    let api_impl = &mut *(ctx as *mut EditorApiImpl);
    if api_impl.can_redo {
        api_impl.pending_redo = true;
        true
    } else {
        false
    }
}

unsafe extern "C" fn host_can_undo(ctx: *mut c_void) -> bool {
    if ctx.is_null() { return false; }
    let api_impl = &*(ctx as *const EditorApiImpl);
    api_impl.can_undo
}

unsafe extern "C" fn host_can_redo(ctx: *mut c_void) -> bool {
    if ctx.is_null() { return false; }
    let api_impl = &*(ctx as *const EditorApiImpl);
    api_impl.can_redo
}

/// FFI-safe wrapper for a loaded plugin
pub struct FfiPluginWrapper {
    /// Plugin handle (opaque pointer to plugin state)
    handle: PluginHandle,
    /// Vtable with function pointers
    vtable: PluginVTable,
    /// Cached manifest
    manifest: PluginManifest,
    /// Whether the plugin is enabled
    enabled: bool,
}

// Safety: Plugin handles are only accessed from the main thread via Bevy's systems.
// The plugin state is owned by this wrapper and properly deallocated on drop.
unsafe impl Send for FfiPluginWrapper {}
unsafe impl Sync for FfiPluginWrapper {}

impl FfiPluginWrapper {
    pub fn new(export: PluginExport) -> Self {
        // Get manifest via FFI
        let ffi_manifest = unsafe { (export.vtable.manifest)(export.handle) };
        let manifest = unsafe { ffi_manifest.into_manifest() };

        Self {
            handle: export.handle,
            vtable: export.vtable,
            manifest,
            enabled: true,
        }
    }

    pub fn manifest(&self) -> &PluginManifest {
        &self.manifest
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Create a HostApi struct with callbacks pointing to host functions
    fn create_host_api(api: &mut EditorApiImpl) -> HostApi {
        HostApi {
            ctx: api as *mut EditorApiImpl as *mut c_void,
            log_info: host_log_info,
            log_warn: host_log_warn,
            log_error: host_log_error,
            set_status_item: host_set_status_item,
            remove_status_item: host_remove_status_item,
            undo: host_undo,
            redo: host_redo,
            can_undo: host_can_undo,
            can_redo: host_can_redo,
        }
    }

    /// Call on_load via FFI
    pub fn on_load(&mut self, api: &mut EditorApiImpl) -> Result<(), PluginError> {
        let host_api = Self::create_host_api(api);
        let host_api_ptr = &host_api as *const HostApi as *mut c_void;
        let result = unsafe { (self.vtable.on_load)(self.handle, host_api_ptr) };
        if result.success {
            Ok(())
        } else {
            let msg = unsafe { result.error_message.into_string() };
            Err(PluginError::InitFailed(msg))
        }
    }

    /// Call on_unload via FFI
    pub fn on_unload(&mut self, api: &mut EditorApiImpl) {
        let host_api = Self::create_host_api(api);
        let host_api_ptr = &host_api as *const HostApi as *mut c_void;
        unsafe { (self.vtable.on_unload)(self.handle, host_api_ptr) };
    }

    /// Call on_update via FFI
    pub fn on_update(&mut self, api: &mut EditorApiImpl, dt: f32) {
        let host_api = Self::create_host_api(api);
        let host_api_ptr = &host_api as *const HostApi as *mut c_void;
        unsafe { (self.vtable.on_update)(self.handle, host_api_ptr, dt) };
    }

    /// Call on_event via FFI (events passed as JSON)
    pub fn on_event(&mut self, api: &mut EditorApiImpl, _event: &EditorEvent) {
        let host_api = Self::create_host_api(api);
        let host_api_ptr = &host_api as *const HostApi as *mut c_void;
        // For now, pass null for event - event handling via FFI needs JSON serialization
        unsafe { (self.vtable.on_event)(self.handle, host_api_ptr, std::ptr::null()) };
    }
}

impl Drop for FfiPluginWrapper {
    fn drop(&mut self) {
        // Call destroy to properly deallocate the plugin
        unsafe { (self.vtable.destroy)(self.handle) };
    }
}

/// The plugin host manages the lifecycle of all loaded plugins.
#[derive(Resource)]
pub struct PluginHost {
    /// Directory to scan for plugins
    plugin_dir: PathBuf,
    /// Loaded plugin libraries (kept alive to prevent unloading)
    libraries: Vec<Library>,
    /// Plugin instances (FFI-safe wrappers)
    plugins: HashMap<String, FfiPluginWrapper>,
    /// Map from plugin ID to the file path it was loaded from
    plugin_paths: HashMap<String, PathBuf>,
    /// API implementation shared with plugins
    api: EditorApiImpl,
    /// Pending events to dispatch
    pending_events: Vec<EditorEvent>,
    /// File watcher for hot reload (wrapped in Mutex for Sync)
    watcher: Option<Mutex<RecommendedWatcher>>,
    /// Receiver for file system events (wrapped in Mutex for Sync)
    watcher_rx: Option<Mutex<Receiver<Result<Event, notify::Error>>>>,
}

impl Default for PluginHost {
    fn default() -> Self {
        Self::new()
    }
}

impl PluginHost {
    /// Create a new plugin host with the default plugins directory
    pub fn new() -> Self {
        let plugin_dir = std::env::current_dir()
            .unwrap_or_default()
            .join("plugins");

        Self {
            plugin_dir,
            libraries: Vec::new(),
            plugins: HashMap::new(),
            plugin_paths: HashMap::new(),
            api: EditorApiImpl::new(),
            pending_events: Vec::new(),
            watcher: None,
            watcher_rx: None,
        }
    }

    /// Create a plugin host with a custom plugin directory
    pub fn with_plugin_dir(plugin_dir: PathBuf) -> Self {
        Self {
            plugin_dir,
            ..Default::default()
        }
    }

    /// Start watching the plugin directory for changes
    pub fn start_watching(&mut self) {
        if self.watcher.is_some() {
            return; // Already watching
        }

        let (tx, rx) = channel();

        match RecommendedWatcher::new(
            move |res| {
                let _ = tx.send(res);
            },
            Config::default(),
        ) {
            Ok(mut watcher) => {
                if self.plugin_dir.exists() {
                    if let Err(e) = watcher.watch(&self.plugin_dir, RecursiveMode::NonRecursive) {
                        warn!("Failed to watch plugin directory: {}", e);
                        return;
                    }
                    info!("Watching plugin directory: {}", self.plugin_dir.display());
                    self.watcher = Some(Mutex::new(watcher));
                    self.watcher_rx = Some(Mutex::new(rx));
                }
            }
            Err(e) => {
                warn!("Failed to create file watcher: {}", e);
            }
        }
    }

    /// Stop watching the plugin directory
    pub fn stop_watching(&mut self) {
        self.watcher = None;
        self.watcher_rx = None;
    }

    /// Check for file system changes and hot reload plugins
    pub fn check_for_changes(&mut self) {
        let Some(rx_mutex) = &self.watcher_rx else {
            return;
        };

        let Ok(rx) = rx_mutex.lock() else {
            return;
        };

        let extension = if cfg!(windows) {
            "dll"
        } else if cfg!(target_os = "macos") {
            "dylib"
        } else {
            "so"
        };

        // Collect all events
        let mut created_files = Vec::new();
        let mut removed_files = Vec::new();

        while let Ok(result) = rx.try_recv() {
            if let Ok(event) = result {
                for path in event.paths {
                    if path.extension() != Some(OsStr::new(extension)) {
                        continue;
                    }

                    match event.kind {
                        notify::EventKind::Create(_) => {
                            created_files.push(path);
                        }
                        notify::EventKind::Remove(_) => {
                            removed_files.push(path);
                        }
                        notify::EventKind::Modify(_) => {
                            // For modifications, we'll treat it as remove + create
                            // But on Windows we can't reload while loaded, so just log
                            info!("Plugin modified: {} (restart to reload)", path.display());
                        }
                        _ => {}
                    }
                }
            }
        }

        // Drop the lock before modifying self
        drop(rx);

        // Handle removed plugins
        for path in removed_files {
            // Find plugin ID by path
            let plugin_id = self
                .plugin_paths
                .iter()
                .find(|(_, p)| **p == path)
                .map(|(id, _)| id.clone());

            if let Some(id) = plugin_id {
                info!("Plugin file removed, unloading: {}", id);
                let _ = self.unload_plugin(&id);
            }
        }

        // Handle new plugins
        for path in created_files {
            // Check if already loaded
            if self.plugin_paths.values().any(|p| *p == path) {
                continue;
            }

            info!("New plugin detected: {}", path.display());
            match self.load_plugin(&path) {
                Ok(id) => info!("Hot loaded plugin: {}", id),
                Err(e) => error!("Failed to hot load plugin: {}", e),
            }
        }
    }

    /// Get the plugin directory
    pub fn plugin_dir(&self) -> &PathBuf {
        &self.plugin_dir
    }

    /// Set the plugin directory
    pub fn set_plugin_dir(&mut self, dir: PathBuf) {
        self.plugin_dir = dir;
    }

    /// Discover available plugins in the plugin directory
    pub fn discover_plugins(&self) -> Vec<PathBuf> {
        let mut plugin_paths = Vec::new();

        let extension = if cfg!(windows) { "dll" } else if cfg!(target_os = "macos") { "dylib" } else { "so" };

        if let Ok(entries) = std::fs::read_dir(&self.plugin_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension() == Some(OsStr::new(extension)) {
                    plugin_paths.push(path);
                }
            }
        }

        plugin_paths
    }

    /// Probe a plugin library to get its manifest without fully loading it
    pub fn probe_plugin(&self, path: &PathBuf) -> Result<PluginManifest, PluginError> {
        let file_name = path.file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "unknown".to_string());

        // Try to load the library
        let library = match unsafe { Library::new(path) } {
            Ok(lib) => lib,
            Err(e) => {
                let err_msg = format!("Failed to load DLL: {}", e);
                console_log(LogLevel::Error, "Plugins", format!("[{}] {}", file_name, err_msg));
                return Err(PluginError::LoadFailed(err_msg));
            }
        };

        unsafe {
            // Try to get the create_plugin symbol
            let create_fn: libloading::Symbol<CreatePluginFn> = match library.get(b"create_plugin") {
                Ok(f) => f,
                Err(e) => {
                    let err_msg = format!("Not a valid plugin (missing create_plugin): {}", e);
                    console_log(LogLevel::Error, "Plugins", format!("[{}] {}", file_name, err_msg));
                    return Err(PluginError::LoadFailed(err_msg));
                }
            };

            // Call create_plugin with panic catching
            let export = match catch_unwind(AssertUnwindSafe(|| create_fn())) {
                Ok(exp) => exp,
                Err(_) => {
                    let err_msg = "Plugin crashed during initialization";
                    console_log(LogLevel::Error, "Plugins", format!("[{}] {}", file_name, err_msg));
                    return Err(PluginError::LoadFailed(err_msg.to_string()));
                }
            };

            // Check FFI version
            if export.ffi_version != FFI_API_VERSION {
                let err_msg = format!(
                    "FFI version mismatch: plugin uses v{}, editor expects v{}",
                    export.ffi_version, FFI_API_VERSION
                );
                console_log(LogLevel::Error, "Plugins", format!("[{}] {}", file_name, err_msg));
                (export.vtable.destroy)(export.handle);
                return Err(PluginError::ApiVersionMismatch {
                    required: export.ffi_version,
                    available: FFI_API_VERSION,
                });
            }

            // Get manifest with panic catching
            let manifest = match catch_unwind(AssertUnwindSafe(|| {
                let ffi_manifest = (export.vtable.manifest)(export.handle);
                ffi_manifest.into_manifest()
            })) {
                Ok(m) => m,
                Err(_) => {
                    let err_msg = "Plugin crashed while reading manifest";
                    console_log(LogLevel::Error, "Plugins", format!("[{}] {}", file_name, err_msg));
                    (export.vtable.destroy)(export.handle);
                    return Err(PluginError::LoadFailed(err_msg.to_string()));
                }
            };

            // Destroy the plugin (we're just probing)
            let _ = catch_unwind(AssertUnwindSafe(|| {
                (export.vtable.destroy)(export.handle);
            }));

            Ok(manifest)
        }
    }

    /// Discover and load all plugins in the plugin directory
    pub fn discover_and_load_plugins(&mut self) -> Result<(), PluginError> {
        console_log(LogLevel::Info, "Plugins", format!("Scanning: {}", self.plugin_dir.display()));

        // Ensure plugin directory exists
        if !self.plugin_dir.exists() {
            console_log(LogLevel::Info, "Plugins", "Creating plugins directory");
            std::fs::create_dir_all(&self.plugin_dir).ok();
            return Ok(());
        }

        // Discover plugin files
        let plugin_paths = self.discover_plugins();
        if plugin_paths.is_empty() {
            console_log(LogLevel::Info, "Plugins", "No plugins found");
            return Ok(());
        }

        console_log(LogLevel::Info, "Plugins", format!("Found {} DLL file(s)", plugin_paths.len()));

        // Probe all plugins to get manifests
        let mut manifests = Vec::new();
        let mut path_map = HashMap::new();
        let mut failed_count = 0;

        for path in &plugin_paths {
            match self.probe_plugin(path) {
                Ok(manifest) => {
                    console_log(LogLevel::Info, "Plugins",
                        format!("Detected: {} v{} ({})", manifest.name, manifest.version, manifest.id));
                    path_map.insert(manifest.id.clone(), path.clone());
                    manifests.push(manifest);
                }
                Err(_) => {
                    // Error already logged in probe_plugin
                    failed_count += 1;
                }
            }
        }

        if manifests.is_empty() {
            if failed_count > 0 {
                console_log(LogLevel::Warning, "Plugins",
                    format!("All {} plugin(s) failed to load", failed_count));
            }
            return Ok(());
        }

        // Build dependency graph and resolve load order
        let graph = DependencyGraph::from_manifests(&manifests);
        let load_order = match graph.topological_sort() {
            Ok(order) => order,
            Err(e) => {
                console_log(LogLevel::Error, "Plugins",
                    format!("Dependency resolution failed: {}", e));
                return Err(e);
            }
        };

        // Load plugins in dependency order
        let mut loaded_count = 0;
        for plugin_id in &load_order {
            if let Some(path) = path_map.get(plugin_id) {
                match self.load_plugin(path) {
                    Ok(_) => loaded_count += 1,
                    Err(_) => {
                        // Error already logged in load_plugin
                    }
                }
            }
        }

        if loaded_count > 0 {
            console_log(LogLevel::Success, "Plugins",
                format!("Successfully loaded {} plugin(s)", loaded_count));
        }
        if failed_count > 0 {
            console_log(LogLevel::Warning, "Plugins",
                format!("{} plugin(s) failed to load", failed_count));
        }

        // Start watching for hot reload
        self.start_watching();

        Ok(())
    }

    /// Load a single plugin from a library path
    pub fn load_plugin(&mut self, path: &PathBuf) -> Result<String, PluginError> {
        let file_name = path.file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "unknown".to_string());

        console_log(LogLevel::Info, "Plugins", format!("Loading: {}", file_name));

        // Load library
        let library = match unsafe { Library::new(path) } {
            Ok(lib) => lib,
            Err(e) => {
                let err_msg = format!("Failed to load DLL: {}", e);
                console_log(LogLevel::Error, "Plugins", format!("[{}] {}", file_name, err_msg));
                return Err(PluginError::LoadFailed(err_msg));
            }
        };

        unsafe {
            // Get create_plugin function
            let create_fn: libloading::Symbol<CreatePluginFn> = match library.get(b"create_plugin") {
                Ok(f) => f,
                Err(e) => {
                    let err_msg = format!("Invalid plugin (no create_plugin): {}", e);
                    console_log(LogLevel::Error, "Plugins", format!("[{}] {}", file_name, err_msg));
                    return Err(PluginError::LoadFailed(err_msg));
                }
            };

            // Create plugin with panic handling
            let export = match catch_unwind(AssertUnwindSafe(|| create_fn())) {
                Ok(exp) => exp,
                Err(_) => {
                    let err_msg = "Plugin crashed during creation";
                    console_log(LogLevel::Error, "Plugins", format!("[{}] {}", file_name, err_msg));
                    return Err(PluginError::LoadFailed(err_msg.to_string()));
                }
            };

            // Check FFI version
            if export.ffi_version != FFI_API_VERSION {
                let err_msg = format!(
                    "Incompatible FFI version (plugin: v{}, editor: v{})",
                    export.ffi_version, FFI_API_VERSION
                );
                console_log(LogLevel::Error, "Plugins", format!("[{}] {}", file_name, err_msg));
                let _ = catch_unwind(AssertUnwindSafe(|| (export.vtable.destroy)(export.handle)));
                return Err(PluginError::ApiVersionMismatch {
                    required: export.ffi_version,
                    available: FFI_API_VERSION,
                });
            }

            // Create the wrapper (this also gets the manifest)
            let mut wrapper = match catch_unwind(AssertUnwindSafe(|| FfiPluginWrapper::new(export))) {
                Ok(w) => w,
                Err(_) => {
                    let err_msg = "Plugin crashed while reading manifest";
                    console_log(LogLevel::Error, "Plugins", format!("[{}] {}", file_name, err_msg));
                    return Err(PluginError::LoadFailed(err_msg.to_string()));
                }
            };

            let manifest = wrapper.manifest().clone();

            // Check API version
            if manifest.min_api_version > EDITOR_API_VERSION {
                let err_msg = format!(
                    "Incompatible API version (requires: v{}, editor: v{})",
                    manifest.min_api_version, EDITOR_API_VERSION
                );
                console_log(LogLevel::Error, "Plugins", format!("[{}] {}", manifest.name, err_msg));
                return Err(PluginError::ApiVersionMismatch {
                    required: manifest.min_api_version,
                    available: EDITOR_API_VERSION,
                });
            }

            let plugin_id = manifest.id.clone();
            let plugin_name = manifest.name.clone();
            let plugin_version = manifest.version.clone();

            // Set current plugin for API tracking
            self.api.set_current_plugin(Some(plugin_id.clone()));

            // Initialize the plugin via FFI with panic handling
            let load_result = catch_unwind(AssertUnwindSafe(|| {
                wrapper.on_load(&mut self.api)
            }));

            self.api.set_current_plugin(None);

            match load_result {
                Ok(Ok(())) => {
                    // Success
                }
                Ok(Err(e)) => {
                    let err_msg = format!("Initialization failed: {}", e);
                    console_log(LogLevel::Error, "Plugins", format!("[{}] {}", plugin_name, err_msg));
                    return Err(e);
                }
                Err(_) => {
                    let err_msg = "Plugin crashed during initialization";
                    console_log(LogLevel::Error, "Plugins", format!("[{}] {}", plugin_name, err_msg));
                    return Err(PluginError::LoadFailed(err_msg.to_string()));
                }
            }

            // Store the library to keep it loaded
            self.libraries.push(library);

            // Store the plugin wrapper and path
            self.plugins.insert(plugin_id.clone(), wrapper);
            self.plugin_paths.insert(plugin_id.clone(), path.clone());

            console_log(LogLevel::Success, "Plugins",
                format!("Loaded: {} v{}", plugin_name, plugin_version));
            Ok(plugin_id)
        }
    }

    /// Unload a plugin by ID
    pub fn unload_plugin(&mut self, plugin_id: &str) -> Result<(), PluginError> {
        if let Some(mut wrapper) = self.plugins.remove(plugin_id) {
            // Set current plugin for API tracking
            self.api.set_current_plugin(Some(plugin_id.to_string()));
            wrapper.on_unload(&mut self.api);
            self.api.set_current_plugin(None);

            // Remove all UI elements registered by this plugin
            self.api.remove_plugin_elements(plugin_id);

            self.plugin_paths.remove(plugin_id);
            // wrapper is dropped here, which calls destroy via FFI
            info!("Plugin unloaded: {}", plugin_id);
            Ok(())
        } else {
            Err(PluginError::NotFound(plugin_id.to_string()))
        }
    }

    /// Unload all plugins (called when project changes)
    pub fn unload_all_plugins(&mut self) {
        // Stop watching first
        self.stop_watching();

        let plugin_ids: Vec<_> = self.plugins.keys().cloned().collect();
        for plugin_id in plugin_ids {
            if let Some(mut wrapper) = self.plugins.remove(&plugin_id) {
                wrapper.on_unload(&mut self.api);
                // wrapper is dropped here, which calls destroy via FFI
                info!("Plugin unloaded: {}", plugin_id);
            }
        }
        // Clear libraries to unload the DLLs
        self.libraries.clear();
        // Clear plugin paths
        self.plugin_paths.clear();
        // Clear API state
        self.api.clear();
        info!("All plugins unloaded");
    }

    /// Update all loaded plugins (called every frame)
    pub fn update(&mut self, dt: f32) {
        // Get plugin IDs for iteration (need to avoid borrow issues)
        let plugin_ids: Vec<_> = self.plugins.keys().cloned().collect();

        // Dispatch pending editor events
        let events: Vec<_> = self.pending_events.drain(..).collect();
        for event in &events {
            for plugin_id in &plugin_ids {
                if let Some(wrapper) = self.plugins.get_mut(plugin_id) {
                    if wrapper.is_enabled() {
                        self.api.set_current_plugin(Some(plugin_id.clone()));
                        wrapper.on_event(&mut self.api, event);
                    }
                }
            }
        }

        // Dispatch pending UI events (wrapped as EditorEvent::UiEvent)
        let ui_events: Vec<_> = self.api.pending_ui_events.drain(..).collect();
        for ui_event in ui_events {
            let event = EditorEvent::UiEvent(ui_event);
            for plugin_id in &plugin_ids {
                if let Some(wrapper) = self.plugins.get_mut(plugin_id) {
                    if wrapper.is_enabled() {
                        self.api.set_current_plugin(Some(plugin_id.clone()));
                        wrapper.on_event(&mut self.api, &event);
                    }
                }
            }
        }

        // Call update on all plugins
        for plugin_id in &plugin_ids {
            if let Some(wrapper) = self.plugins.get_mut(plugin_id) {
                if wrapper.is_enabled() {
                    self.api.set_current_plugin(Some(plugin_id.clone()));
                    wrapper.on_update(&mut self.api, dt);
                }
            }
        }

        self.api.set_current_plugin(None);
    }

    /// Update all plugins with direct World access (called every frame)
    /// Note: Direct World access is disabled in FFI mode for safety.
    /// Plugins should use the EditorApi for entity operations instead.
    pub fn update_with_world(&mut self, _world: &mut World) {
        // Direct World access is not safe across FFI boundaries
        // Plugins should use EditorApi methods instead
    }

    /// Queue an event to be dispatched to plugins
    pub fn queue_event(&mut self, event: EditorEvent) {
        self.pending_events.push(event);
    }

    /// Get the number of loaded plugins
    pub fn plugin_count(&self) -> usize {
        self.plugins.len()
    }

    /// Get list of loaded plugin manifests
    pub fn loaded_plugins(&self) -> Vec<PluginManifest> {
        self.plugins.values().map(|w| w.manifest().clone()).collect()
    }

    /// Enable or disable a plugin
    pub fn set_plugin_enabled(&mut self, plugin_id: &str, enabled: bool) -> Result<(), PluginError> {
        if let Some(wrapper) = self.plugins.get_mut(plugin_id) {
            wrapper.set_enabled(enabled);
            Ok(())
        } else {
            Err(PluginError::NotFound(plugin_id.to_string()))
        }
    }

    /// Check if a plugin is loaded
    pub fn is_plugin_loaded(&self, plugin_id: &str) -> bool {
        self.plugins.contains_key(plugin_id)
    }

    /// Get the API implementation (for internal use)
    pub fn api(&self) -> &EditorApiImpl {
        &self.api
    }

    /// Get mutable API implementation (for internal use)
    pub fn api_mut(&mut self) -> &mut EditorApiImpl {
        &mut self.api
    }
}
