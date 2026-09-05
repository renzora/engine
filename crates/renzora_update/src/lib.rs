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
//! ([`dialog`]) are new; the sidecar keeps the old one's structure and fixes its
//! macOS process-wait, which polled `/proc` on a system that has no `/proc`.
//!
//! # Channels
//!
//! `auto` (the default) follows the build: a nightly build is offered newer
//! nightlies, a released build is offered releases, and a build from source
//! tracks nightlies. `stable`/`nightly` override it. See
//! [`check::UpdateChannel`].
//!
//! **Nightlies are gated on developer mode.** With the toggle off every
//! preference resolves to `stable`, because a nightly is last night's `main` and
//! nobody who is just using the editor should be nudged onto one — least of all
//! by a chip in the top bar. The stored preference survives the gate, so turning
//! dev mode back on restores it.
//!
//! # Skipping a version
//!
//! "Skip This Version" records one tag in `~/.renzora/editor.toml` and silences
//! the top bar's chip (and the Help menu's "Update to …") for it. One tag, not a
//! list: skipping means "not this one", and the next release asks again. The
//! dialog still lists a skipped version — someone who opened the dialog is
//! asking — and downloading it clears the skip.
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
//!
//! # Install path
//!
//! The dialog exposes where the swap lands, pre-filled with the directory this
//! binary is running from — the detected default, and the right answer almost
//! always. Retargeting re-derives everything that depends on the path (which
//! binary to relaunch, and whether the destination is a checkout), so pointing
//! the install away from `dist/` genuinely drops the overwrite confirmation
//! rather than only moving the files.

#[cfg(not(target_arch = "wasm32"))]
mod check;
#[cfg(not(target_arch = "wasm32"))]
mod install;
#[cfg(not(target_arch = "wasm32"))]
mod dialog;
#[cfg(not(target_arch = "wasm32"))]
mod splash_page;
#[cfg(not(target_arch = "wasm32"))]
mod version;

use bevy::prelude::*;

#[cfg(not(target_arch = "wasm32"))]
pub use check::{ReleaseEntry, UpdateChannel, UpdateCheckResult};

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
    /// Mirror of [`renzora::core::DevMode`]. Nightlies are only offered while
    /// this is on; see [`UpdateChannel::resolve`].
    pub dev_mode: bool,
    /// The release tag the user asked not to be told about again, if any. Only
    /// suppresses the top bar's chip — the dialog still lists it, because
    /// someone who opened the dialog is asking.
    pub skipped: Option<String>,
    /// Where the install goes.
    ///
    /// Empty means "the detected default", which is the directory this binary is
    /// running from — the field is pre-filled with it, so the string is only
    /// ever different from the default because someone typed something else.
    pub install_path: String,
    pub layout: Option<install::InstallLayout>,
    /// True once the check has been kicked off at least once, so opening the
    /// dialog doesn't start a second one over the top of the first.
    checked_once: bool,
    /// Which version the action button targets.
    ///
    /// `None` means "the newest one the channel offers", which is what you want
    /// almost always. Set by picking a row in the version list, including an
    /// older one — going back a version is a legitimate thing to want and the
    /// check already has every entry in hand.
    pub selected_tag: Option<String>,
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
        let channel = UpdateChannel::resolve(&self.channel_pref, self.dev_mode);
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
        self.selected_tag = None;
        self.start_check();
    }

    /// Do we know where this engine is installed, i.e. is an install possible at
    /// all? False only when the layout could not be detected.
    pub fn can_install(&self) -> bool {
        self.layout.is_some()
    }

    /// The detected install location — what the path field defaults to, and what
    /// it falls back to when left empty.
    pub fn default_install_path(&self) -> String {
        self.layout
            .as_ref()
            .map(|l| l.target.display().to_string())
            .unwrap_or_default()
    }

    /// The layout the action button actually acts on: the detected one, aimed at
    /// whatever the install-path field says.
    pub fn effective_layout(&self) -> Option<install::InstallLayout> {
        let base = self.layout.as_ref()?;
        let typed = self.install_path.trim();
        if typed.is_empty() {
            return Some(base.clone());
        }
        let path = std::path::PathBuf::from(typed);
        if path == base.target {
            return Some(base.clone());
        }
        Some(base.retargeted(path))
    }

    /// Would installing overwrite a source checkout's `dist/`?
    ///
    /// Not a veto — installing there is allowed, but it overwrites build output,
    /// so the UI makes you say so twice. Asked of the *effective* layout, so
    /// retargeting the install away from the checkout also drops the extra
    /// confirmation instead of nagging about a directory nothing will touch.
    pub fn is_source_checkout(&self) -> bool {
        self.effective_layout()
            .is_some_and(|l| l.is_source_checkout)
    }

    /// Stop offering the version currently on the table.
    ///
    /// Persisted, and only ever one tag: skipping is "stop nagging me about
    /// *this one*", so the next release asks again. Returns the tag so the
    /// caller can drop [`renzora::core::UpdateAvailable`] with it.
    pub fn skip_target(&mut self) -> Option<String> {
        let tag = self
            .result
            .as_ref()
            .and_then(|r| r.latest_version.clone())?;
        let _ = renzora::core::save_skipped_update(Some(&tag));
        self.skipped = Some(tag.clone());
        Some(tag)
    }

    /// Is the newest version on offer one the user already waved away?
    pub fn target_is_skipped(&self) -> bool {
        match (self.skipped.as_deref(), self.result.as_ref()) {
            (Some(skipped), Some(r)) => r.latest_version.as_deref() == Some(skipped),
            _ => false,
        }
    }

    pub fn downloading(&self) -> bool {
        self.download.is_some()
    }

    /// The release the action button acts on: the explicit pick, else the newest.
    pub fn target(&self) -> Option<&ReleaseEntry> {
        let result = self.result.as_ref()?;
        match self.selected_tag.as_deref() {
            Some(tag) => result.entry(tag),
            None => result.releases.first(),
        }
    }

    /// Pick a version from the list. Clears anything staged for the previous
    /// pick — that download is for a different tag and must not be installed
    /// under this one.
    pub fn select(&mut self, tag: &str) {
        if self.selected_tag.as_deref() == Some(tag) {
            return;
        }
        self.selected_tag = Some(tag.to_string());
        self.staged = None;
        self.progress = None;
        self.overwrite_armed = false;
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
                .add_systems(
                    Update,
                    (
                        watch_dev_mode,
                        open_on_request,
                        poll_check,
                        poll_download,
                    ),
                );
            dialog::register(_app);
            // The same updater as a page on the splash dashboard. Registered
            // here rather than in `renzora_splash` because everything it needs
            // is in this crate; see the `splash_page` module doc.
            splash_page::register(_app);
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
    state.skipped = renzora::core::load_skipped_update();
    // Read straight off disk rather than from `renzora::core::DevMode`: this is
    // Startup, and the framework's mirror system has not run a frame yet.
    // `watch_dev_mode` takes over from here.
    state.dev_mode = renzora::load_dev_mode();
    match install::detect_layout() {
        Ok(layout) => {
            state.install_path = layout.target.display().to_string();
            state.layout = Some(layout);
            state.start_check();
        }
        Err(e) => state.error = Some(e),
    }
}

