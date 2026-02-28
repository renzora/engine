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

    // Fetch all releases (includes pre-releases, unlike /releases/latest)
    let response = ureq::get("https://api.github.com/repos/renzora/engine/releases")
        .set("User-Agent", "renzora-editor")
        .set("Accept", "application/vnd.github.v3+json")
        .call()
        .map_err(|e| format!("Failed to check for updates: {}", e))?;

    let releases: Vec<GitHubRelease> = response
        .into_json()
        .map_err(|e| format!("Failed to parse release info: {}", e))?;

    // Find the newest release by version comparison
    let newest = releases.iter()
        .filter_map(|r| {
            let parsed = ParsedVersion::parse(&r.tag_name)?;
            Some((parsed, r))
        })
        .max_by(|(a, _), (b, _)| a.cmp(b));

    let Some((_, release)) = newest else {
        return Ok(UpdateCheckResult {
            update_available: false,
            latest_version: None,
            current_version: current.to_string(),
            release_url: None,
            release_notes: None,
            download_url: None,
            asset_name: None,
        });
    };

    let latest_version = release.tag_name.clone();
    let update_available = is_newer_version(&latest_version, current);
    let (download_url, asset_name) = find_download_asset(&release.assets);

    Ok(UpdateCheckResult {
        update_available,
        latest_version: Some(latest_version),
        current_version: current.to_string(),
        release_url: Some(release.html_url.clone()),
        release_notes: release.body.clone(),
        download_url,
        asset_name,
    })
}

/// Find downloadable asset from release assets (.exe preferred, .zip fallback)
fn find_download_asset(assets: &[GitHubAsset]) -> (Option<String>, Option<String>) {
    // Prefer .exe files (single-binary distribution)
    for asset in assets {
        let name_lower = asset.name.to_lowercase();
        if name_lower.ends_with(".exe") {
            return (Some(asset.browser_download_url.clone()), Some(asset.name.clone()));
        }
    }

    // Fallback: .zip files
    for asset in assets {
        let name_lower = asset.name.to_lowercase();
        if name_lower.ends_with(".zip") {
            return (Some(asset.browser_download_url.clone()), Some(asset.name.clone()));
        }
    }

    (None, None)
}

/// Parsed version from tag format like "r1-alpha4", "r1-beta1", "r1", "r2-alpha1"
#[derive(Debug, Clone, PartialEq, Eq)]
struct ParsedVersion {
    release: u32,
    /// None = stable, Some("alpha", N) or Some("beta", N) = pre-release
    pre: Option<(String, u32)>,
}

impl PartialOrd for ParsedVersion {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ParsedVersion {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        match self.release.cmp(&other.release) {
            std::cmp::Ordering::Equal => {}
            ord => return ord,
        }
        match (&self.pre, &other.pre) {
            (None, None) => std::cmp::Ordering::Equal,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (Some(_), None) => std::cmp::Ordering::Less,
            (Some((a_type, a_num)), Some((b_type, b_num))) => {
                match a_type.cmp(b_type) {
                    std::cmp::Ordering::Equal => a_num.cmp(b_num),
                    ord => ord,
                }
            }
        }
    }
}

impl ParsedVersion {
    /// Parse a version string like "r1-alpha4", "r1-beta1", "r1"
    fn parse(s: &str) -> Option<Self> {
        let s = s.trim_start_matches('v');
        let s = s.strip_prefix('r')?;

        if let Some((release_str, pre_str)) = s.split_once('-') {
            let release = release_str.parse().ok()?;
            // Split pre-release like "alpha2" into ("alpha", 2)
            let split_pos = pre_str.find(|c: char| c.is_ascii_digit())?;
            let (pre_type, pre_num_str) = pre_str.split_at(split_pos);
            let pre_num = pre_num_str.parse().ok()?;
            Some(Self { release, pre: Some((pre_type.to_string(), pre_num)) })
        } else {
            let release = s.parse().ok()?;
            Some(Self { release, pre: None })
        }
    }

    /// Returns true if self is newer than other
    fn is_newer_than(&self, other: &Self) -> bool {
        if self.release != other.release {
            return self.release > other.release;
        }
        // Same release number — stable (None) is newer than any pre-release
        match (&self.pre, &other.pre) {
            (None, Some(_)) => true,
            (Some(_), None) => false,
            (None, None) => false,
            (Some((a_type, a_num)), Some((b_type, b_num))) => {
                if a_type != b_type {
                    // alpha < beta
                    a_type > b_type
                } else {
                    a_num > b_num
                }
            }
        }
    }
}

/// Compare two version strings in rX-alphaN format
/// Returns true if `latest` is newer than `current`
fn is_newer_version(latest: &str, current: &str) -> bool {
    match (ParsedVersion::parse(latest), ParsedVersion::parse(current)) {
        (Some(l), Some(c)) => l.is_newer_than(&c),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_comparison() {
        // Same release, higher alpha
        assert!(is_newer_version("r1-alpha5", "r1-alpha4"));
        assert!(!is_newer_version("r1-alpha4", "r1-alpha5"));
        // Same version
        assert!(!is_newer_version("r1-alpha4", "r1-alpha4"));
        // Higher release
        assert!(is_newer_version("r2-alpha1", "r1-alpha4"));
        // Stable is newer than pre-release
        assert!(is_newer_version("r1", "r1-alpha4"));
        assert!(!is_newer_version("r1-alpha4", "r1"));
        // Beta is newer than alpha
        assert!(is_newer_version("r1-beta1", "r1-alpha4"));
        assert!(!is_newer_version("r1-alpha4", "r1-beta1"));
    }

    #[test]
    fn test_version_parsing() {
        assert!(ParsedVersion::parse("r1-alpha4").is_some());
        assert!(ParsedVersion::parse("r1-beta1").is_some());
        assert!(ParsedVersion::parse("r1").is_some());
        assert!(ParsedVersion::parse("r10-alpha15").is_some());
        // Invalid formats
        assert!(ParsedVersion::parse("0.1.0").is_none());
        assert!(ParsedVersion::parse("invalid").is_none());
    }
}
