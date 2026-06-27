//! Rust toolchain detection + private provisioning for the lean single-binary
//! export.
//!
//! The lean export *recompiles* the game from source (static Bevy + static std),
//! so it needs a working `cargo`. On a developer machine that's already on
//! `PATH`. On a canonical-editor machine there may be **no Rust at all** — so we
//! bootstrap `rustup` non-interactively into a **private** cache and point the
//! build at it. We deliberately use our own `CARGO_HOME`/`RUSTUP_HOME` and pass
//! `--no-modify-path`, so provisioning never touches the user's global
//! environment, shell profile, or any existing system Rust install.
//!
//! Detection order (cheapest first): cargo already on `PATH` → a toolchain we
//! provisioned on a previous run → fresh `rustup` bootstrap.

use std::io::Read as _;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::templates::Platform;

/// Pinned channel — keep in lockstep with `rust-toolchain.toml` and
/// `docker/base/Dockerfile` so a provisioned native build matches the canonical
/// container compiler (and the project's own `rust-toolchain.toml`, which cargo
/// would otherwise try to auto-install).
const TOOLCHAIN_CHANNEL: &str = "1.95.0";

/// A resolved toolchain plus the environment needed to invoke its `cargo`.
///
/// When `cargo` came from `PATH` the `*_home` fields are `None` (use the
/// ambient env). When we provisioned privately they point into the cache so the
/// `rustup` cargo proxy resolves the right toolchain without consulting — or
/// polluting — the user's global `~/.cargo` / `~/.rustup`.
pub struct Toolchain {
    pub cargo: PathBuf,
    cargo_home: Option<PathBuf>,
    rustup_home: Option<PathBuf>,
    /// Dirs to prepend to `PATH` (the private `cargo/bin`) so the cargo proxy,
    /// `rustc`, and bundled `rust-lld` resolve.
    path_prepend: Vec<PathBuf>,
}

impl Toolchain {
    /// A `cargo` [`Command`] pre-wired with the private env (if any). Callers
    /// add the build args.
    pub fn cargo_command(&self) -> Command {
        let mut cmd = Command::new(&self.cargo);
        if let Some(h) = &self.cargo_home {
            cmd.env("CARGO_HOME", h);
        }
        if let Some(h) = &self.rustup_home {
            cmd.env("RUSTUP_HOME", h);
        }
        if !self.path_prepend.is_empty() {
            let existing = std::env::var_os("PATH").unwrap_or_default();
            let mut parts: Vec<PathBuf> = self.path_prepend.clone();
            parts.extend(std::env::split_paths(&existing));
            if let Ok(joined) = std::env::join_paths(parts) {
                cmd.env("PATH", joined);
            }
        }
        cmd
    }
}

fn cargo_exe() -> &'static str {
    if cfg!(windows) {
        "cargo.exe"
    } else {
        "cargo"
    }
}

/// Look up an executable on `PATH` (adds `.exe` on Windows).
fn which(name: &str) -> Option<PathBuf> {
    let exe = if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_string()
    };
    let paths = std::env::var_os("PATH")?;
    std::env::split_paths(&paths)
        .map(|d| d.join(&exe))
        .find(|p| p.is_file())
}

/// rustup's host triple for the current desktop platform (`None` off-desktop —
/// the lean native path only targets the host).
fn host_rustup_triple() -> Option<&'static str> {
    Some(match Platform::current()? {
        Platform::WindowsX64 => "x86_64-pc-windows-msvc",
        Platform::LinuxX64 => "x86_64-unknown-linux-gnu",
        Platform::MacOSX64 => "x86_64-apple-darwin",
        Platform::MacOSArm64 => "aarch64-apple-darwin",
        _ => return None,
    })
}

/// The private cache root for a provisioned toolchain, under the editor's
/// runtime dir so it persists across exports and is reused.
fn cache_root(runtime_dir: &Path) -> PathBuf {
    runtime_dir.join(".toolchain")
}

