//! Plugin descriptor + registry resource.
//!
//! A descriptor is what we know about a plugin *before* loading it. Today
//! that is: the file path of its `.clap` bundle, the file size, and a derived
//! display name from the filename. Once `clack-host` integration lands we
//! will additionally populate vendor / version / category / parameter count
//! by reading the bundle's plugin factory.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use bevy::prelude::*;

/// Stable hash-friendly identifier for a plugin. Today this is just the
/// canonicalised file path of the `.clap` bundle. When we start reading
/// CLAP factories properly we can switch to a `(file_path, plugin_id)`
/// pair to support multi-plugin bundles.
#[derive(Clone, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct PluginId(pub String);

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct PluginDescriptor {
    pub id: PluginId,
    /// Display name. Filename stem until we can read the CLAP descriptor.
    pub name: String,
    /// Vendor / company. Empty until CLAP loading is wired up.
    pub vendor: String,
    /// Plugin version string.
    pub version: String,
    /// Free-text category. Empty by default.
    pub category: String,
    /// Absolute path to the `.clap` bundle / file on disk.
    pub bundle_path: PathBuf,
}

impl PluginDescriptor {
    /// Build a minimal descriptor from just a discovered `.clap` path.
    /// Used by the scanner when CLAP factory parsing is not available.
    pub fn from_path(path: PathBuf) -> Self {
        let name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("plugin")
            .to_string();
        let id = PluginId(path.to_string_lossy().to_string());
        Self {
            id,
            name,
            vendor: String::new(),
            version: String::new(),
            category: String::new(),
            bundle_path: path,
        }
    }
}

/// Resource holding all currently-known plugin descriptors plus a flag the
/// UI can use to show a "scanning..." indicator.
#[derive(Resource, Default)]
pub struct PluginRegistry {
    pub plugins: Vec<PluginDescriptor>,
    /// Set to true while a background scan is running. Shared with the
    /// worker via `Arc<AtomicBool>`.
    pub scan_in_progress: Arc<AtomicBool>,
    /// Path roots searched on the most recent scan, for display in the UI.
    pub last_scan_roots: Vec<PathBuf>,
}

impl PluginRegistry {
    /// Whether a scan is currently running (background thread alive).
    pub fn is_scanning(&self) -> bool {
        self.scan_in_progress.load(Ordering::Relaxed)
    }

    /// Look up a descriptor by id.
    pub fn get(&self, id: &PluginId) -> Option<&PluginDescriptor> {
        self.plugins.iter().find(|d| &d.id == id)
    }

    /// Replace the registry contents in one shot.
    pub fn set_all(&mut self, plugins: Vec<PluginDescriptor>, roots: Vec<PathBuf>) {
        self.plugins = plugins;
        self.plugins.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        self.last_scan_roots = roots;
    }
}
