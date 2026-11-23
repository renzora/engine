//! Whether this machine can compile Rust, and the request to make it so.
//!
//! A downloaded editor needs no Rust to run, but scripts and native plugins are
//! compiled on the machine that uses them, so a machine with none failed every
//! one with a wall of linker errors and nothing to do about it. The runtime side
//! (`renzora_rust_script`) checks when the editor starts, holds script builds
//! while the compiler is missing, and installs it when asked. The splash offers
//! the install up front, and the editor asks again if a project's scripts are
//! waiting on it.
//!
//! Here because those are three crates and none should link the others: the
//! script crate is runtime and ships in games, the splash and the prompt are UI.

use bevy::prelude::*;

/// What the editor knows about the compiler scripts and plugins need.
#[derive(Resource, Debug, Clone, Default, PartialEq, Eq)]
pub enum RustToolchain {
    /// Not checked yet. A game process never checks: it has nobody to ask.
    #[default]
    Unknown,
    /// The check is running.
    Checking,
    /// The pinned compiler is installed; builds go ahead.
    Ready,
    /// The pinned compiler is not installed.
    Missing {
        /// The release the SDK was cut with, e.g. `1.100.0`.
        version: String,
        /// rustup is present and only the toolchain is missing. Installing is
        /// then one `rustup toolchain install`, rather than putting Rust on a
        /// machine that has none.
        rustup_installed: bool,
        /// A project's scripts are being held until this is answered. The
        /// in-project prompt waits for this, so opening a project with no
        /// scripts does not ask about a compiler it will never use.
        scripts_waiting: bool,
    },
    /// The install the user agreed to is running.
    Installing { version: String },
    /// Installed during this session. Builds go ahead exactly as for
    /// [`Ready`](Self::Ready); kept apart so the splash can say it worked.
    Installed { version: String },
    /// The user said no. Builds go ahead and fail with the compiler error, which
    /// names the command to install it. The splash still offers the install.
    Declined,
    /// The install failed. Builds go ahead and fail the same way, and the splash
    /// offers to try again.
    Failed(String),
}

impl RustToolchain {
    /// Whether a build can expect to find the compiler.
    pub fn is_ready(&self) -> bool {
        matches!(self, RustToolchain::Ready | RustToolchain::Installed { .. })
    }
}

/// The user's answer to the install offer, from the splash or the prompt.
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub enum RustToolchainAnswer {
    Install,
    NotNow,
}
