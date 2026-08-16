//! Software updates for the editor.
//!
//! Checks GitHub for a newer engine, downloads the `<platform>.zip` for this
//! host, and hands the swap to the `renzora-update` sidecar (`tools/updater`),
//! because a running executable cannot replace itself.
//!
//! # What this replaced
//!
//! An updater existed before and was removed in the alpha-5 restructure. Its
//! version comparator was worth keeping and is recovered in [`version`], tests
//! included. The rest could not be: it replaced a **single `.exe`**, and an
//! install stopped being one file when the editor and runtime split into two
//! binaries with `plugins/` beside them — never mind that Linux ships one
//! `.AppImage` and macOS one `.app`. Its UI was also egui, and the editor is
//! bevy_ui now. So the shape of the swap ([`install`]) and the whole dialog
//! ([`native`]) are new; the sidecar keeps the old one's structure and fixes its
//! macOS process-wait, which polled `/proc` on a system that has no `/proc`.
//!
//! # Channels
//!
//! `auto` (the default) follows the build: a nightly build is offered newer
//! nightlies, a released build is offered releases, and a build from source
//! tracks nightlies. `stable`/`nightly` override it. See
//! [`check::UpdateChannel`].
//!
//! # Running from a source checkout
//!
//! The editor then lives in `<checkout>/dist/<platform>/`, so installing a
//! release replaces the tree `cargo renzora` stages into — recoverable by
//! rebuilding, but never something to do on one stray click.
//! [`install::detect_layout`] notices, and the dialog makes you say it twice:
//! the action button reads "Overwrite dist/…", arms on the first click, and only
//! installs on the second, naming the exact directory in between. Downloading is
//! never gated — it writes to `~/.renzora/updates/`, not to the install.

#[cfg(not(target_arch = "wasm32"))]
mod check;
#[cfg(not(target_arch = "wasm32"))]
mod install;
#[cfg(not(target_arch = "wasm32"))]
mod native;
#[cfg(not(target_arch = "wasm32"))]
mod version;

use bevy::prelude::*;

#[cfg(not(target_arch = "wasm32"))]
pub use check::{UpdateChannel, UpdateCheckResult};

/// Everything the dialog and its workers share.
#[cfg(not(target_arch = "wasm32"))]
#[derive(Resource, Default)]
pub struct UpdateState {
    pub visible: bool,
    /// A check is in flight.
    pub checking: bool,
    pub result: Option<UpdateCheckResult>,
    pub error: Option<String>,
    /// Set once a download completes: what the sidecar should install.
    pub staged: Option<std::path::PathBuf>,
    /// Bytes so far / total, mirrored out of the worker for the progress bar.
    pub progress: Option<(u64, u64)>,
    /// The stored channel preference (`auto` / `stable` / `nightly`).
    pub channel_pref: String,
    pub layout: Option<install::InstallLayout>,
    /// True once the check has been kicked off at least once, so opening the
    /// dialog doesn't start a second one over the top of the first.
    checked_once: bool,
    /// Second click armed for the overwrite confirmation.
    ///
    /// Only ever consulted for a source checkout, where installing replaces the
    /// `dist/` tree `cargo renzora` stages into — recoverable by rebuilding, but
    /// never something to do on one stray click.
    pub overwrite_armed: bool,
    check_rx: Option<std::sync::Mutex<std::sync::mpsc::Receiver<Result<UpdateCheckResult, String>>>>,
    download: Option<install::DownloadHandle>,
}

#[cfg(not(target_arch = "wasm32"))]
impl UpdateState {
    /// Start a check on a worker thread, unless one is already running.
    pub fn start_check(&mut self) {
        if self.checking {
            return;
        }
        self.checking = true;
        self.checked_once = true;
        self.error = None;
        self.overwrite_armed = false;
        let channel = UpdateChannel::resolve(&self.channel_pref);
        self.check_rx = Some(std::sync::Mutex::new(check::spawn_check(channel)));
    }

    /// Switch channel and re-check — the answer is channel-dependent, so a
    /// stale result from the other channel would be actively misleading.
    pub fn set_channel(&mut self, pref: &str) {
        if self.channel_pref == pref {
            return;
        }
        self.channel_pref = pref.to_string();
        let _ = renzora::core::save_update_channel(pref);
        self.result = None;
        self.staged = None;
        self.progress = None;
        self.download = None;
        self.overwrite_armed = false;
        self.start_check();
    }

    /// Do we know where this engine is installed, i.e. is an install possible at
    /// all? False only when the layout could not be detected.
    pub fn can_install(&self) -> bool {
        self.layout.is_some()
    }

    /// Is the editor running out of a source checkout's `dist/`?
    ///
    /// Not a veto any more — installing here is allowed, but it overwrites build
    /// output, so the UI makes you say so twice.
    pub fn is_source_checkout(&self) -> bool {
        self.layout.as_ref().is_some_and(|l| l.is_source_checkout)
    }

