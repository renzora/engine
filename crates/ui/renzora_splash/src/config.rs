use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Persisted update configuration
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct UpdateConfig {
    /// Whether to automatically check for updates on startup
    pub auto_check: bool,
    /// Version that the user has chosen to skip (won't be notified again)
    pub skipped_version: Option<String>,
}

impl Default for UpdateConfig {
    fn default() -> Self {
        Self {
            auto_check: true,
            skipped_version: None,
        }
    }
}

/// Application-wide configuration stored in user's config directory
#[derive(Resource, Serialize, Deserialize, Clone, Default)]
pub struct AppConfig {
    /// List of recently opened project paths
    pub recent_projects: Vec<PathBuf>,
    /// Update checker settings
    #[serde(default)]
    pub update_config: UpdateConfig,
    /// Plugin IDs the user has persistently disabled
    #[serde(default)]
    pub disabled_plugins: Vec<String>,
}

impl AppConfig {
    /// Get the path to the config file
    #[cfg(not(target_arch = "wasm32"))]
    pub fn config_path() -> Option<PathBuf> {
        dirs::config_dir().map(|p| p.join("bevy_editor").join("config.toml"))
    }

    /// Load configuration from disk or browser storage
    pub fn load() -> Self {
        #[cfg(not(target_arch = "wasm32"))]
        {
            Self::config_path()
                .and_then(|path| std::fs::read_to_string(&path).ok())
                .and_then(|content| toml::from_str(&content).ok())
                .unwrap_or_default()
        }
        #[cfg(target_arch = "wasm32")]
        {
            crate::web_storage::load_config()
        }
    }

    /// Save configuration to disk or browser storage
    pub fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let path = Self::config_path().ok_or("Could not determine config directory")?;
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let content = toml::to_string_pretty(self)?;
            std::fs::write(&path, content)?;
        }
        #[cfg(target_arch = "wasm32")]
        {
            crate::web_storage::save_config(self);
        }
        Ok(())
    }

    /// Add a project to recent projects list
    pub fn add_recent_project(&mut self, path: PathBuf) {
        self.recent_projects.retain(|p| p != &path);
        self.recent_projects.insert(0, path);
        self.recent_projects.truncate(10);
    }
}
