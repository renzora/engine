use bevy::prelude::*;
use bevy::asset::UntypedAssetId;
use std::collections::HashMap;
use std::path::PathBuf;

/// Application state for managing splash screen vs editor
#[derive(States, Debug, Clone, PartialEq, Eq, Hash, Default)]
pub enum AppState {
    #[default]
    Splash,
    Editor,
    Runtime,
}

/// Configuration for runtime (--play) mode
#[derive(Resource)]
pub struct RuntimeConfig {
    pub project_path: PathBuf,
}

/// Information about an asset being tracked
#[derive(Clone)]
pub struct TrackedAsset {
    pub size_bytes: u64,
}

/// Resource to track asset loading progress
#[derive(Resource, Default)]
pub struct AssetLoadingProgress {
    pub loading: bool,
    pub loaded: usize,
    pub total: usize,
    /// Total bytes of all tracked assets
    pub total_bytes: u64,
    /// Bytes loaded so far
    pub loaded_bytes: u64,
    /// Asset IDs currently being tracked with their info
    pub(crate) tracking: HashMap<UntypedAssetId, TrackedAsset>,
}

impl AssetLoadingProgress {
    /// Start tracking an asset for loading progress
    pub fn track<T: Asset>(&mut self, handle: &Handle<T>, size_bytes: u64) {
        let info = TrackedAsset { size_bytes };
        self.tracking.insert(handle.id().untyped(), info);
        self.total = self.tracking.len();
        self.total_bytes = self.tracking.values().map(|a| a.size_bytes).sum();
        self.loading = true;
    }
}

/// Format bytes into human-readable string
pub fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}
