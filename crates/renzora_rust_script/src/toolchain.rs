//! Making sure the compiler scripts and plugins need is on this machine, and
//! installing it when the user agrees.
//!
//! This module moves [`RustToolchain`]: it checks once when the editor starts,
//! publishes `Missing` for the splash and the in-project prompt to offer, and
//! runs the install the answer asks for. [`watch`](crate::watch) holds its
//! builds in an editor session until the state says a compiler is there or the
//! user has declined.
//!
//! # Why check at startup
//!
//! The splash is where someone new lands, and it is the one screen where an
//! install of a few minutes costs them nothing: no project is open and nothing
//! is waiting on it. Checking only once a script needed building meant the
//! question arrived in the middle of a project, or (for a marketplace plugin)
//! never arrived at all. The check spawns `rustup` off the main thread, so it
//! costs a launch nothing visible.
//!
//! # Why check before building rather than read the build's error
//!
//! A build with no compiler fails with whatever the toolchain lookup or the
//! linker said, once per script, and a project with ten scripts put ten walls of
//! text in the Jobs list. Holding the builds turns that into one question, and
//! the builds then run once against a compiler that is there.
//!
//! # Why only the editor
//!
//! Play's child and an exported game run the same watcher but have nobody to
//! ask. They never check, build straight away, and a missing compiler shows up
//! there as the build error, which names the command that fixes it.

use std::path::PathBuf;

use bevy::prelude::*;
use bevy::tasks::{block_on, poll_once, AsyncComputeTaskPool, Task};
use renzora_log_api::{console_error, console_info, console_success};
use renzora_platform_api::{RustToolchain, RustToolchainAnswer};
use renzora_plugin_build::Toolchain;

use crate::watch::ScriptWatcher;

/// The check and install running off the main thread. Both spawn processes,
/// and an install downloads hundreds of megabytes.
#[derive(Resource, Default)]
pub struct ToolchainTasks {
    check: Option<Task<RustToolchain>>,
    install: Option<Task<Result<(), String>>>,
    /// What the check found missing: the version and whether rustup is here.
    ///
    /// Kept apart from the state because `Declined` and `Failed` carry neither,
    /// and the splash still offers the install from both. Without this an
    /// Install pressed after "Not now" would have nothing to install.
    missing: Option<(String, bool)>,
}

/// Advance [`RustToolchain`]: check when the editor starts, keep the
/// scripts-waiting flag current, and act on the user's answer.
pub fn drive(
    mut state: ResMut<RustToolchain>,
    mut tasks: ResMut<ToolchainTasks>,
    mut answers: MessageReader<RustToolchainAnswer>,
    watcher: Res<ScriptWatcher>,
    session: Option<Res<renzora_project_api::EditorSession>>,
) {
    let editor = session.is_some_and(|s| s.is_editor());
    if *state == RustToolchain::Unknown && editor {
        if let Some(sdk_dir) = crate::sdk_dir() {
            tasks.check = Some(AsyncComputeTaskPool::get().spawn(async move { check(sdk_dir) }));
            *state = RustToolchain::Checking;
        } else {
            // No SDK beside the editor: nothing could build either way, and the
            // build reports that better than a toolchain offer would.
            *state = RustToolchain::Ready;
        }
    }

    if let Some(result) = tasks.check.as_mut().and_then(|t| block_on(poll_once(t))) {
        tasks.check = None;
        if let RustToolchain::Missing { version, rustup_installed, .. } = &result {
            tasks.missing = Some((version.clone(), *rustup_installed));
        }
        *state = result;
    }

    // Written only when it actually changes, so the splash and the prompt are
    // not woken every frame by a write of the same value.
    let held = watcher.has_held();
    if matches!(&*state, RustToolchain::Missing { scripts_waiting, .. } if *scripts_waiting != held) {
        if let RustToolchain::Missing { scripts_waiting, .. } = &mut *state {
            *scripts_waiting = held;
        }
    }

    for answer in answers.read() {
        match answer {
            RustToolchainAnswer::Install => {
                // Missing, or a "Not now" or failed install the user has come
                // back to. Not while one is already running.
                let offered = matches!(
                    *state,
                    RustToolchain::Missing { .. } | RustToolchain::Declined | RustToolchain::Failed(_)
                );
                let Some((version, rustup_installed)) = tasks.missing.clone().filter(|_| offered)
                else {
                    continue;
                };
                console_info("Rust", format!("Installing Rust {version}."));
                let v = version.clone();
                tasks.install = Some(
                    AsyncComputeTaskPool::get().spawn(async move { install(&v, rustup_installed) }),
                );
                *state = RustToolchain::Installing { version };
            }
            RustToolchainAnswer::NotNow => {
                if matches!(*state, RustToolchain::Missing { .. }) {
                    *state = RustToolchain::Declined;
                }
            }
        }
    }

    if let Some(result) = tasks.install.as_mut().and_then(|t| block_on(poll_once(t))) {
        tasks.install = None;
        match result {
            Ok(()) => {
                let version = tasks.missing.take().map(|(v, _)| v).unwrap_or_default();
                console_success("Rust", format!("Rust {version} installed"));
                *state = RustToolchain::Installed { version };
            }
            Err(e) => {
                console_error("Rust", format!("Installing Rust failed: {e}"));
                *state = RustToolchain::Failed(e);
            }
        }
    }
}

/// Resolve the compiler the SDK was cut with.
///
/// An SDK that will not load is reported as `Ready`: the build will fail on it
/// with a message about the SDK, which is the real problem, where a toolchain
/// offer would send the user to fix the wrong thing.
fn check(sdk_dir: PathBuf) -> RustToolchain {
    let Ok(sdk) = renzora_plugin_build::Sdk::load(sdk_dir) else {
        return RustToolchain::Ready;
    };
    let (version, rustup_installed) = match sdk.toolchain() {
        Toolchain::Ready(_) => return RustToolchain::Ready,
        Toolchain::ToolchainMissing { version } => (version, true),
        Toolchain::RustupMissing { version } => (version, false),
    };
    RustToolchain::Missing { version, rustup_installed, scripts_waiting: false }
}

/// Install the pinned toolchain, through rustup if it is here and through
/// rustup's own installer if it is not, then confirm the compiler resolves.
fn install(version: &str, rustup_installed: bool) -> Result<(), String> {
    if rustup_installed {
        renzora_plugin_build::toolchain::install_toolchain(version)?;
    } else {
        let url = renzora_plugin_build::toolchain::rustup_init_url()
            .ok_or("this platform has no Rust installer; see https://rustup.rs")?;
        let response = renzora_net_api::net::Request::get(&url)
            .header("User-Agent", "renzora-editor")
            .send()
            .map_err(|e| format!("could not download the Rust installer: {e}"))?;
        if !response.is_ok() {
            return Err(format!("could not download the Rust installer: HTTP {}", response.status));
        }
        let name = if cfg!(windows) { "renzora-rustup-init.exe" } else { "renzora-rustup-init" };
        let init = std::env::temp_dir().join(name);
        std::fs::write(&init, &response.body)
            .map_err(|e| format!("could not save the Rust installer: {e}"))?;
        let result = renzora_plugin_build::toolchain::run_rustup_init(&init, version);
        let _ = std::fs::remove_file(&init);
        result?;
    }
    match renzora_plugin_build::toolchain::resolve(version) {
        Toolchain::Ready(_) => Ok(()),
        _ => Err(format!("the installer finished but Rust {version} still cannot be found")),
    }
}
