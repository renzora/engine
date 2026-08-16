//! Finding the newest release this engine should be offered.

use serde::Deserialize;
use std::sync::mpsc;

use crate::version::ParsedVersion;

const RELEASES_API: &str = "https://api.github.com/repos/renzora/engine/releases?per_page=100";
const USER_AGENT: &str = "renzora-editor";

/// Which releases the updater offers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdateChannel {
    /// Published `r1-alpha*` releases only.
    Stable,
    /// Nightlies **and** releases — a nightly user should still be moved onto a
    /// version the day it ships, which the ordering in [`crate::version`] gives
    /// for free (a release outranks its own nightlies).
    Nightly,
}

impl UpdateChannel {
    /// Resolve the stored preference (`"auto"` / `"stable"` / `"nightly"`)
    /// against the channel this build came from.
    pub fn resolve(pref: &str) -> Self {
        match pref {
            "stable" => Self::Stable,
            "nightly" => Self::Nightly,
            // "auto" and anything unrecognised: follow this build. A build from
            // source has no release of its own and tracks `main`, so it gets
            // nightlies — which is also the only channel that could be newer
            // than an unreleased in-development version.
            _ => match renzora::version::channel() {
                renzora::version::BuildChannel::Release => Self::Stable,
                renzora::version::BuildChannel::Nightly
                | renzora::version::BuildChannel::Dev => Self::Nightly,
            },
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Stable => "stable",
            Self::Nightly => "nightly",
        }
    }
}

/// What a check found.
#[derive(Clone, Debug)]
pub struct UpdateCheckResult {
    pub update_available: bool,
    pub current_version: String,
    pub latest_version: Option<String>,
    pub release_url: Option<String>,
    pub release_notes: Option<String>,
    /// Download URL of the `<platform>.zip` engine asset for THIS host.
    pub download_url: Option<String>,
    pub asset_name: Option<String>,
    pub asset_size: u64,
    /// `sha256:<hex>` digest GitHub publishes for the asset, if it has one.
    pub asset_sha256: Option<String>,
    pub channel: UpdateChannel,
}

#[derive(Deserialize)]
struct GitHubRelease {
    tag_name: String,
    html_url: String,
    body: Option<String>,
    #[serde(default)]
    draft: bool,
    assets: Vec<GitHubAsset>,
}

#[derive(Deserialize, Clone)]
struct GitHubAsset {
    name: String,
    #[serde(default)]
    size: u64,
    browser_download_url: String,
    #[serde(default)]
    digest: Option<String>,
}

/// The tag this binary reports as its own, for display.
pub fn current_tag() -> String {
    renzora::version::release_tag()
        .map(|t| t.to_string())
        .unwrap_or_else(|| renzora::version::ENGINE_VERSION.to_string())
}

/// What the running binary compares AS.
///
/// Not the same thing as [`current_tag`], and conflating them was a bug: a build
/// from source has no tag, so it reported the bare `r1-alpha7` — which parses as
/// the *finished* release and therefore outranks every `r1-alpha7-nightly-*`. A
/// source checkout was told it was up to date while the dialog displayed the
/// very nightly it was refusing to offer.
///
/// [`ParsedVersion::dev`] puts it at `Stage::Dev` instead: the least finished
/// build of that version, below even last night's.
fn current_version() -> Option<ParsedVersion> {
    match renzora::version::release_tag() {
        Some(tag) => ParsedVersion::parse(tag),
        None => ParsedVersion::dev(renzora::version::ENGINE_VERSION),
    }
}

/// Run the check on a worker thread and send the result back.
///
/// Always off the main thread: this is a blocking network round-trip, and doing
/// it in a system would stall a frame for as long as GitHub takes to answer.
pub fn spawn_check(channel: UpdateChannel) -> mpsc::Receiver<Result<UpdateCheckResult, String>> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(perform_check(channel));
    });
    rx
}

