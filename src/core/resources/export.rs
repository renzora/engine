//! Export dialog state resource

use bevy::prelude::*;
use std::path::PathBuf;

use crate::export::{ExportTarget, BuildType};

/// State for the export dialog
#[derive(Resource)]
pub struct ExportState {
    /// Whether the export dialog is open
    pub show_dialog: bool,
    /// Game name (window title)
    pub game_name: String,
    /// Selected target platforms
    pub target_windows: bool,
    pub target_linux: bool,
    pub target_macos_intel: bool,
    pub target_macos_arm: bool,
    /// Build type (debug/release)
    pub build_release: bool,
    /// Copy all assets or just referenced ones
    pub copy_all_assets: bool,
    /// Output directory
    pub output_dir: PathBuf,
    /// Export progress (0.0 to 1.0, negative means not exporting)
    pub progress: f32,
    /// Export status message
    pub status_message: String,
    /// Whether an export is currently in progress
    pub exporting: bool,
    /// Export errors
    pub errors: Vec<String>,
    /// Cached: whether targets have been checked
    pub targets_checked: bool,
    /// Cached: Windows target installed
    pub windows_installed: bool,
    /// Cached: Linux target installed
    pub linux_installed: bool,
    /// Cached: macOS Intel target installed
    pub macos_installed: bool,
    /// Cached: macOS ARM target installed
    pub macos_arm_installed: bool,
}

impl Default for ExportState {
    fn default() -> Self {
        Self {
            show_dialog: false,
            game_name: "My Game".to_string(),
            target_windows: true,
            target_linux: false,
            target_macos_intel: false,
            target_macos_arm: false,
            build_release: true,
            copy_all_assets: true,
            output_dir: PathBuf::from("export"),
            progress: -1.0,
            status_message: String::new(),
            exporting: false,
            errors: Vec::new(),
            targets_checked: false,
            windows_installed: false,
            linux_installed: false,
            macos_installed: false,
            macos_arm_installed: false,
        }
    }
}

impl ExportState {
    /// Get list of selected export targets
    pub fn selected_targets(&self) -> Vec<ExportTarget> {
        let mut targets = Vec::new();
        if self.target_windows {
            targets.push(ExportTarget::Windows);
        }
        if self.target_linux {
            targets.push(ExportTarget::Linux);
        }
        if self.target_macos_intel {
            targets.push(ExportTarget::MacOS);
        }
        if self.target_macos_arm {
            targets.push(ExportTarget::MacOSArm);
        }
        targets
    }

    /// Get the build type
    pub fn build_type(&self) -> BuildType {
        if self.build_release {
            BuildType::Release
        } else {
            BuildType::Debug
        }
    }
}
