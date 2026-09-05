//! The changelog feed behind the dashboard's Changelog page.
//!
//! The source is GitHub's releases for `renzora/engine`, not a `CHANGELOG.md` in
//! the repo. That is deliberate: a file in the checkout only ever describes the
//! build you already have, and the reason to read a changelog from the launcher
//! is to find out what landed *after* it. The releases API answers that, and it
//! is the same list `releases.json` and the export templates are cut from.
//!
//! Fetched once per launch on a worker thread, like [`crate::github`] — an
//! unauthenticated GitHub call is rate-limited per IP, so re-fetching on every
//! visit to the page would spend the budget on someone clicking back and forth.
//! A failure is kept and shown, because "we could not reach GitHub" is a better
//! page than an empty one that looks like no releases exist.

use bevy::prelude::*;
#[cfg(not(target_arch = "wasm32"))]
use serde::Deserialize;
use std::sync::{mpsc, Mutex};

#[cfg(not(target_arch = "wasm32"))]
const RELEASES_API: &str = "https://api.github.com/repos/renzora/engine/releases?per_page=12";

/// Where the Changelog page's "see them all" link goes.
pub const RELEASES_URL: &str = "https://github.com/renzora/engine/releases";

/// One published release, flattened to what the page draws.
#[derive(Clone, Debug)]
pub struct ReleaseEntry {
    /// `r1-alpha6` — the tag, and what a plugin's `min_engine_version` names.
    pub tag: String,
    /// The release's title, or the tag again when it was published untitled.
    pub title: String,
    /// `2026-05-01`, the date part of the API's RFC 3339 timestamp.
    pub date: String,
    /// The release notes, as the markdown GitHub stores them.
    pub body: String,
    pub url: String,
    pub prerelease: bool,
}

/// The API's shape, kept separate from [`ReleaseEntry`] so the page draws a
/// flattened value and never an `Option` the JSON happened to omit.
#[cfg(not(target_arch = "wasm32"))]
#[derive(Deserialize)]
struct ApiRelease {
    tag_name: String,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    body: Option<String>,
    #[serde(default)]
    published_at: Option<String>,
    #[serde(default)]
    html_url: String,
    #[serde(default)]
    prerelease: bool,
    #[serde(default)]
    draft: bool,
}

/// The worker's one-shot answer. `Mutex` because a Bevy resource must be `Sync`
/// and `mpsc::Receiver` is not.
type FetchRx = Mutex<mpsc::Receiver<Result<Vec<ReleaseEntry>, String>>>;

/// The fetched changelog. `loaded` distinguishes "still fetching" from "fetched
/// and there is nothing", which are different pages.
#[derive(Resource, Default)]
pub struct ReleaseFeed {
    pub entries: Vec<ReleaseEntry>,
    pub error: Option<String>,
    pub loaded: bool,
    receiver: Option<FetchRx>,
}

impl ReleaseFeed {
    pub fn new() -> Self {
        let mut feed = Self::default();
        feed.kick_off();
        feed
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn kick_off(&mut self) {
        let (tx, rx) = mpsc::channel();
        self.receiver = Some(Mutex::new(rx));
        std::thread::Builder::new()
            .name("renzora-splash-releases".to_string())
            .spawn(move || {
                let _ = tx.send(fetch_releases());
            })
            .ok();
    }

    /// No worker, and no fetch: the browser build has no thread to run a
    /// blocking request on, and `renzora_net` has no backend there.
    #[cfg(target_arch = "wasm32")]
    fn kick_off(&mut self) {
        self.loaded = true;
        self.error = Some("The changelog isn't available in the browser build.".into());
    }

    pub fn poll(&mut self) {
        if self.loaded {
            return;
        }
        let msg = self
            .receiver
            .as_ref()
            .and_then(|rx| rx.lock().ok())
            .and_then(|rx| rx.try_recv().ok());
        let Some(msg) = msg else { return };
        self.receiver = None;
        self.loaded = true;
        match msg {
            Ok(entries) => self.entries = entries,
            Err(e) => self.error = Some(e),
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn fetch_releases() -> Result<Vec<ReleaseEntry>, String> {
    let response = renzora_net::Request::get(RELEASES_API)
        // GitHub rejects an API request with no User-Agent outright.
        .header("User-Agent", "renzora-splash")
        .header("Accept", "application/vnd.github+json")
        .send()
        .map_err(|e| format!("Could not reach GitHub: {e}"))?;
    if !response.is_ok() {
        return Err(format!("GitHub answered HTTP {}", response.status));
    }
    let raw: Vec<ApiRelease> = response
        .json()
        .map_err(|e| format!("Could not read the release list: {e}"))?;
    Ok(raw.into_iter().filter(|r| !r.draft).map(entry).collect())
}

#[cfg(not(target_arch = "wasm32"))]
fn entry(r: ApiRelease) -> ReleaseEntry {
    let title = r
        .name
        .filter(|n| !n.trim().is_empty())
        .unwrap_or_else(|| r.tag_name.clone());
    // `published_at` is RFC 3339 (`2026-05-01T09:12:00Z`) and the page has room
    // for the day, not the minute.
    let date = r
        .published_at
        .and_then(|s| s.split('T').next().map(str::to_string))
        .unwrap_or_default();
    ReleaseEntry {
        tag: r.tag_name,
        title,
        date,
        body: r.body.unwrap_or_default(),
        url: r.html_url,
        prerelease: r.prerelease,
    }
}

/// Drain the worker's answer. Runs only while the splash is up — nothing else
/// reads this feed.
pub fn poll_releases(mut feed: ResMut<ReleaseFeed>) {
    feed.poll();
}
