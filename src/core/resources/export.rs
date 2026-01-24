//! Export dialog state resource

use bevy::prelude::*;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use crate::export::{BuildType, ExportTarget};

/// Export log entry level
#[derive(Clone, Copy, PartialEq)]
pub enum ExportLogLevel {
    Info,
    Success,
    Warning,
    Error,
}

/// A single export log entry
#[derive(Clone)]
pub struct ExportLogEntry {
    pub level: ExportLogLevel,
    pub message: String,
}

/// Shared export logger that can be passed to export functions
#[derive(Clone, Default)]
pub struct ExportLogger {
    logs: Arc<Mutex<Vec<ExportLogEntry>>>,
    progress: Arc<Mutex<(f32, String)>>, // (progress 0-1, current step)
}

impl ExportLogger {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn info(&self, msg: impl Into<String>) {
        let msg = msg.into();
        println!("[EXPORT] {}", msg);
        if let Ok(mut logs) = self.logs.lock() {
            logs.push(ExportLogEntry {
                level: ExportLogLevel::Info,
                message: msg,
            });
        }
    }

    pub fn success(&self, msg: impl Into<String>) {
        let msg = msg.into();
        println!("[EXPORT] ✓ {}", msg);
        if let Ok(mut logs) = self.logs.lock() {
            logs.push(ExportLogEntry {
                level: ExportLogLevel::Success,
                message: msg,
            });
        }
    }

    pub fn warning(&self, msg: impl Into<String>) {
        let msg = msg.into();
        println!("[EXPORT] ⚠ {}", msg);
        if let Ok(mut logs) = self.logs.lock() {
            logs.push(ExportLogEntry {
                level: ExportLogLevel::Warning,
                message: msg,
            });
        }
    }

    pub fn error(&self, msg: impl Into<String>) {
        let msg = msg.into();
        eprintln!("[EXPORT] ✗ {}", msg);
        if let Ok(mut logs) = self.logs.lock() {
            logs.push(ExportLogEntry {
                level: ExportLogLevel::Error,
                message: msg,
            });
        }
    }

    pub fn set_progress(&self, progress: f32, step: impl Into<String>) {
        if let Ok(mut p) = self.progress.lock() {
            *p = (progress, step.into());
        }
    }

    pub fn get_logs(&self) -> Vec<ExportLogEntry> {
        self.logs.lock().map(|l| l.clone()).unwrap_or_default()
    }

    pub fn get_progress(&self) -> (f32, String) {
        self.progress
            .lock()
            .map(|p| p.clone())
            .unwrap_or((0.0, String::new()))
    }

    pub fn clear(&self) {
        if let Ok(mut logs) = self.logs.lock() {
            logs.clear();
        }
        if let Ok(mut p) = self.progress.lock() {
            *p = (0.0, String::new());
        }
    }
}

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
    /// Export logger with log history
    pub logger: ExportLogger,
    /// Whether to show the console log
    pub show_console: bool,
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
            logger: ExportLogger::new(),
            show_console: true,
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
