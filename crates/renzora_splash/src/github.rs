//! Lightweight GitHub stats fetch (just the repo star count).
//!
//! Runs once on splash startup. The result is cached in a resource so the
//! UI can show "— stars" while it's loading and the real number once it
//! arrives.

use bevy::prelude::*;
use serde::Deserialize;
use std::sync::{mpsc, Mutex};

const REPO_API: &str = "https://api.github.com/repos/renzora/engine";

#[derive(Deserialize)]
struct RepoResponse {
    stargazers_count: u64,
}

#[derive(Resource, Default)]
pub struct GithubStats {
    pub stars: Option<u64>,
    receiver: Option<Mutex<mpsc::Receiver<u64>>>,
}

impl GithubStats {
    pub fn new() -> Self {
        let mut stats = Self::default();
        stats.kick_off();
        stats
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn kick_off(&mut self) {
        let (tx, rx) = mpsc::channel();
        self.receiver = Some(Mutex::new(rx));
        std::thread::spawn(move || {
            if let Some(count) = fetch_stars() {
                let _ = tx.send(count);
            }
        });
    }

    #[cfg(target_arch = "wasm32")]
    fn kick_off(&mut self) {}

    pub fn poll(&mut self) {
        if self.stars.is_some() {
            return;
        }
        let msg = self
            .receiver
            .as_ref()
            .and_then(|rx| rx.lock().ok())
            .and_then(|rx| rx.try_recv().ok());
        if let Some(count) = msg {
            self.stars = Some(count);
            self.receiver = None;
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn fetch_stars() -> Option<u64> {
    let response = ureq::get(REPO_API)
        .header("User-Agent", "renzora-splash")
        .header("Accept", "application/vnd.github+json")
        .call()
        .ok()?;
    let text = response.into_body().read_to_string().ok()?;
    let parsed: RepoResponse = serde_json::from_str(&text).ok()?;
    Some(parsed.stargazers_count)
}

/// Formats a star count compactly: 1234 -> "1.2k".
pub fn format_count(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.1}k", n as f64 / 1_000.0)
    } else {
        n.to_string()
    }
}
