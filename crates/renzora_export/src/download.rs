//! Download runtime templates from GitHub releases.
//!
//! Fetches the latest release manifest from `releases/latest`, picks the
//! asset matching the requested platform, and installs it into the editor's
//! runtime template directory. Desktop assets are zip archives that get
//! extracted alongside the editor; mobile/web assets are saved as-is.

use std::io::Read as _;
use std::path::{Path, PathBuf};
use std::sync::{mpsc, Mutex};

use serde::Deserialize;

use crate::templates::Platform;

const RELEASE_API: &str = "https://api.github.com/repos/renzora/engine/releases/latest";
const USER_AGENT: &str = "renzora-editor";

/// Asset filename for each platform on the GitHub release.
pub fn release_asset_name(platform: Platform) -> &'static str {
    match platform {
        Platform::WindowsX64 => "renzora-runtime-windows.zip",
        Platform::LinuxX64 => "renzora-runtime-linux.zip",
        Platform::MacOSX64 => "renzora-runtime-macos-x64.zip",
        Platform::MacOSArm64 => "renzora-runtime-macos-arm64.zip",
        Platform::AndroidArm64 => "renzora-runtime-android-arm64.apk",
        Platform::AndroidX86_64 => "renzora-runtime-android-x86_64.apk",
        Platform::FireTVArm64 => "renzora-runtime-firetv-arm64.apk",
        Platform::IOSArm64 => "renzora-runtime-ios-arm64.zip",
        Platform::TvOSArm64 => "renzora-runtime-tvos-arm64.zip",
        Platform::WebWasm32 => "renzora-runtime-web-wasm32.zip",
    }
}

/// Whether the asset is a zip that should be extracted into runtime_dir
/// (desktop), or saved as a single file (mobile/web templates).
fn extract_into_runtime_dir(platform: Platform) -> bool {
    matches!(
        platform,
        Platform::WindowsX64 | Platform::LinuxX64 | Platform::MacOSX64 | Platform::MacOSArm64
    )
}

#[derive(Debug, Clone)]
pub enum DownloadProgress {
    Fetching(String),
    Done(String),
    Error(String),
}

pub struct DownloadTask {
    pub platform: Platform,
    pub rx: Mutex<mpsc::Receiver<DownloadProgress>>,
}

#[derive(Deserialize)]
struct ReleaseManifest {
    tag_name: String,
    assets: Vec<ReleaseAsset>,
}

#[derive(Deserialize, Clone)]
struct ReleaseAsset {
    name: String,
    size: u64,
    browser_download_url: String,
}

/// Cached info about the latest release.
#[derive(Debug, Clone, Default)]
pub struct ReleaseInfo {
    pub tag_name: String,
    /// Platforms that have an asset published in the latest release.
    pub available_platforms: std::collections::HashSet<Platform>,
}

/// Fetch the latest release manifest and figure out which platforms have assets.
pub fn fetch_release_info() -> Result<ReleaseInfo, String> {
    let text = ureq::get(RELEASE_API)
        .header("User-Agent", USER_AGENT)
        .header("Accept", "application/vnd.github+json")
        .call()
        .map_err(|e| format!("Failed to fetch release: {}", e))?
        .into_body()
        .read_to_string()
        .map_err(|e| format!("Failed to read release body: {}", e))?;

    let manifest: ReleaseManifest = serde_json::from_str(&text)
        .map_err(|e| format!("Failed to parse release manifest: {}", e))?;

    let mut available = std::collections::HashSet::new();
    for platform in Platform::ALL {
        let name = release_asset_name(*platform);
        if manifest.assets.iter().any(|a| a.name == name) {
            available.insert(*platform);
        }
    }

    Ok(ReleaseInfo {
        tag_name: manifest.tag_name,
        available_platforms: available,
    })
}

/// Spawn a background thread to download and install the runtime for a platform.
pub fn spawn_download(platform: Platform, runtime_dir: PathBuf) -> DownloadTask {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(DownloadProgress::Fetching("Querying release...".into()));
        match download_and_install(platform, &runtime_dir, &tx) {
            Ok(msg) => {
                let _ = tx.send(DownloadProgress::Done(msg));
            }
            Err(e) => {
                let _ = tx.send(DownloadProgress::Error(e));
            }
        }
    });
    DownloadTask {
        platform,
        rx: Mutex::new(rx),
    }
}

fn download_and_install(
    platform: Platform,
    runtime_dir: &Path,
    tx: &mpsc::Sender<DownloadProgress>,
) -> Result<String, String> {
    let asset_name = release_asset_name(platform);

    let manifest_text = ureq::get(RELEASE_API)
        .header("User-Agent", USER_AGENT)
        .header("Accept", "application/vnd.github+json")
        .call()
        .map_err(|e| format!("Failed to fetch release: {}", e))?
        .into_body()
        .read_to_string()
        .map_err(|e| format!("Failed to read release body: {}", e))?;

    let manifest: ReleaseManifest = serde_json::from_str(&manifest_text)
        .map_err(|e| format!("Failed to parse release manifest: {}", e))?;

    let asset = manifest
        .assets
        .iter()
        .find(|a| a.name == asset_name)
        .ok_or_else(|| {
            format!(
                "No asset '{}' in release {} (not yet published)",
                asset_name, manifest.tag_name
            )
        })?
        .clone();

    let _ = tx.send(DownloadProgress::Fetching(format!(
        "Downloading {} ({:.1} MB)...",
        asset.name,
        asset.size as f64 / 1_000_000.0
    )));

    let mut response = ureq::get(&asset.browser_download_url)
        .header("User-Agent", USER_AGENT)
        .call()
        .map_err(|e| format!("Download failed: {}", e))?;

    let mut bytes = Vec::with_capacity(asset.size as usize);
    response
        .body_mut()
        .as_reader()
        .read_to_end(&mut bytes)
        .map_err(|e| format!("Read failed: {}", e))?;

    std::fs::create_dir_all(runtime_dir)
        .map_err(|e| format!("Failed to create runtime dir: {}", e))?;

    if extract_into_runtime_dir(platform) {
        let _ = tx.send(DownloadProgress::Fetching(format!(
            "Extracting into {}...",
            runtime_dir.display()
        )));
        let cursor = std::io::Cursor::new(&bytes);
        let mut archive = zip::ZipArchive::new(cursor).map_err(|e| format!("Bad zip: {}", e))?;
        archive
            .extract(runtime_dir)
            .map_err(|e| format!("Extract failed: {}", e))?;
    } else {
        let dest = runtime_dir.join(platform.runtime_binary_name());
        std::fs::write(&dest, &bytes).map_err(|e| format!("Write failed: {}", e))?;
    }

    Ok(format!(
        "Installed {} from {}",
        platform.display_name(),
        manifest.tag_name
    ))
}