/// Follow the developer-mode toggle: what the channel resolves to depends on it,
/// so a stale result would keep offering (or keep hiding) nightlies after the
/// switch flipped.
///
/// `UpdateAvailable` is dropped straight away rather than waiting for the new
/// check to come back — the point of switching dev mode off is to stop being
/// told about nightlies, and a chip that lingers for a network round-trip after
/// the toggle reads as the toggle not working.
#[cfg(not(target_arch = "wasm32"))]
fn watch_dev_mode(
    dev: Option<Res<renzora::core::DevMode>>,
    mut state: ResMut<UpdateState>,
    mut commands: Commands,
) {
    let Some(dev) = dev else { return };
    if !dev.is_changed() || state.dev_mode == dev.0 {
        return;
    }
    state.dev_mode = dev.0;
    state.result = None;
    state.staged = None;
    state.progress = None;
    state.selected_tag = None;
    state.overwrite_armed = false;
    commands.remove_resource::<renzora::core::UpdateAvailable>();
    state.start_check();
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
                    // A skipped tag still shows up in the dialog's list — the
                    // user asked to stop being *told*, not to be prevented from
                    // installing it later. Only the chip and the Help menu item
                    // go quiet.
                    if state.skipped.as_deref() == Some(tag.as_str()) {
                        commands.remove_resource::<renzora::core::UpdateAvailable>();
                    } else {
                        commands.insert_resource(renzora::core::UpdateAvailable(tag));
                    }
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
    let (Some(entry), Some(layout)) = (state.target().cloned(), state.effective_layout()) else {
        return;
    };
    // Choosing to download a version is choosing to hear about it again: the
    // skip only ever meant "not this one", and leaving it set would mute the
    // chip for the very release about to be installed.
    if state.skipped.as_deref() == Some(entry.tag.as_str()) {
        state.skipped = None;
        let _ = renzora::core::save_skipped_update(None);
    }
    match install::spawn_download(&entry, &layout) {
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
    let (Some(staged), Some(layout)) = (state.staged.clone(), state.effective_layout()) else {
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
