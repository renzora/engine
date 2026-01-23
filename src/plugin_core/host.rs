//! Plugin host for discovering, loading, and managing plugins.

use std::collections::HashMap;
use std::ffi::OsStr;
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver};
use std::sync::Mutex;

use bevy::prelude::*;
use libloading::Library;
use notify::{Config, Event, RecommendedWatcher, RecursiveMode, Watcher};

use super::abi::{PluginError, PluginManifest, EDITOR_API_VERSION};
use super::api::EditorApiImpl;
use super::dependency::DependencyGraph;
use super::traits::{CreatePluginFn, EditorEvent, LoadedPluginWrapper};

/// The plugin host manages the lifecycle of all loaded plugins.
#[derive(Resource)]
pub struct PluginHost {
    /// Directory to scan for plugins
    plugin_dir: PathBuf,
    /// Loaded plugin libraries (kept alive to prevent unloading)
    libraries: Vec<Library>,
    /// Plugin instances
    plugins: HashMap<String, LoadedPluginWrapper>,
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
        unsafe {
            let library = Library::new(path)
                .map_err(|e| PluginError::LoadFailed(e.to_string()))?;

            let create_fn: libloading::Symbol<CreatePluginFn> = library
                .get(b"create_plugin")
                .map_err(|e| PluginError::LoadFailed(format!("Missing create_plugin symbol: {}", e)))?;

            let plugin_ptr = create_fn();
            let plugin = Box::from_raw(plugin_ptr);
            let manifest = plugin.manifest();

            // Plugin is dropped here, library will be unloaded
            Ok(manifest)
        }
    }

    /// Discover and load all plugins in the plugin directory
    pub fn discover_and_load_plugins(&mut self) -> Result<(), PluginError> {
        // Ensure plugin directory exists
        if !self.plugin_dir.exists() {
            info!("Creating plugins directory: {}", self.plugin_dir.display());
            std::fs::create_dir_all(&self.plugin_dir).ok();
            return Ok(());
        }

        // Discover plugin files
        let plugin_paths = self.discover_plugins();
        if plugin_paths.is_empty() {
            info!("No plugins found in {}", self.plugin_dir.display());
            return Ok(());
        }

        info!("Found {} plugin(s) in {}", plugin_paths.len(), self.plugin_dir.display());

        // Probe all plugins to get manifests
        let mut manifests = Vec::new();
        let mut path_map = HashMap::new();

        for path in &plugin_paths {
            match self.probe_plugin(path) {
                Ok(manifest) => {
                    info!("  Found plugin: {} v{}", manifest.name, manifest.version);
                    path_map.insert(manifest.id.clone(), path.clone());
                    manifests.push(manifest);
                }
                Err(e) => {
                    warn!("  Failed to probe {}: {}", path.display(), e);
                }
            }
        }

        // Build dependency graph and resolve load order
        let graph = DependencyGraph::from_manifests(&manifests);
        let load_order = graph.topological_sort()?;

        // Load plugins in dependency order
        for plugin_id in &load_order {
            if let Some(path) = path_map.get(plugin_id) {
                if let Err(e) = self.load_plugin(path) {
                    error!("Failed to load plugin {}: {}", plugin_id, e);
                }
            }
        }

        info!("Loaded {} plugin(s)", self.plugins.len());

        // Start watching for hot reload
        self.start_watching();

        Ok(())
    }

    /// Load a single plugin from a library path
    pub fn load_plugin(&mut self, path: &PathBuf) -> Result<String, PluginError> {
        info!("Loading plugin: {}", path.display());

        unsafe {
            let library = Library::new(path)
                .map_err(|e| PluginError::LoadFailed(e.to_string()))?;

            let create_fn: libloading::Symbol<CreatePluginFn> = library
                .get(b"create_plugin")
                .map_err(|e| PluginError::LoadFailed(format!("Missing create_plugin symbol: {}", e)))?;

            let plugin_ptr = create_fn();
            let mut plugin = Box::from_raw(plugin_ptr);
            let manifest = plugin.manifest();

            // Check API version
            if manifest.min_api_version > EDITOR_API_VERSION {
                return Err(PluginError::ApiVersionMismatch {
                    required: manifest.min_api_version,
                    available: EDITOR_API_VERSION,
                });
            }

            let plugin_id = manifest.id.clone();

            // Set current plugin for API tracking
            self.api.set_current_plugin(Some(plugin_id.clone()));

            // Initialize the plugin
            plugin.on_load(&mut self.api)
                .map_err(|e| PluginError::InitFailed(format!("{}", e)))?;

            self.api.set_current_plugin(None);

            // Store the library to keep it loaded
            self.libraries.push(library);

            // Store the plugin wrapper and path
            let wrapper = LoadedPluginWrapper::new(plugin);
            self.plugins.insert(plugin_id.clone(), wrapper);
            self.plugin_paths.insert(plugin_id.clone(), path.clone());

            info!("Plugin loaded: {} v{}", manifest.name, manifest.version);
            Ok(plugin_id)
        }
    }

    /// Unload a plugin by ID
    pub fn unload_plugin(&mut self, plugin_id: &str) -> Result<(), PluginError> {
        if let Some(mut wrapper) = self.plugins.remove(plugin_id) {
            // Set current plugin for API tracking
            self.api.set_current_plugin(Some(plugin_id.to_string()));
            wrapper.plugin_mut().on_unload(&mut self.api);
            self.api.set_current_plugin(None);

            // Remove all UI elements registered by this plugin
            self.api.remove_plugin_elements(plugin_id);

            self.plugin_paths.remove(plugin_id);
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
                wrapper.plugin_mut().on_unload(&mut self.api);
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
                        wrapper.plugin_mut().on_event(&mut self.api, event);
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
                        wrapper.plugin_mut().on_event(&mut self.api, &event);
                    }
                }
            }
        }

        // Call update on all plugins
        for plugin_id in &plugin_ids {
            if let Some(wrapper) = self.plugins.get_mut(plugin_id) {
                if wrapper.is_enabled() {
                    self.api.set_current_plugin(Some(plugin_id.clone()));
                    wrapper.plugin_mut().on_update(&mut self.api, dt);
                }
            }
        }

        self.api.set_current_plugin(None);
    }

    /// Update all plugins with direct World access (called every frame)
    /// This allows plugins to query components, spawn entities, use gizmos, etc.
    pub fn update_with_world(&mut self, world: &mut World) {
        for wrapper in self.plugins.values_mut() {
            if wrapper.is_enabled() {
                wrapper.plugin_mut().on_world_update(world);
            }
        }
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
    pub fn loaded_plugins(&self) -> Vec<&PluginManifest> {
        self.plugins.values().map(|w| w.manifest()).collect()
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
