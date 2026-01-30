//! GitHub API version checking

use serde::Deserialize;
use std::sync::{mpsc, Arc};

use super::{UpdateState, SyncReceiver};

/// Result of checking for updates
#[derive(Clone, Debug)]
pub struct UpdateCheckResult {
    /// Whether an update is available
    pub update_available: bool,
    /// Latest version string (e.g., "0.2.0")
    pub latest_version: Option<String>,
    /// Current version string
    pub current_version: String,
    /// URL to the release page
    pub release_url: Option<String>,
    /// Release notes (markdown)
    pub release_notes: Option<String>,
    /// Direct download URL for the binary
    pub download_url: Option<String>,
    /// Asset file name
    pub asset_name: Option<String>,
}

/// GitHub API response structures
#[derive(Deserialize, Debug)]
struct GitHubRelease {
    tag_name: String,
    html_url: String,
    body: Option<String>,
    assets: Vec<GitHubAsset>,
}

#[derive(Deserialize, Debug)]
struct GitHubAsset {
    name: String,
    browser_download_url: String,
}

/// Start an update check in a background thread
pub fn start_update_check(state: &mut UpdateState) {
    if state.checking {
        return;
    }

    state.checking = true;
    state.error = None;

    let (sender, receiver) = mpsc::channel();
    state.check_receiver = Some(Arc::new(SyncReceiver::new(receiver)));

    std::thread::spawn(move || {
        let result = perform_check();
        let _ = sender.send(result);
    });
}

/// Perform the actual GitHub API check (runs in background thread)
fn perform_check() -> Result<UpdateCheckResult, String> {
    let current = super::current_version();

    // Make request to GitHub API
    let response = match ureq::get("https://api.github.com/repos/renzora/engine/releases/latest")
        .set("User-Agent", "renzora-editor")
        .set("Accept", "application/vnd.github.v3+json")
        .call()
    {
        Ok(resp) => resp,
        Err(ureq::Error::Status(404, _)) => {
            // No releases yet - this is not an error, just means no updates available
            return Ok(UpdateCheckResult {
                update_available: false,
                latest_version: None,
                current_version: current.to_string(),
                release_url: None,
                release_notes: None,
                download_url: None,
                asset_name: None,
            });
        }
        Err(e) => return Err(format!("Failed to check for updates: {}", e)),
    };

    let release: GitHubRelease = response
        .into_json()
        .map_err(|e| format!("Failed to parse release info: {}", e))?;

    // Parse version from tag (remove 'v' prefix if present)
    let latest_version = release.tag_name.trim_start_matches('v').to_string();

    // Compare versions using simple semver comparison
    let update_available = is_newer_version(&latest_version, current);

    // Find the appropriate asset (looking for renzora_rX.exe pattern)
    let (download_url, asset_name) = find_windows_asset(&release.assets);

    Ok(UpdateCheckResult {
        update_available,
        latest_version: Some(latest_version),
        current_version: current.to_string(),
        release_url: Some(release.html_url),
        release_notes: release.body,
        download_url,
        asset_name,
    })
}

/// Find Windows executable asset from release assets
fn find_windows_asset(assets: &[GitHubAsset]) -> (Option<String>, Option<String>) {
    // Look for patterns: renzora_rX.exe, renzora_editor.exe, etc.
    for asset in assets {
        let name_lower = asset.name.to_lowercase();
        if name_lower.ends_with(".exe") &&
           (name_lower.starts_with("renzora_r") ||
            name_lower.starts_with("renzora_editor") ||
            name_lower.contains("renzora")) {
            return (Some(asset.browser_download_url.clone()), Some(asset.name.clone()));
        }
    }

    // Fallback: any .exe file
    for asset in assets {
        if asset.name.to_lowercase().ends_with(".exe") {
            return (Some(asset.browser_download_url.clone()), Some(asset.name.clone()));
        }
    }

    (None, None)
}

/// Compare two semver version strings
/// Returns true if `latest` is newer than `current`
fn is_newer_version(latest: &str, current: &str) -> bool {
    let parse_version = |s: &str| -> (u32, u32, u32) {
        let parts: Vec<&str> = s.split('.').collect();
        let major = parts.first().and_then(|s| s.parse().ok()).unwrap_or(0);
        let minor = parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(0);
        let patch = parts.get(2).and_then(|s| s.parse().ok()).unwrap_or(0);
        (major, minor, patch)
    };

    let (l_major, l_minor, l_patch) = parse_version(latest);
    let (c_major, c_minor, c_patch) = parse_version(current);

    if l_major > c_major {
        return true;
    }
    if l_major == c_major && l_minor > c_minor {
        return true;
    }
    if l_major == c_major && l_minor == c_minor && l_patch > c_patch {
        return true;
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_comparison() {
        assert!(is_newer_version("1.0.0", "0.9.0"));
        assert!(is_newer_version("0.2.0", "0.1.0"));
        assert!(is_newer_version("0.1.1", "0.1.0"));
        assert!(!is_newer_version("0.1.0", "0.1.0"));
        assert!(!is_newer_version("0.0.9", "0.1.0"));
    }
}
