//! Finding — and if the user agrees, installing — the exact `rustc` a plugin
//! build needs.
//!
//! # Why a bare `rustc` is not good enough
//!
//! Two reasons, and the second is the dangerous one.
//!
//! A downloaded editor runs on a machine that may have no Rust at all. The
//! editor itself needs none — it is an ordinary binary — but plugins are
//! compiled where they are installed, so the compiler has to come from
//! somewhere. That is a missing-tool problem, and it announces itself.
//!
//! The other is that a machine may have the *wrong* Rust. Crate metadata is
//! versioned and rustc refuses to read another version's, so a 1.93 compiler
//! against a 1.95 SDK stops at `error[E0514]: found crate 'bevy' compiled by an
//! incompatible version of rustc`. That message is accurate but arrives after
//! the user has downloaded ~444 MB and pressed Install. Resolving a *pinned*
//! toolchain by absolute path makes the mismatch unreachable rather than
//! diagnosable.
//!
//! # Why rustup rather than shipping a compiler
//!
//! Shipping one is possible — the minimum useful set is about 400 MB (the
//! `rustc_driver` dll, `rust-lld`, and the sysroot rlibs). But it means
//! redistributing rustc, hosting it per platform per release, and re-publishing
//! whenever the pin moves. Driving rustup instead costs the same bytes on the
//! user's disk, fetches them from Rust's own CDN, and names the version
//! explicitly at both install and invoke.
//!
//! `rustup which --toolchain <version> rustc` returns an absolute path, so the
//! build never depends on `PATH` and cannot pick up whatever else is installed.

use std::path::PathBuf;
use std::process::Command;

/// What the machine can currently do about building a plugin.
#[derive(Debug, Clone)]
pub enum Toolchain {
    /// Ready. The absolute path to the pinned compiler.
    Ready(PathBuf),
    /// rustup is here but the pinned toolchain is not installed.
    ///
    /// Recoverable without a download prompt beyond rustup's own: see
    /// [`install_toolchain`].
    ToolchainMissing { version: String },
    /// No rustup, and no `rustc` on `PATH` of the right version.
    ///
    /// The only state that needs the user to agree to install software.
    RustupMissing { version: String },
}

/// Locate the compiler for `version`, e.g. `"1.95.0"`.
///
/// Tries rustup first because it is the only source that can be *asked* for a
/// specific version. A bare `rustc` is accepted as a fallback, but only after
/// checking that it is the right one — a machine with the wrong compiler on
/// `PATH` is worse than a machine with none, because it gets further before
/// failing.
pub fn resolve(version: &str) -> Toolchain {
    if let Some(path) = rustup_which(version) {
        return Toolchain::Ready(path);
    }
    if path_rustc_release().as_deref() == Some(version) {
        // No rustup, but the compiler on PATH happens to be the pinned one.
        // Common for someone who installed Rust another way.
        return Toolchain::Ready(renzora_native_build::tool("rustc"));
    }
    if have_rustup() {
        Toolchain::ToolchainMissing { version: version.to_string() }
    } else {
        Toolchain::RustupMissing { version: version.to_string() }
    }
}

/// Ask rustup for the absolute path to a specific toolchain's `rustc`.
///
/// Returns `None` when rustup is absent or does not have that toolchain, which
/// are handled differently by the caller and so are not distinguished here.
fn rustup_which(version: &str) -> Option<PathBuf> {
    let out = renzora_native_build::hide_console(&mut Command::new(renzora_native_build::tool("rustup")))
        .args(["which", "--toolchain", version, "rustc"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let path = PathBuf::from(String::from_utf8_lossy(&out.stdout).trim().to_string());
    path.is_file().then_some(path)
}

fn have_rustup() -> bool {
    renzora_native_build::hide_console(&mut Command::new(renzora_native_build::tool("rustup")))
        .arg("--version")
        .output()
        .is_ok_and(|o| o.status.success())
}

/// The release string of whatever `rustc` is on `PATH`, if any.
fn path_rustc_release() -> Option<String> {
    let out = renzora_native_build::hide_console(&mut Command::new(renzora_native_build::tool("rustc")))
        .arg("-vV")
        .output()
        .ok()?;
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .find_map(|l| l.strip_prefix("release: "))
        .map(|s| s.trim().to_string())
}

/// Install the pinned toolchain through an existing rustup.
///
/// `--profile minimal` fetches rustc, the standard library and cargo, and skips
/// `rustdoc`, `clippy` and the LLVM tools — roughly 400 MB rather than the 1.9 GB
/// a default profile lands on disk. A plugin build opens none of the extras.
///
/// Deliberately NOT `--no-self-update`: leaving rustup's own maintenance alone is
/// its owner's business, not ours.
pub fn install_toolchain(version: &str) -> Result<(), String> {
    let out = renzora_native_build::hide_console(&mut Command::new(renzora_native_build::tool("rustup")))
        .args(["toolchain", "install", version, "--profile", "minimal"])
        .output()
        .map_err(|e| format!("could not run rustup: {e}"))?;
    if out.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
    }
}

impl Toolchain {
    /// The compiler to invoke, if there is one.
    pub fn rustc(&self) -> Option<&PathBuf> {
        match self {
            Toolchain::Ready(p) => Some(p),
            _ => None,
        }
    }

    /// What to tell the user, phrased for a dialog rather than a log.
    ///
    /// Says "Rust compiler" outright rather than folding it into "SDK". Anyone
    /// who cares what is on their machine will find `~/.rustup` afterwards; a
    /// prompt that already said so is a non-event, and one that did not is a bug
    /// report.
    pub fn needs(&self) -> Option<String> {
        match self {
            Toolchain::Ready(_) => None,
            Toolchain::ToolchainMissing { version } => Some(format!(
                "Plugins are compiled on your machine and need Rust {version}. \
                 It will be downloaded to ~/.rustup (about 400 MB)."
            )),
            Toolchain::RustupMissing { version } => Some(format!(
                "Plugins are compiled on your machine and need the Rust compiler \
                 ({version}), which is not installed. It will be downloaded to \
                 ~/.rustup (about 400 MB)."
            )),
        }
    }
}
