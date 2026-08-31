//! Download runtime export templates from GitHub releases.
//!
//! An export template is the game runtime for one platform (see
//! [`crate::templates`]). Exporting for a platform you are not sitting on needs
//! one, and this is where it comes from.
//!
//! # Which release
//!
//! Not `releases/latest` — *this engine's own*. The runtime and the editor are
//! two halves of one version: pairing an `r1-alpha7` editor with an `r1-alpha6`
//! runtime produces a game that fails to load the scene the editor just saved,
//! and the previous implementation asked for `releases/latest` unconditionally,
//! so it would have done exactly that the moment it worked at all.
//!
//! Resolution, in order:
//!
//! 1. **This binary's own tag**, when CI stamped one in
//!    (`renzora::version::release_tag`). A published build never has to guess.
//! 2. **A release tagged exactly [`ENGINE_VERSION`]** — the normal case once the
//!    version has shipped.
//! 3. **The newest nightly for this version** (`r1-alpha7-nightly-*`) — the case
//!    for a build from source, whose version has no release yet. Reported to the
//!    UI as a fallback so it is visible, not silent.
//!
//! There is deliberately no fourth step. Falling back to the previous *version*
//! would reintroduce exactly the mismatch this ordering exists to prevent.
//!
//! # Where it lands
//!
//! `~/.renzora/templates/<version>/<platform>/`, version-scoped and outside the
//! install directory — see [`crate::templates::user_templates_root`]. The old
//! code extracted into the editor's own exe directory, which `TemplateManager`
//! never scans, so even a successful download registered as "not installed".

use std::path::Path;
use std::sync::{mpsc, Mutex};

use serde::Deserialize;
use sha2::{Digest, Sha256};

use crate::templates::{self, Platform, TemplateStamp, TEMPLATE_STAMP};

const RELEASES_API: &str = "https://api.github.com/repos/renzora/engine/releases";
const USER_AGENT: &str = "renzora-editor";

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
    #[serde(default)]
    prerelease: bool,
    assets: Vec<ReleaseAsset>,
}

#[derive(Deserialize, Clone)]
struct ReleaseAsset {
    name: String,
    size: u64,
    browser_download_url: String,
    /// GitHub reports `sha256:<hex>` for assets it has digested. Absent on older
    /// releases, in which case the download is installed unverified rather than
    /// refused — a missing digest is GitHub's gap, not a corrupt file.
    #[serde(default)]
    digest: Option<String>,
}

/// The release this engine resolved to, and what it can offer.
#[derive(Debug, Clone, Default)]
pub struct ReleaseInfo {
    pub tag_name: String,
    /// Platforms that have an export template published in this release.
    pub available_platforms: std::collections::HashSet<Platform>,
    /// True when the engine's exact version had no release and this is the
    /// newest matching nightly. Surfaced in the UI so "where did this runtime
    /// come from?" is answerable.
    pub is_fallback: bool,
    /// Kept so a download reuses the resolution instead of re-querying — the
    /// unauthenticated GitHub API allows 60 requests an hour per IP, which a
    /// shared office network can exhaust between two people.
    assets: Vec<AssetRef>,
}

#[derive(Debug, Clone)]
struct AssetRef {
    name: String,
    size: u64,
    url: String,
    sha256: Option<String>,
}

/// Resolve the release matching this engine and list its export templates.
pub fn fetch_release_info() -> Result<ReleaseInfo, String> {
    let (manifest, is_fallback) = resolve_release()?;

    let mut available = std::collections::HashSet::new();
    for platform in Platform::ALL {
        let name = platform.release_asset_name();
        if manifest.assets.iter().any(|a| a.name == name) {
            available.insert(*platform);
        }
    }

    Ok(ReleaseInfo {
        tag_name: manifest.tag_name,
        available_platforms: available,
        is_fallback,
        assets: manifest
            .assets
            .into_iter()
            .map(|a| AssetRef {
                name: a.name,
                size: a.size,
                url: a.browser_download_url,
                sha256: a.digest.and_then(|d| {
                    d.strip_prefix("sha256:").map(|h| h.to_ascii_lowercase())
                }),
            })
            .collect(),
    })
}

