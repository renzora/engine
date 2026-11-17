//! Whether this machine can compile Rust scripts, and the request to make it so.
//!
//! A downloaded editor needs no Rust to run, but a `.rs` script is compiled on
//! the machine that opens it, so a machine with none failed every script with a
//! wall of linker errors in the Jobs list and nothing to do about it. The runtime side
//! (`renzora_rust_script`) checks once, holds its builds while the compiler is
//! missing, and installs it when asked; the editor side asks.
//!
//! Here because those are two crates and neither should link the other: the
//! script crate is runtime and ships in games, the prompt is editor UI.

use bevy::prelude::*;

/// What the script builder knows about the compiler it needs.
#[derive(Resource, Debug, Clone, Default, PartialEq, Eq)]
pub enum RustToolchain {
    /// Not checked yet. Checking spawns `rustup`, so it waits until a script
    /// actually needs building rather than costing every project open.
    #[default]
    Unknown,
    /// The check is running.
    Checking,
    /// The pinned compiler is installed; builds go ahead.
    Ready,
    /// The pinned compiler is not installed, and builds are being held until
    /// the user answers.
    Missing {
        /// The release the SDK was cut with, e.g. `1.100.0`.
        version: String,
        /// rustup is present and only the toolchain is missing. Installing is
        /// then one `rustup toolchain install`, rather than putting Rust on a
        /// machine that has none.
        rustup_installed: bool,
    },
    /// The install the user agreed to is running.
    Installing { version: String },
    /// The user said no. Builds go ahead and fail with the compiler error, which
    /// names the command to install it; the prompt does not come back this
    /// session.
    Declined,
    /// The install failed. Builds go ahead and fail the same way.
    Failed(String),
}

/// The user's answer to the install prompt.
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub enum RustToolchainAnswer {
    Install,
    NotNow,
}
