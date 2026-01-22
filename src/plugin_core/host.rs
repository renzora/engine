//! Plugin host for discovering, loading, and managing plugins.

use std::collections::HashMap;
use std::ffi::OsStr;
use std::path::PathBuf;

use bevy::prelude::*;
use libloading::Library;

use super::abi::{PluginError, PluginManifest, EDITOR_API_VERSION};
use super::api::EditorApiImpl;
use super::dependency::DependencyGraph;
use super::traits::{CreatePluginFn, EditorEvent, EditorPlugin, LoadedPluginWrapper};

/// The plugin host manages the lifecycle of all loaded plugins.
#[derive(Resource)]
pub struct PluginHost {
    /// Directory to scan for plugins
    plugin_dir: PathBuf,
    /// Loaded plugin libraries (kept alive to prevent unloading)
    libraries: Vec<Library>,
    /// Plugin instances
    plugins: HashMap<String, LoadedPluginWrapper>,
    /// API implementation shared with plugins
    api: EditorApiImpl,
    /// Pending events to dispatch
    pending_events: Vec<EditorEvent>,
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
            api: EditorApiImpl::new(),
            pending_events: Vec::new(),
        }
    }

    /// Create a plugin host with a custom plugin directory
    pub fn with_plugin_dir(plugin_dir: PathBuf) -> Self {
        Self {
            plugin_dir,
            ..Default::default()
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

            // Initialize the plugin
            plugin.on_load(&mut self.api)
                .map_err(|e| PluginError::InitFailed(format!("{}", e)))?;

            // Store the library to keep it loaded
            self.libraries.push(library);

            // Store the plugin wrapper
            let wrapper = LoadedPluginWrapper::new(plugin);
            self.plugins.insert(plugin_id.clone(), wrapper);

            info!("Plugin loaded: {} v{}", manifest.name, manifest.version);
            Ok(plugin_id)
        }
    }

    /// Unload a plugin by ID
    pub fn unload_plugin(&mut self, plugin_id: &str) -> Result<(), PluginError> {
        if let Some(mut wrapper) = self.plugins.remove(plugin_id) {
            wrapper.plugin_mut().on_unload(&mut self.api);
            info!("Plugin unloaded: {}", plugin_id);
            Ok(())
        } else {
            Err(PluginError::NotFound(plugin_id.to_string()))
        }
    }

    /// Update all loaded plugins (called every frame)
    pub fn update(&mut self, dt: f32) {
        // Dispatch pending events
        let events: Vec<_> = self.pending_events.drain(..).collect();
        for event in &events {
            for wrapper in self.plugins.values_mut() {
                if wrapper.is_enabled() {
                    wrapper.plugin_mut().on_event(&mut self.api, event);
                }
            }
        }

        // Call update on all plugins
        for wrapper in self.plugins.values_mut() {
            if wrapper.is_enabled() {
                wrapper.plugin_mut().on_update(&mut self.api, dt);
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
