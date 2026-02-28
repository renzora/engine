//! Auto-update system for Renzora Editor
//!
//! This module provides functionality to:
//! - Check GitHub releases for new versions
//! - Download updates with progress tracking
//! - Launch external updater for exe replacement

mod check;
mod download;
mod ui;

use bevy::prelude::*;
use bevy_egui::EguiPrimaryContextPass;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, atomic::{AtomicBool, AtomicU64, Ordering}};

use crate::core::AppState;

pub use check::UpdateCheckResult;
pub use ui::UpdateDialogState;

/// Thread-safe wrapper for mpsc::Receiver
pub(crate) struct SyncReceiver<T>(Mutex<Option<std::sync::mpsc::Receiver<T>>>);

impl<T> SyncReceiver<T> {
    fn new(receiver: std::sync::mpsc::Receiver<T>) -> Self {
        Self(Mutex::new(Some(receiver)))
    }

    fn try_recv(&self) -> Option<Result<T, std::sync::mpsc::TryRecvError>> {
        if let Ok(guard) = self.0.lock() {
            if let Some(ref receiver) = *guard {
                return Some(receiver.try_recv());
            }
        }
        None
    }

    fn take(&self) -> Option<std::sync::mpsc::Receiver<T>> {
        if let Ok(mut guard) = self.0.lock() {
            return guard.take();
        }
        None
    }
}

/// Plugin that provides auto-update functionality
pub struct UpdatePlugin;

impl Plugin for UpdatePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<UpdateState>()
            .init_resource::<UpdateDialogState>()
            .add_systems(Startup, trigger_startup_check)
            .add_systems(Update, poll_update_check.run_if(in_state(AppState::Editor)))
            .add_systems(Update, poll_download_progress.run_if(in_state(AppState::Editor)))
            .add_systems(
                EguiPrimaryContextPass,
                ui::render_update_dialog.run_if(in_state(AppState::Editor)),
            );
    }
}

/// Persisted update configuration (stored in AppConfig)
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

/// Runtime state for the update system
#[derive(Resource)]
pub struct UpdateState {
    /// Result of the version check (populated by background thread)
    pub check_result: Option<UpdateCheckResult>,
    /// Whether a check is currently in progress
    pub checking: bool,
    /// Download progress (0.0 to 1.0)
    pub download_progress: Option<f32>,
    /// Path to the downloaded binary
    pub downloaded_path: Option<PathBuf>,
    /// Whether a download is currently in progress
    pub downloading: bool,
    /// Error message if something went wrong
    pub error: Option<String>,
    /// Shared progress for background download thread
    pub(crate) download_progress_shared: Option<Arc<AtomicU64>>,
    /// Shared total size for background download thread
    pub(crate) download_total_shared: Option<Arc<AtomicU64>>,
    /// Shared completion flag for background download thread
    pub(crate) download_complete: Option<Arc<AtomicBool>>,
    /// Shared error for background download thread
    pub(crate) download_error: Option<Arc<Mutex<Option<String>>>>,
    /// Receiver for background check results (wrapped for Sync)
    pub(crate) check_receiver: Option<Arc<SyncReceiver<Result<UpdateCheckResult, String>>>>,
}

impl Default for UpdateState {
    fn default() -> Self {
        Self {
            check_result: None,
            checking: false,
            download_progress: None,
            downloaded_path: None,
            downloading: false,
            error: None,
            download_progress_shared: None,
            download_total_shared: None,
            download_complete: None,
            download_error: None,
            check_receiver: None,
        }
    }
}

/// The current editor version — update this before each release to match the git tag
pub const EDITOR_VERSION: &str = "r1-alpha4";

/// Get the current editor version
pub fn current_version() -> &'static str {
    EDITOR_VERSION
}

/// System to trigger update check on startup (if auto_check is enabled)
fn trigger_startup_check(
    mut update_state: ResMut<UpdateState>,
    app_config: Res<crate::project::AppConfig>,
) {
    if app_config.update_config.auto_check {
        check::start_update_check(&mut update_state);
    }
}

/// System to poll for update check results from background thread
fn poll_update_check(mut update_state: ResMut<UpdateState>) {
    let receiver = update_state.check_receiver.clone();
    if let Some(ref receiver) = receiver {
        if let Some(recv_result) = receiver.try_recv() {
            match recv_result {
                Ok(result) => {
                    update_state.checking = false;
                    match result {
                        Ok(check_result) => {
                            update_state.check_result = Some(check_result);
                            update_state.error = None;
                        }
                        Err(e) => {
                            update_state.error = Some(e);
                        }
                    }
                    update_state.check_receiver = None;
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {
                    // Still waiting
                }
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    update_state.checking = false;
                    update_state.error = Some("Update check thread disconnected".to_string());
                    update_state.check_receiver = None;
                }
            }
        }
    }
}

/// System to poll download progress from background thread
fn poll_download_progress(mut update_state: ResMut<UpdateState>) {
    if !update_state.downloading {
        return;
    }

    // Clone Arcs to avoid borrow issues
    let complete = update_state.download_complete.clone();
    let error_mutex = update_state.download_error.clone();
    let progress_shared = update_state.download_progress_shared.clone();
    let total_shared = update_state.download_total_shared.clone();

    // Check if download completed
    if let Some(complete) = complete {
        if complete.load(Ordering::SeqCst) {
            update_state.downloading = false;
            update_state.download_progress = Some(1.0);

            // Check for errors
            if let Some(error_mutex) = error_mutex {
                if let Ok(guard) = error_mutex.lock() {
                    if let Some(err) = guard.as_ref() {
                        update_state.error = Some(err.clone());
                        update_state.downloaded_path = None;
                    }
                }
            }
            return;
        }
    }

    // Update progress
    if let (Some(progress), Some(total)) = (progress_shared, total_shared) {
        let downloaded = progress.load(Ordering::SeqCst);
        let total_size = total.load(Ordering::SeqCst);
        if total_size > 0 {
            update_state.download_progress = Some(downloaded as f32 / total_size as f32);
        }
    }
}

impl UpdateState {
    /// Start checking for updates
    pub fn start_check(&mut self) {
        check::start_update_check(self);
    }

    /// Start downloading the update
    pub fn start_download(&mut self) {
        // Clone the needed data to avoid borrow issues
        let download_info = self.check_result.as_ref().and_then(|result| {
            let url = result.download_url.clone()?;
            let version = result.latest_version.clone()?;
            Some((url, version))
        });

        if let Some((url, version)) = download_info {
            download::start_download(self, &url, &version);
        }
    }

    /// Launch the updater and exit the editor
    pub fn install_and_restart(&self) -> Result<(), String> {
        if let Some(ref downloaded_path) = self.downloaded_path {
            download::launch_updater(downloaded_path)
        } else {
            Err("No downloaded update available".to_string())
        }
    }

    /// Clear error state
    pub fn clear_error(&mut self) {
        self.error = None;
    }
}
