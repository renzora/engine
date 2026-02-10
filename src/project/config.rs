use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::update::UpdateConfig;

/// Application-wide configuration stored in user's config directory
#[derive(Resource, Serialize, Deserialize, Clone, Default)]
pub struct AppConfig {
    /// List of recently opened project paths
    pub recent_projects: Vec<PathBuf>,
    /// Update configuration
    #[serde(default)]
    pub update_config: UpdateConfig,
    /// Plugin IDs that the user has disabled
    #[serde(default)]
    pub disabled_plugins: Vec<String>,
}

impl AppConfig {
    /// Get the path to the config file
    pub fn config_path() -> Option<PathBuf> {
        dirs::config_dir().map(|p| p.join("bevy_editor").join("config.toml"))
    }

    /// Load configuration from disk
    pub fn load() -> Self {
        Self::config_path()
            .and_then(|path| std::fs::read_to_string(&path).ok())
            .and_then(|content| toml::from_str(&content).ok())
            .unwrap_or_default()
    }

    /// Save configuration to disk
    pub fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
        let path = Self::config_path().ok_or("Could not determine config directory")?;

        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let content = toml::to_string_pretty(self)?;
        std::fs::write(&path, content)?;
        Ok(())
    }

    /// Add a project to recent projects list
    pub fn add_recent_project(&mut self, path: PathBuf) {
        // Remove if already exists (to move it to front)
        self.recent_projects.retain(|p| p != &path);

        // Add to front
        self.recent_projects.insert(0, path);

        // Keep only last 10 projects
        self.recent_projects.truncate(10);
    }
}