    pub fn downloading(&self) -> bool {
        self.download.is_some()
    }
}

/// Checks for a newer engine, downloads it, and hands the swap to the sidecar.
#[derive(Default)]
pub struct UpdatePlugin;

impl Plugin for UpdatePlugin {
    fn build(&self, _app: &mut App) {
        info!("[editor] UpdatePlugin");
        #[cfg(not(target_arch = "wasm32"))]
        {
            _app.init_resource::<UpdateState>()
                .add_systems(Startup, load_prefs_and_check)
                .add_systems(Update, (open_on_request, poll_check, poll_download));
            native::register(_app);
        }
    }
}

/// Read the stored channel and do one check at startup.
///
/// The check is silent — it only inserts [`renzora::core::UpdateAvailable`], so
/// the Help menu can offer "Update to …" instead of "Check for Updates". Nothing
/// is downloaded and no window appears; an editor that interrupts you at launch
/// to talk about itself is worse than one that is out of date.
///
#[cfg(not(target_arch = "wasm32"))]
fn load_prefs_and_check(mut state: ResMut<UpdateState>) {
    state.channel_pref = renzora::core::load_update_channel();
    match install::detect_layout() {
        Ok(layout) => {
            state.layout = Some(layout);
            state.start_check();
        }
        Err(e) => state.error = Some(e),
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn open_on_request(
    marker: Option<Res<renzora::core::UpdateRequested>>,
    mut state: ResMut<UpdateState>,
    mut commands: Commands,
) {
    if marker.is_none() {
        return;
    }
    commands.remove_resource::<renzora::core::UpdateRequested>();
    state.visible = true;
    // Opening the dialog is also how you ask "is there anything?", so a checkout
    // (which skipped the startup check) still gets an answer here.
    if !state.checked_once {
        if state.layout.is_none() {
            match install::detect_layout() {
                Ok(l) => state.layout = Some(l),
                Err(e) => state.error = Some(e),
            }
        }
        state.start_check();
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn poll_check(mut state: ResMut<UpdateState>, mut commands: Commands) {
    let Some(rx) = state.check_rx.as_ref() else {
        return;
    };
    let received = rx.lock().ok().and_then(|r| r.try_recv().ok());
    let Some(received) = received else {
        return;
    };
    state.check_rx = None;
    state.checking = false;
    match received {
        Ok(result) => {
            if result.update_available {
                if let Some(tag) = result.latest_version.clone() {
                    commands.insert_resource(renzora::core::UpdateAvailable(tag));
                }
            } else {
                // A channel switch can make a previously-available update
                // disappear; the menu must follow.
                commands.remove_resource::<renzora::core::UpdateAvailable>();
            }
            state.result = Some(result);
            state.error = None;
        }
        Err(e) => state.error = Some(e),
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn poll_download(mut state: ResMut<UpdateState>) {
    use std::sync::atomic::Ordering;
    let Some(handle) = state.download.as_ref() else {
        return;
    };
    let got = handle.downloaded.load(Ordering::SeqCst);
    let total = handle.total;
    let finished = handle.done.load(Ordering::SeqCst);
    let outcome = finished
        .then(|| handle.outcome.lock().ok().and_then(|mut o| o.take()))
        .flatten();

    state.progress = Some((got, total));
    if !finished {
        return;
    }
    state.download = None;
    match outcome {
        Some(Ok(path)) => {
            state.staged = Some(path);
            state.error = None;
        }
        Some(Err(e)) => {
            state.error = Some(e);
            state.progress = None;
        }
        // Finished with no outcome recorded means the worker died before it
        // could write one — report rather than sit on a finished download that
        // never appears.
        None => {
            state.error = Some("The download stopped unexpectedly.".to_string());
            state.progress = None;
        }
    }
}

/// Begin downloading the update the last check found.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn start_download(state: &mut UpdateState) {
    let (Some(result), Some(layout)) = (state.result.clone(), state.layout.clone()) else {
        return;
    };
    match install::spawn_download(&result, &layout) {
        Ok(handle) => {
            state.progress = Some((0, handle.total));
            state.download = Some(handle);
            state.error = None;
        }
        Err(e) => state.error = Some(e),
    }
}

/// Hand off to the sidecar. Does not return if it succeeds.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn install_and_restart(state: &mut UpdateState) {
    let (Some(staged), Some(layout)) = (state.staged.clone(), state.layout.clone()) else {
        return;
    };
    if let Err(e) = install::launch_sidecar(&staged, &layout) {
        // Only reached on failure — a successful handoff exits the process.
        // Disarm so the next attempt has to be confirmed again.
        state.overwrite_armed = false;
        state.error = Some(e);
    }
}

renzora::add!(UpdatePlugin, Editor);