/// Resolve a usable Rust toolchain, provisioning `rustup` privately if none is
/// found. `progress` receives human-readable status lines for the export modal.
pub fn ensure_rust(
    runtime_dir: &Path,
    progress: &mut dyn FnMut(String),
) -> Result<Toolchain, String> {
    // 1. Already on PATH (the dev case) — use it as-is, ambient env.
    if let Some(cargo) = which("cargo") {
        progress("Found Rust toolchain on PATH".into());
        return Ok(Toolchain {
            cargo,
            cargo_home: None,
            rustup_home: None,
            path_prepend: Vec::new(),
        });
    }

    // 2. Privately provisioned on a previous run — reuse the cache.
    let cache = cache_root(runtime_dir);
    let cargo_home = cache.join("cargo");
    let rustup_home = cache.join("rustup");
    let cargo_bin = cargo_home.join("bin").join(cargo_exe());
    if cargo_bin.is_file() {
        progress("Using cached private Rust toolchain".into());
        return Ok(Toolchain {
            cargo: cargo_bin,
            cargo_home: Some(cargo_home.clone()),
            rustup_home: Some(rustup_home),
            path_prepend: vec![cargo_home.join("bin")],
        });
    }

    // 3. Bootstrap rustup into the private cache (one-time, ~minutes).
    bootstrap_rustup(&cache, &cargo_home, &rustup_home, progress)?;

    let cargo_bin = cargo_home.join("bin").join(cargo_exe());
    if !cargo_bin.is_file() {
        return Err(format!(
            "rustup finished but cargo is missing at {}",
            cargo_bin.display()
        ));
    }
    Ok(Toolchain {
        cargo: cargo_bin,
        cargo_home: Some(cargo_home.clone()),
        rustup_home: Some(rustup_home),
        path_prepend: vec![cargo_home.join("bin")],
    })
}

/// Download `rustup-init` for the host and run it non-interactively, installing
/// the pinned toolchain (minimal profile = rustc + cargo + host std, no docs)
/// into the private cache. `--no-modify-path` keeps the user's shell untouched.
fn bootstrap_rustup(
    cache: &Path,
    cargo_home: &Path,
    rustup_home: &Path,
    progress: &mut dyn FnMut(String),
) -> Result<(), String> {
    let triple = host_rustup_triple()
        .ok_or("Lean builds are only supported on the host desktop platform")?;
    std::fs::create_dir_all(cache).map_err(|e| format!("Failed to create toolchain cache: {e}"))?;

    let init_name = if cfg!(windows) {
        "rustup-init.exe"
    } else {
        "rustup-init"
    };
    let init_path = cache.join(init_name);

    progress("Downloading rustup-init…".into());
    let url = format!("https://static.rust-lang.org/rustup/dist/{triple}/{init_name}");
    let mut resp = ureq::get(&url)
        .header("User-Agent", "renzora-editor")
        .call()
        .map_err(|e| format!("Failed to download rustup-init: {e}"))?;
    let mut bytes = Vec::new();
    resp.body_mut()
        .as_reader()
        .read_to_end(&mut bytes)
        .map_err(|e| format!("Failed to read rustup-init: {e}"))?;
    std::fs::write(&init_path, &bytes)
        .map_err(|e| format!("Failed to write rustup-init: {e}"))?;

    // rustup-init must be executable on Unix.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perm = std::fs::metadata(&init_path)
            .map_err(|e| e.to_string())?
            .permissions();
        perm.set_mode(0o755);
        std::fs::set_permissions(&init_path, perm).map_err(|e| e.to_string())?;
    }

    progress(format!("Installing Rust {TOOLCHAIN_CHANNEL} (minimal) — one-time setup…"));
    let status = Command::new(&init_path)
        .env("CARGO_HOME", cargo_home)
        .env("RUSTUP_HOME", rustup_home)
        .args([
            "-y",
            "--no-modify-path",
            "--profile",
            "minimal",
            "--default-toolchain",
            TOOLCHAIN_CHANNEL,
        ])
        .status()
        .map_err(|e| format!("Failed to run rustup-init: {e}"))?;
    if !status.success() {
        return Err("rustup-init exited with an error".into());
    }
    progress("Rust toolchain installed".into());
    Ok(())
}
