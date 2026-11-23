//! NVIDIA's DLSS runtime libraries: which ones a DLSS build loads, where the
//! editor installs them, and pointing the engine at them before the GPU starts.
//!
//! # Why the libraries are not in the download
//!
//! DLSS has two halves. The code is compiled into the engine (the `dlss`
//! feature, which needs the DLSS SDK on the machine that builds it, so CI has
//! it). The libraries that code loads at run time, `nvngx_dlss` and
//! `nvngx_dlssd`, are about 107 MB per platform and only do anything on an
//! NVIDIA RTX GPU, so the official downloads leave them out and the Solari
//! inspector offers to fetch them where they would actually run.
//!
//! # Why a folder of our own and `DLSS_SDK`
//!
//! `dlss_wgpu` looks for the libraries beside the executable and in
//! `$DLSS_SDK/lib/<platform>/rel`, and nowhere else. Beside the executable is
//! not always writable: a Linux AppImage is a read-only mount, and Windows may
//! have the editor somewhere protected. So the install goes to
//! `~/.renzora/dlss/<release>/lib/<platform>/rel`, laid out like the SDK, and
//! [`point_env_at_install`] sets `DLSS_SDK` to it at startup when nothing else
//! already has. A developer with the real SDK keeps theirs.
//!
//! # Why the release is pinned and every file is checksummed
//!
//! The bindings compiled into the engine match one SDK release exactly, so the
//! libraries are fetched from NVIDIA's own repository at that tag, and each is
//! checked against the checksum below before it is kept. A library that does
//! not match is deleted rather than loaded.

use std::path::{Path, PathBuf};

/// The DLSS SDK release the engine is built against. Must match `REQUIRED` in
/// `xtask/src/dlss.rs`, which checks the SDK a build compiles with.
pub const RELEASE: &str = "310.9.1";

/// One library the DLSS build loads at run time.
#[derive(Clone, Copy, Debug)]
pub struct Library {
    /// The file name the loader looks for.
    pub name: &'static str,
    /// Its SHA-256, as lowercase hex, at [`RELEASE`].
    pub sha256: &'static str,
    /// Its size in bytes, for the progress bar before the server says.
    pub size: u64,
}

/// Where NVIDIA keeps each platform's libraries in the SDK repository.
fn sdk_platform() -> Option<&'static str> {
    if cfg!(all(target_os = "windows", target_arch = "x86_64")) {
        Some("Windows_x86_64")
    } else if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        Some("Linux_x86_64")
    } else {
        None
    }
}

/// The libraries this platform needs, or none where NVIDIA ships no DLSS.
/// Super Resolution, then Ray Reconstruction.
pub fn libraries() -> &'static [Library] {
    if cfg!(all(target_os = "windows", target_arch = "x86_64")) {
        &[
            Library {
                name: "nvngx_dlss.dll",
                sha256: "3975567b8943c53acce397f2b72380092f84f162d00b0d2c7d08a1025c563983",
                size: 58_956_912,
            },
            Library {
                name: "nvngx_dlssd.dll",
                sha256: "4bc7ea5fcb2f32cf86bc2cb072e8d2860914a0be511cbdcb2fc206c1d9d83b80",
                size: 48_343_664,
            },
        ]
    } else if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        &[
            Library {
                name: "libnvidia-ngx-dlss.so.310.9.1",
                sha256: "7561f16cab74e2b7ccf791bf44bb9cc43abb44bcf78f33c5663520ad4e857e02",
                size: 59_481_944,
            },
            Library {
                name: "libnvidia-ngx-dlssd.so.310.9.1",
                sha256: "15043b4129a420b09dcd45a0294cb15f135da23054602827f68095f8080b05b0",
                size: 48_238_232,
            },
        ]
    } else {
        &[]
    }
}

/// The download URL of one library, from NVIDIA's repository at [`RELEASE`].
pub fn library_url(library: &Library) -> Option<String> {
    let platform = sdk_platform()?;
    Some(format!(
        "https://raw.githubusercontent.com/NVIDIA/DLSS/v{RELEASE}/lib/{platform}/rel/{}",
        library.name
    ))
}

/// NVIDIA's licence for the libraries, which travels with them.
pub fn license_url() -> String {
    format!("https://raw.githubusercontent.com/NVIDIA/DLSS/v{RELEASE}/LICENSE.txt")
}

/// `~/.renzora/dlss/<release>`: the root the install is laid out under, the
/// same shape as the SDK so `DLSS_SDK` can point at it.
pub fn install_root() -> Option<PathBuf> {
    if cfg!(target_arch = "wasm32") {
        return None;
    }
    let home = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)?;
    Some(home.join(".renzora").join("dlss").join(RELEASE))
}

/// The folder the libraries go in under [`install_root`].
pub fn install_dir() -> Option<PathBuf> {
    Some(install_root()?.join("lib").join(sdk_platform()?).join("rel"))
}

/// Whether every library is in `dir`.
fn complete_in(dir: &Path) -> bool {
    let libs = libraries();
    !libs.is_empty() && libs.iter().all(|l| dir.join(l.name).is_file())
}

/// Whether the libraries are somewhere the loader will find them: beside the
/// executable, in the editor's install, or in an SDK named by `DLSS_SDK`.
pub fn libraries_present() -> bool {
    let beside_exe = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(Path::to_path_buf))
        .is_some_and(|dir| complete_in(&dir));
    let installed = install_dir().is_some_and(|dir| complete_in(&dir));
    let sdk = std::env::var_os("DLSS_SDK")
        .zip(sdk_platform())
        .is_some_and(|(sdk, p)| complete_in(&PathBuf::from(sdk).join("lib").join(p).join("rel")));
    beside_exe || installed || sdk
}

/// Point `DLSS_SDK` at the editor's install when it is complete and nothing
/// has set the variable already. Call once, first thing in `main`, before any
/// thread is started: the variable is read when the GPU is set up.
pub fn point_env_at_install() {
    if std::env::var_os("DLSS_SDK").is_some() {
        return;
    }
    let (Some(root), Some(dir)) = (install_root(), install_dir()) else {
        return;
    };
    if complete_in(&dir) {
        // Single-threaded here: `main` calls this before Bevy, or anything
        // else that could read the environment, has started a thread.
        std::env::set_var("DLSS_SDK", root);
    }
}

/// Where the editor's DLSS install has got to, for the Solari inspector.
#[derive(bevy::prelude::Resource, Clone, Debug, Default, PartialEq)]
pub enum DlssInstall {
    /// Nothing started this run.
    #[default]
    Idle,
    /// Downloading: bytes so far, of the total.
    Downloading { done: u64, total: u64 },
    /// Installed and checked; DLSS starts on the next launch.
    Installed,
    /// Stopped, with why, in words for the inspector.
    Failed(String),
}
