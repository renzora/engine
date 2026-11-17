//! Making sure the compiler a script needs is on this machine, and installing it
//! when the user agrees.
//!
//! [`watch`](crate::watch) holds its builds in an editor session until
//! [`RustToolchain`] says what to do with them. This module moves that state:
//! it checks the first time a script is waiting, publishes `Missing` for the
//! editor to ask about, and runs the install the answer asks for.
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
//! ask. They build straight away, as before, and a missing compiler shows up
//! there as the build error, which names the command that fixes it.

use std::path::PathBuf;

use bevy::prelude::*;
use bevy::tasks::{block_on, poll_once, AsyncComputeTaskPool, Task};
use renzora::core::console_log::{console_error, console_info, console_success};
use renzora::{RustToolchain, RustToolchainAnswer};
use renzora_plugin_build::Toolchain;

use crate::watch::ScriptWatcher;

/// The check and install running off the main thread. Both spawn processes,
/// and an install downloads hundreds of megabytes.
#[derive(Resource, Default)]
pub struct ToolchainTasks {
    check: Option<Task<RustToolchain>>,
    install: Option<Task<Result<(), String>>>,
}

/// Advance [`RustToolchain`]: start the check when a script is waiting, take in
/// its answer, and act on the user's.
pub fn drive(
    mut state: ResMut<RustToolchain>,
    mut tasks: ResMut<ToolchainTasks>,
    mut answers: MessageReader<RustToolchainAnswer>,
    watcher: Res<ScriptWatcher>,
) {
    if *state == RustToolchain::Unknown && watcher.has_held() {
        if let Some(sdk_dir) = crate::sdk_dir() {
            tasks.check = Some(AsyncComputeTaskPool::get().spawn(async move { check(sdk_dir) }));
            state.set_if_neq(RustToolchain::Checking);
        } else {
            // No SDK beside the editor: nothing could build either way, and the
            // build reports that better than a toolchain prompt would.
            state.set_if_neq(RustToolchain::Ready);
        }
    }

    if let Some(result) = tasks.check.as_mut().and_then(|t| block_on(poll_once(t))) {
        tasks.check = None;
        *state = result;
    }

    for answer in answers.read() {
        let RustToolchain::Missing { version, rustup_installed } = state.clone() else {
            continue;
        };
        match answer {
            RustToolchainAnswer::Install => {
                console_info(
                    "Script",
                    format!("Installing Rust {version}. Scripts will compile when it finishes."),
                );
                let v = version.clone();
                tasks.install = Some(
                    AsyncComputeTaskPool::get().spawn(async move { install(&v, rustup_installed) }),
                );
                *state = RustToolchain::Installing { version };
            }
            RustToolchainAnswer::NotNow => *state = RustToolchain::Declined,
        }
    }

    if let Some(result) = tasks.install.as_mut().and_then(|t| block_on(poll_once(t))) {
        tasks.install = None;
        match result {
            Ok(()) => {
                console_success("Script", "Rust installed; compiling scripts");
                *state = RustToolchain::Ready;
            }
            Err(e) => {
                console_error("Script", format!("Installing Rust failed: {e}"));
                *state = RustToolchain::Failed(e);
            }
        }
    }
}

/// Resolve the compiler the SDK was cut with.
///
/// An SDK that will not load is reported as `Ready`: the build will fail on it
/// with a message about the SDK, which is the real problem, where a toolchain
/// prompt would send the user to fix the wrong thing.
fn check(sdk_dir: PathBuf) -> RustToolchain {
    let Ok(sdk) = renzora_plugin_build::Sdk::load(sdk_dir) else {
        return RustToolchain::Ready;
    };
    match sdk.toolchain() {
        Toolchain::Ready(_) => RustToolchain::Ready,
        Toolchain::ToolchainMissing { version } => {
            RustToolchain::Missing { version, rustup_installed: true }
        }
        Toolchain::RustupMissing { version } => {
            RustToolchain::Missing { version, rustup_installed: false }
        }
    }
}

/// Install the pinned toolchain, through rustup if it is here and through
/// rustup's own installer if it is not, then confirm the compiler resolves.
fn install(version: &str, rustup_installed: bool) -> Result<(), String> {
    if rustup_installed {
        renzora_plugin_build::toolchain::install_toolchain(version)?;
    } else {
        let url = renzora_plugin_build::toolchain::rustup_init_url()
            .ok_or("this platform has no Rust installer; see https://rustup.rs")?;
        let response = renzora::net::Request::get(&url)
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
