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
    /// Active locale code (e.g. "en", "fr"). Empty string means "en" (default).
    #[serde(default)]
    pub language: String,
    /// Persisted mixer volumes (master, sfx, music, ambient) as linear amplitude 0.0–1.5
    #[serde(default = "default_mixer_volumes")]
    pub mixer_volumes: MixerVolumes,
}

/// Persisted per-channel strip state
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ChannelStripConfig {
    #[serde(default = "default_volume")]
    pub volume: f64,
    #[serde(default)]
    pub panning: f64,
    #[serde(default)]
    pub muted: bool,
    #[serde(default)]
    pub soloed: bool,
}

fn default_volume() -> f64 { 1.0 }

impl Default for ChannelStripConfig {
    fn default() -> Self {
        Self { volume: 1.0, panning: 0.0, muted: false, soloed: false }
    }
}

/// Persisted mixer state (all buses + custom buses)
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct MixerVolumes {
    // Legacy fields kept for backwards compatibility with existing config files
    #[serde(default = "default_volume")]
    pub master: f64,
    #[serde(default = "default_volume")]
    pub sfx: f64,
    #[serde(default = "default_volume")]
    pub music: f64,
    #[serde(default = "default_volume")]
    pub ambient: f64,
    // Full channel strip state
    #[serde(default)]
    pub master_strip: ChannelStripConfig,
    #[serde(default)]
    pub sfx_strip: ChannelStripConfig,
    #[serde(default)]
    pub music_strip: ChannelStripConfig,
    #[serde(default)]
    pub ambient_strip: ChannelStripConfig,
    /// Custom user-created buses: (name, strip config)
    #[serde(default)]
    pub custom_buses: Vec<(String, ChannelStripConfig)>,
}

impl Default for MixerVolumes {
    fn default() -> Self {
        Self {
            master: 1.0, sfx: 1.0, music: 1.0, ambient: 1.0,
            master_strip: ChannelStripConfig::default(),
            sfx_strip: ChannelStripConfig::default(),
            music_strip: ChannelStripConfig::default(),
            ambient_strip: ChannelStripConfig::default(),
            custom_buses: Vec::new(),
        }
    }
}

fn default_mixer_volumes() -> MixerVolumes {
    MixerVolumes::default()
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