/// Fetch one release by tag. `Ok(None)` for a 404 (no such release), which is an
/// expected answer during resolution rather than an error.
fn release_by_tag(tag: &str) -> Result<Option<ReleaseManifest>, String> {
    let response = renzora_net::Request::get(&format!("{RELEASES_API}/tags/{tag}"))
        .header("User-Agent", USER_AGENT)
        .header("Accept", "application/vnd.github+json")
        .send()
        .map_err(|e| format!("Failed to reach GitHub: {e}"))?;
    if response.status == 404 {
        return Ok(None);
    }
    if !response.is_ok() {
        return Err(format!("GitHub returned HTTP {}", response.status));
    }
    response
        .json()
        .map(Some)
        .map_err(|e| format!("Failed to parse release {tag}: {e}"))
}

/// See the module docs for the ordering. Returns the release and whether it was
/// reached by the nightly fallback.
fn resolve_release() -> Result<(ReleaseManifest, bool), String> {
    // 1. A published binary knows its own tag.
    if let Some(tag) = renzora::version::release_tag() {
        return match release_by_tag(tag)? {
            Some(r) => Ok((r, false)),
            None => Err(format!(
                "This engine was built as {tag}, but that release no longer exists."
            )),
        };
    }

    // 2. A release for this exact version.
    let version = renzora::version::ENGINE_VERSION;
    if let Some(r) = release_by_tag(version)? {
        return Ok((r, false));
    }

    // 3. The newest nightly for this version. `?per_page=100` is one request;
    //    GitHub returns releases newest-first, so the first tag matching the
    //    prefix is the one we want.
    let prefix = renzora::version::fallback_tag_prefix();
    let response = renzora_net::Request::get(&format!("{RELEASES_API}?per_page=100"))
        .header("User-Agent", USER_AGENT)
        .header("Accept", "application/vnd.github+json")
        .send()
        .map_err(|e| format!("Failed to reach GitHub: {e}"))?;
    if !response.is_ok() {
        return Err(format!("GitHub returned HTTP {}", response.status));
    }
    let releases: Vec<ReleaseManifest> = response
        .json()
        .map_err(|e| format!("Failed to parse release list: {e}"))?;

    releases
        .into_iter()
        .find(|r| r.prerelease && r.tag_name.starts_with(&prefix))
        .map(|r| (r, true))
        .ok_or_else(|| {
            format!(
                "No release or nightly for {version} yet. Export templates for other \
                 platforms need one — build them yourself with `renzora build <platform>`, \
                 or wait for tonight's nightly."
            )
        })
}

/// Spawn a background thread to download and install the template for a platform.
///
/// Takes the already-resolved [`ReleaseInfo`] so the install does not re-query
/// GitHub — the UI has fetched it before any Download button can be clicked.
pub fn spawn_download(platform: Platform, release: ReleaseInfo) -> DownloadTask {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(DownloadProgress::Fetching("Resolving release...".into()));
        match download_and_install(platform, &release, &tx) {
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

/// Spawn a background download of the engine SOURCE for this release.
///
/// A lean export recompiles the engine, so it needs the source — which a
/// canonical editor download does not ship. Rather than making lean builds a
/// contributors-only feature, the source rides the release as one more asset and
/// installs into `~/.renzora/src/<version>/`, exactly as a runtime template
/// installs into `~/.renzora/templates/<version>/<platform>/`.
///
/// Reuses [`DownloadTask`] so the modal's existing progress rendering works
/// unchanged. `platform` is carried only so the UI can key the status line it
/// already has; the archive itself is platform-independent.
pub fn spawn_source_download(platform: Platform, release: ReleaseInfo) -> DownloadTask {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(DownloadProgress::Fetching("Resolving release...".into()));
        match download_and_install_source(&release, &tx) {
            Ok(msg) => {
                let _ = tx.send(DownloadProgress::Done(msg));
            }
            Err(e) => {
                let _ = tx.send(DownloadProgress::Error(e));
            }
        }
    });
    DownloadTask { platform, rx: Mutex::new(rx) }
}