fn perform_check(channel: UpdateChannel) -> Result<UpdateCheckResult, String> {
    let current = current_tag();

    let platform = renzora::version::host_platform_key().ok_or_else(|| {
        "No engine builds are published for this platform, so there is nothing to update to."
            .to_string()
    })?;
    let asset_name = format!("{platform}.zip");

    let response = renzora_net::Request::get(RELEASES_API)
        .header("User-Agent", USER_AGENT)
        .header("Accept", "application/vnd.github+json")
        .send()
        .map_err(|e| format!("Failed to reach GitHub: {e}"))?;
    if !response.is_ok() {
        return Err(format!("GitHub returned HTTP {}", response.status));
    }
    let releases: Vec<GitHubRelease> = response
        .json()
        .map_err(|e| format!("Failed to parse release list: {e}"))?;

    // Pick the newest release the channel allows. Note the filter is on the
    // PARSED TAG, not on GitHub's `prerelease` flag: the flag says how the
    // release was published, the tag says what it is, and the tag is what the
    // ordering rules are written against.
    let newest = releases
        .iter()
        .filter(|r| !r.draft)
        .filter_map(|r| ParsedVersion::parse(&r.tag_name).map(|v| (v, r)))
        .filter(|(v, _)| channel == UpdateChannel::Nightly || !v.is_nightly())
        .max_by(|(a, _), (b, _)| a.cmp(b));

    let Some((_, release)) = newest else {
        return Ok(UpdateCheckResult {
            update_available: false,
            current_version: current,
            latest_version: None,
            release_url: None,
            release_notes: None,
            download_url: None,
            asset_name: None,
            asset_size: 0,
            asset_sha256: None,
            channel,
        });
    };

    let asset = release.assets.iter().find(|a| a.name == asset_name);

    // Compared as ParsedVersion, not as strings: a dev build has no tag of its
    // own and has to compare as Stage::Dev (see `current_version`).
    let newer = match (ParsedVersion::parse(&release.tag_name), current_version()) {
        (Some(found), Some(running)) => found.is_newer_than(&running),
        _ => false,
    };

    Ok(UpdateCheckResult {
        update_available: newer,
        current_version: current,
        latest_version: Some(release.tag_name.clone()),
        release_url: Some(release.html_url.clone()),
        release_notes: release.body.clone(),
        download_url: asset.map(|a| a.browser_download_url.clone()),
        asset_name: asset.map(|a| a.name.clone()),
        asset_size: asset.map(|a| a.size).unwrap_or(0),
        asset_sha256: asset.and_then(|a| {
            a.digest
                .as_ref()
                .and_then(|d| d.strip_prefix("sha256:"))
                .map(|h| h.to_ascii_lowercase())
        }),
        channel,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_channel_prefs_win_over_the_build() {
        assert_eq!(UpdateChannel::resolve("stable"), UpdateChannel::Stable);
        assert_eq!(UpdateChannel::resolve("nightly"), UpdateChannel::Nightly);
    }

    #[test]
    fn auto_follows_this_build() {
        // The test binary is never CI-stamped, so this is the Dev case, which
        // tracks `main` — i.e. nightlies.
        assert_eq!(UpdateChannel::resolve("auto"), UpdateChannel::Nightly);
        // An unrecognised value must behave like "auto", not panic or pick a
        // channel at random — a hand-edited editor.toml is a normal thing.
        assert_eq!(
            UpdateChannel::resolve("banana"),
            UpdateChannel::resolve("auto")
        );
    }

    /// The screenshot bug: the check found `r1-alpha7-nightly-16aug26`, rendered
    /// its release notes, and still said "Renzora is up to date".
    #[test]
    fn a_dev_build_is_offered_its_versions_nightlies() {
        let running = current_version().expect("dev build parses");
        let nightly = ParsedVersion::parse("r1-alpha7-nightly-16aug26").unwrap();
        if renzora::version::release_tag().is_none()
            && renzora::version::ENGINE_VERSION == "r1-alpha7"
        {
            assert!(
                nightly.is_newer_than(&running),
                "a source checkout must be offered its own version's nightlies"
            );
        }
    }

    #[test]
    fn a_dev_build_reports_its_version_as_its_tag() {
        if renzora::version::release_tag().is_none() {
            assert_eq!(current_tag(), renzora::version::ENGINE_VERSION);
        }
    }

    #[test]
    fn asset_digests_parse_the_way_github_sends_them() {
        let a: GitHubAsset = serde_json::from_str(
            r#"{"name":"windows-x64.zip","size":42,"browser_download_url":"u","digest":"sha256:AB"}"#,
        )
        .unwrap();
        assert_eq!(a.size, 42);
        assert_eq!(
            a.digest
                .and_then(|d| d.strip_prefix("sha256:").map(|h| h.to_ascii_lowercase())),
            Some("ab".to_string())
        );
        // Older releases have no digest; that must parse, not fail the check.
        let b: GitHubAsset =
            serde_json::from_str(r#"{"name":"x.zip","browser_download_url":"u"}"#).unwrap();
        assert!(b.digest.is_none());
        assert_eq!(b.size, 0);
    }
}
