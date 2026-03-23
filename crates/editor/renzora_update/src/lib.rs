//! Auto-update system for Renzora Editor
//!
//! Checks GitHub releases for new versions, downloads updates with progress
//! tracking, and launches an external updater for exe replacement.
//!
//! Disabled on WASM (no filesystem, no HTTP client, no process spawning).

#[cfg(not(target_arch = "wasm32"))]
mod check;
#[cfg(not(target_arch = "wasm32"))]
mod download;
#[cfg(not(target_arch = "wasm32"))]
mod ui;

use bevy::prelude::*;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, atomic::{AtomicBool, AtomicU64, Ordering}};

#[cfg(not(target_arch = "wasm32"))]
use renzora_splash::SplashState;

#[cfg(not(target_arch = "wasm32"))]
pub use check::UpdateCheckResult;

/// The current editor version — update this before each release to match the git tag.
pub const EDITOR_VERSION: &str = "r1-alpha4";

/// Get the current editor version.
pub fn current_version() -> &'static str {
    EDITOR_VERSION
}

/// Thread-safe wrapper for mpsc::Receiver
pub(crate) struct SyncReceiver<T>(Mutex<Option<std::sync::mpsc::Receiver<T>>>);

impl<T> SyncReceiver<T> {
    #[cfg(not(target_arch = "wasm32"))]
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
}

/// State for the update dialog UI.
#[derive(Resource, Default)]
pub struct UpdateDialogState {
    pub open: bool,
}

/// Runtime state for the update system.
#[derive(Resource)]
pub struct UpdateState {
    #[cfg(not(target_arch = "wasm32"))]
    pub check_result: Option<UpdateCheckResult>,
    pub checking: bool,
    pub download_progress: Option<f32>,
    pub downloaded_path: Option<PathBuf>,
    pub downloading: bool,
    pub error: Option<String>,
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) download_progress_shared: Option<Arc<AtomicU64>>,
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) download_total_shared: Option<Arc<AtomicU64>>,
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) download_complete: Option<Arc<AtomicBool>>,
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) download_error: Option<Arc<Mutex<Option<String>>>>,
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) check_receiver: Option<Arc<SyncReceiver<Result<UpdateCheckResult, String>>>>,
}

impl Default for UpdateState {
    fn default() -> Self {
        Self {
            #[cfg(not(target_arch = "wasm32"))]
            check_result: None,
            checking: false,
            download_progress: None,
            downloaded_path: None,
            downloading: false,
            error: None,
            #[cfg(not(target_arch = "wasm32"))]
            download_progress_shared: None,
            #[cfg(not(target_arch = "wasm32"))]
            download_total_shared: None,
            #[cfg(not(target_arch = "wasm32"))]
            download_complete: None,
            #[cfg(not(target_arch = "wasm32"))]
            download_error: None,
            #[cfg(not(target_arch = "wasm32"))]
            check_receiver: None,
        }
    }
}

impl UpdateState {
    #[cfg(not(target_arch = "wasm32"))]
    pub fn start_check(&mut self) {
        check::start_update_check(self);
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn start_download(&mut self) {
        let download_info = self.check_result.as_ref().and_then(|result| {
            let url = result.download_url.clone()?;
            let version = result.latest_version.clone()?;
            Some((url, version))
        });

        if let Some((url, version)) = download_info {
            download::start_download(self, &url, &version);
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn install_and_restart(&self) -> Result<(), String> {
        if let Some(ref downloaded_path) = self.downloaded_path {
            download::launch_updater(downloaded_path)
        } else {
            Err("No downloaded update available".to_string())
        }
    }

    pub fn clear_error(&mut self) {
        self.error = None;
    }
}

/// Plugin that provides auto-update functionality.
pub struct UpdatePlugin;

impl Plugin for UpdatePlugin {
    fn build(&self, app: &mut App) {
        info!("[editor] UpdatePlugin");
        app.init_resource::<UpdateState>()
            .init_resource::<UpdateDialogState>();

        #[cfg(not(target_arch = "wasm32"))]
        {
            use bevy_egui::EguiPrimaryContextPass;

            app.add_systems(Startup, trigger_startup_check)
                .add_systems(Update, poll_update_check.run_if(in_state(SplashState::Editor)))
                .add_systems(Update, poll_download_progress.run_if(in_state(SplashState::Editor)))
                .add_systems(
                    EguiPrimaryContextPass,
                    ui::render_update_dialog.run_if(in_state(SplashState::Editor)),
                );
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn trigger_startup_check(
    mut update_state: ResMut<UpdateState>,
    app_config: Res<renzora_splash::AppConfig>,
) {
    if app_config.update_config.auto_check {
        check::start_update_check(&mut update_state);
    }
}

#[cfg(not(target_arch = "wasm32"))]
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
                Err(std::sync::mpsc::TryRecvError::Empty) => {}
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    update_state.checking = false;
                    update_state.error = Some("Update check thread disconnected".to_string());
                    update_state.check_receiver = None;
                }
            }
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn poll_download_progress(mut update_state: ResMut<UpdateState>) {
    if !update_state.downloading {
        return;
    }

    let complete = update_state.download_complete.clone();
    let error_mutex = update_state.download_error.clone();
    let progress_shared = update_state.download_progress_shared.clone();
    let total_shared = update_state.download_total_shared.clone();

    if let Some(complete) = complete {
        if complete.load(Ordering::SeqCst) {
            update_state.downloading = false;
            update_state.download_progress = Some(1.0);

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

    if let (Some(progress), Some(total)) = (progress_shared, total_shared) {
        let downloaded = progress.load(Ordering::SeqCst);
        let total_size = total.load(Ordering::SeqCst);
        if total_size > 0 {
            update_state.download_progress = Some(downloaded as f32 / total_size as f32);
        }
    }
}