fn download_and_install_source(
    release: &ReleaseInfo,
    tx: &mpsc::Sender<DownloadProgress>,
) -> Result<String, String> {
    let asset = release
        .assets
        .iter()
        .find(|a| a.name == templates::SOURCE_ASSET)
        .ok_or_else(|| {
            format!(
                "{} has no {} — this release was published without the engine source, \
                 so a lean build needs a source checkout instead.",
                release.tag_name,
                templates::SOURCE_ASSET
            )
        })?;

    let bytes = fetch_verified(asset, tx)?;

    let dest = templates::user_source_dir().ok_or_else(|| {
        "No home directory to install into (neither HOME nor USERPROFILE is set).".to_string()
    })?;

    // Replace rather than merge, for the same reason a template is replaced: a
    // half-updated source tree would compile into a binary that matches neither
    // version, and the failure would surface as a mismatched `TypeId` at run
    // time rather than a compile error.
    if dest.exists() {
        std::fs::remove_dir_all(&dest)
            .map_err(|e| format!("Failed to clear {}: {e}", dest.display()))?;
    }
    std::fs::create_dir_all(&dest)
        .map_err(|e| format!("Failed to create {}: {e}", dest.display()))?;

    let _ = tx.send(DownloadProgress::Fetching(format!("Extracting into {}...", dest.display())));
    let cursor = std::io::Cursor::new(&bytes);
    let mut archive = zip::ZipArchive::new(cursor).map_err(|e| format!("Bad zip: {e}"))?;
    archive.extract(&dest).map_err(|e| format!("Extract failed: {e}"))?;

    Ok(format!("Installed engine source from {}", release.tag_name))
}

/// Download an asset and check it against the digest the release published.
///
/// Verify BEFORE writing anything: a truncated or tampered archive becomes a
/// binary handed to someone's players, so a mismatch aborts rather than
/// installing and hoping.
fn fetch_verified(
    asset: &AssetRef,
    tx: &mpsc::Sender<DownloadProgress>,
) -> Result<Vec<u8>, String> {
    let _ = tx.send(DownloadProgress::Fetching(format!(
        "Downloading {} ({:.1} MB)...",
        asset.name,
        asset.size as f64 / 1_000_000.0
    )));

    let response = renzora_net::Request::get(&asset.url)
        .header("User-Agent", USER_AGENT)
        .send()
        .map_err(|e| format!("Download failed: {e}"))?;
    if !response.is_ok() {
        return Err(format!("Download failed: HTTP {}", response.status));
    }
    let bytes = response.body;

    let actual = hex(&Sha256::digest(&bytes));
    if let Some(expected) = &asset.sha256 {
        if &actual != expected {
            return Err(format!(
                "Checksum mismatch for {} — expected {expected}, got {actual}. \
                 Nothing was installed.",
                asset.name
            ));
        }
        let _ = tx.send(DownloadProgress::Fetching("Checksum OK".into()));
    }
    Ok(bytes)
}

