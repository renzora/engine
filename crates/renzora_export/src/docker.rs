//! Is Docker usable on this machine, and which toolchain image does a platform
//! need?
//!
//! A lean export recompiles the engine from source, and cargo can only target
//! the host triple. Every other platform therefore has to be built inside the
//! `ghcr.io/renzora/<platform>` toolchain container — which is the same thing
//! `renzora build <platform>` does, and the reason those images exist.
//!
//! The engine talks to `docker` directly rather than shelling out to the
//! `renzora` CLI. The CLI is `cargo install`ed and a game developer running a
//! downloaded editor will not have it, whereas Docker is the thing they must
//! install anyway. The image tag is a content hash of the Dockerfiles in the
//! checkout, computed the same way in three places now (CI, the CLI, here) —
//! duplicated deliberately, because the alternative is a dependency on a tool
//! that may not be present.

use std::path::Path;
use std::process::Command;

use crate::templates::Platform;

/// What a probe of the local Docker installation found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DockerStatus {
    /// `docker info` succeeded — the daemon is up and we can build.
    Ready,
    /// The `docker` binary is not on PATH.
    NotInstalled,
    /// `docker` exists but the daemon did not answer. Almost always Docker
    /// Desktop not being started yet, which is worth saying plainly rather than
    /// showing the raw error.
    NotRunning(String),
}

impl DockerStatus {
    pub fn is_ready(&self) -> bool {
        matches!(self, DockerStatus::Ready)
    }
}

/// Probe the local Docker installation.
///
/// Spawns a process, so call it when the modal opens or the selected preset
/// changes — never per frame.
pub fn probe() -> DockerStatus {
    let out = Command::new("docker").arg("info").arg("--format").arg("{{.ServerVersion}}").output();
    match out {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => DockerStatus::NotInstalled,
        Err(e) => DockerStatus::NotRunning(e.to_string()),
        Ok(o) if o.status.success() => DockerStatus::Ready,
        Ok(o) => {
            // Docker's own message is the useful half ("Cannot connect to the
            // Docker daemon…"); keep the first line and drop the rest, which is
            // usually a usage dump.
            let err = String::from_utf8_lossy(&o.stderr);
            let first = err.lines().find(|l| !l.trim().is_empty()).unwrap_or("docker info failed");
            DockerStatus::NotRunning(first.trim().to_string())
        }
    }
}

/// Where to send someone who does not have it.
pub const INSTALL_URL: &str = "https://www.docker.com/products/docker-desktop/";

/// The toolchain image a platform is built in.
///
/// `None` for a platform with no container of its own — currently only the web
/// bundle, which the wasm image builds but which does not take this path.
pub fn image_for(platform: Platform) -> Option<&'static str> {
    Some(match platform {
        Platform::LinuxX64 | Platform::LinuxArm64 => "linux",
        Platform::WindowsX64 | Platform::WindowsArm64 => "windows",
        Platform::MacOSX64 | Platform::MacOSArm64 => "macos",
        _ => return None,
    })
}

/// The Rust target triple a platform builds for.
///
/// `None` for anything the lean cross-build does not cover — the mobile and web
/// targets, which are not single desktop binaries and take other paths entirely.
pub fn rust_triple(platform: Platform) -> Option<&'static str> {
    Some(match platform {
        Platform::WindowsX64 => "x86_64-pc-windows-msvc",
        Platform::WindowsArm64 => "aarch64-pc-windows-msvc",
        Platform::LinuxX64 => "x86_64-unknown-linux-gnu",
        Platform::LinuxArm64 => "aarch64-unknown-linux-gnu",
        Platform::MacOSX64 => "x86_64-apple-darwin",
        Platform::MacOSArm64 => "aarch64-apple-darwin",
        _ => return None,
    })
}

/// Can a lean build for `platform` be produced here at all?
///
/// True for the host (native cargo) and for anything with a toolchain image
/// (a container). The caller still has to check that Docker is actually
/// [`probe`]-able for the second case.
pub fn lean_supported(platform: Platform) -> bool {
    Platform::current() == Some(platform)
        || (image_for(platform).is_some() && rust_triple(platform).is_some())
}

/// The `docker run` that compiles the export inside `platform`'s toolchain
/// image.
///
/// The isolated export workspace is mounted at `/app/src` — the same path
/// `docker/build-all.sh` builds from, and that matters rather than being
/// arbitrary. The images carry `/app/.cargo/config.toml` holding the per-target
/// linker settings (the xwin library paths for MSVC, the osxcross linker for
/// Darwin), and cargo finds it by walking up from the working directory. Mount
/// anywhere else and that config is out of scope, so the link fails with missing
/// system libraries and nothing says why.
///
/// `--rm` because this container is one build: the persistent per-checkout
/// containers belong to the `renzora` CLI, and an editor that quietly
/// accumulated them would be a surprise the user never asked for. The cost is
/// that cargo's cache lives in the mounted workspace rather than a container
/// volume, which is where we want it anyway — `target/export-src/target/`
/// persists between exports and keeps them incremental.
pub fn build_command(image_ref: &str, workspace: &Path) -> Command {
    let mut cmd = Command::new("docker");
    cmd.arg("run")
        .arg("--rm")
        .arg("-v")
        .arg(format!("{}:/app/src", workspace.display()))
        .arg("-w")
        .arg("/app/src")
        .arg(image_ref);
    cmd
}

/// `ghcr.io/renzora/<image>:<tag>`, where the tag is a content hash of the
/// Dockerfiles — `sha256(baseTag + docker/<image>/Dockerfile)[:12]`, with
/// `baseTag = sha256(docker/base/Dockerfile)[:12]`.
///
/// Folding the base tag into every platform tag is what makes a base change
/// cascade: edit `docker/base/Dockerfile` and every platform tag re-rolls, so a
/// stale image is never silently reused. CR bytes are stripped before hashing so
/// a checkout with CRLF line endings produces the same tag as one without —
/// otherwise a Windows clone would ask for an image tag that CI never published.
pub fn image_ref(workspace_dir: &Path, image: &str) -> Option<String> {
    let base = hash_file(&workspace_dir.join("docker").join("base").join("Dockerfile"), None)?;
    let tag = hash_file(&workspace_dir.join("docker").join(image).join("Dockerfile"), Some(&base))?;
    Some(format!("ghcr.io/renzora/{image}:{tag}"))
}

/// First 12 hex of `sha256(prefix + contents-without-CR)`.
fn hash_file(path: &Path, prefix: Option<&str>) -> Option<String> {
    use sha2::{Digest, Sha256};
    let bytes = std::fs::read(path).ok()?;
    let mut hasher = Sha256::new();
    if let Some(p) = prefix {
        hasher.update(p.as_bytes());
    }
    hasher.update(bytes.iter().copied().filter(|b| *b != b'\r').collect::<Vec<u8>>());
    Some(format!("{:x}", hasher.finalize())[..12].to_string())
}