fn download_and_install(
    platform: Platform,
    release: &ReleaseInfo,
    tx: &mpsc::Sender<DownloadProgress>,
) -> Result<String, String> {
    let asset_name = platform.release_asset_name();
    let asset = release
        .assets
        .iter()
        .find(|a| a.name == asset_name)
        .ok_or_else(|| {
            format!(
                "{} has no {} — that platform wasn't built for this release.",
                release.tag_name, asset_name
            )
        })?;

    let _ = tx.send(DownloadProgress::Fetching(format!(
        "Downloading {} ({:.1} MB)...",
        asset.name,
        asset.size as f64 / 1_000_000.0
    )));

    let response = renzora_net::Request::get(&asset.url)
        .header("User-Agent", USER_AGENT)
        .send()
        .map_err(|e| format!("Download failed: {e}"))?;
    if !response.is_ok() {
        return Err(format!("Download failed: HTTP {}", response.status));
    }
    let bytes = response.body;

    // Verify before writing anything. A truncated or tampered template is a
    // binary we would hand to someone's players, so a mismatch aborts rather
    // than installing and hoping.
    let actual = hex(&Sha256::digest(&bytes));
    if let Some(expected) = &asset.sha256 {
        if &actual != expected {
            return Err(format!(
                "Checksum mismatch for {asset_name} — expected {expected}, got {actual}. \
                 Nothing was installed."
            ));
        }
        let _ = tx.send(DownloadProgress::Fetching("Checksum OK".into()));
    }

    let dest = templates::user_template_dir(platform).ok_or_else(|| {
        "No home directory to install into (neither HOME nor USERPROFILE is set).".to_string()
    })?;

    // Replace rather than merge: a previous template for the same platform may
    // hold plugins this release dropped, and a stale one would be loaded by the
    // exported game with no indication of where it came from.
    if dest.exists() {
        std::fs::remove_dir_all(&dest)
            .map_err(|e| format!("Failed to clear {}: {e}", dest.display()))?;
    }
    std::fs::create_dir_all(&dest)
        .map_err(|e| format!("Failed to create {}: {e}", dest.display()))?;

    let _ = tx.send(DownloadProgress::Fetching(format!(
        "Extracting into {}...",
        dest.display()
    )));
    let cursor = std::io::Cursor::new(&bytes);
    let mut archive = zip::ZipArchive::new(cursor).map_err(|e| format!("Bad zip: {e}"))?;
    archive
        .extract(&dest)
        .map_err(|e| format!("Extract failed: {e}"))?;

    ensure_executable(&dest, platform);

    let stamp = TemplateStamp {
        tag: release.tag_name.clone(),
        sha256: actual,
    };
    let _ = std::fs::write(
        dest.join(TEMPLATE_STAMP),
        serde_json::to_string_pretty(&stamp).unwrap_or_default(),
    );

    Ok(format!(
        "Installed {} from {}",
        platform.display_name(),
        release.tag_name
    ))
}

/// Restore the executable bit on the runtime binary.
///
/// The zip carries unix modes and the `zip` crate applies them, so this is
/// normally a no-op — but a template built on a Windows runner (windows-arm64)
/// has no modes to carry, and a host that unpacked one for a unix target would
/// otherwise produce an export nobody can launch.
#[cfg(unix)]
fn ensure_executable(dir: &Path, platform: Platform) {
    use std::os::unix::fs::PermissionsExt;
    let bin = dir.join(platform.runtime_binary_name());
    if let Ok(meta) = std::fs::metadata(&bin) {
        let mut perms = meta.permissions();
        perms.set_mode(perms.mode() | 0o755);
        let _ = std::fs::set_permissions(&bin, perms);
    }
}

#[cfg(not(unix))]
fn ensure_executable(_dir: &Path, _platform: Platform) {}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn asset_names_track_the_platform_key() {
        // The packaging script names assets `renzora-runtime-<dist_dir_name>.zip`;
        // if these ever diverge again, no download can succeed.
        for &p in Platform::ALL {
            assert_eq!(
                p.release_asset_name(),
                format!("renzora-runtime-{}.zip", p.dist_dir_name())
            );
        }
    }

    #[test]
    fn hex_is_lowercase_and_padded() {
        assert_eq!(hex(&[0x00, 0x0f, 0xff]), "000fff");
    }

    #[test]
    fn digest_prefix_is_stripped_and_lowercased() {
        let a: ReleaseAsset = serde_json::from_str(
            r#"{"name":"x.zip","size":1,"browser_download_url":"u","digest":"sha256:AABB"}"#,
        )
        .unwrap();
        let stripped = a
            .digest
            .and_then(|d| d.strip_prefix("sha256:").map(|h| h.to_ascii_lowercase()));
        assert_eq!(stripped.as_deref(), Some("aabb"));
    }

    #[test]
    fn an_asset_without_a_digest_parses() {
        let a: ReleaseAsset =
            serde_json::from_str(r#"{"name":"x.zip","size":1,"browser_download_url":"u"}"#)
                .unwrap();
        assert!(a.digest.is_none());
    }
}
