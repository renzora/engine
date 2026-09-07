//! The engine's GitHub contributors, fetched once for the About overlay.
//!
//! About credits every upstream project the engine is built on, which left the
//! people who built *this* one as the only ones it did not name. This closes
//! that: the list comes from GitHub rather than a constant in the source, so it
//! cannot drift out of date the way a hand-maintained roll of names does, and
//! nobody has to remember to add themselves.
//!
//! Fetched lazily on the first About open, not at startup. It is one request
//! for a dialog most sessions never open, and an editor should not talk to the
//! network to draw its menus. It is also entirely optional: an offline editor
//! shows the rest of the overlay and simply omits the section, which is why
//! every failure here is silent.

use std::sync::{mpsc, Mutex};

use bevy::prelude::*;
use serde::Deserialize;

/// Contributors to the engine repository, most commits first. `per_page` is the
/// API's own cap for one page and far more than the overlay shows; there is no
/// paging because a second page would be people the section has no room for.
const CONTRIBUTORS_API: &str =
    "https://api.github.com/repos/renzora/engine/contributors?per_page=100";

/// One credited person, as About renders them.
pub(crate) struct Contributor {
    pub login: String,
    /// Sized down at the source: GitHub serves whatever the account uploaded,
    /// and the overlay draws these at 34px.
    pub avatar_url: String,
    pub html_url: String,
}

/// GitHub's shape. `type` distinguishes a person from a bot ("Bot"), which is
/// the one field here that exists to filter rather than to display.
#[derive(Deserialize)]
struct ApiContributor {
    login: String,
    avatar_url: String,
    html_url: String,
    #[serde(rename = "type")]
    kind: String,
}

/// The fetch, and its result. Absent list + no receiver = never asked.
#[derive(Resource, Default)]
pub(crate) struct Contributors {
    pub list: Vec<Contributor>,
    /// Set once the request is away, cleared when it lands or fails, so
    /// [`start`] is a no-op on the second About open.
    receiver: Option<Mutex<mpsc::Receiver<Vec<Contributor>>>>,
    asked: bool,
}

impl Contributors {
    /// Kick the fetch off, unless it has already been asked for this session.
    pub fn start(&mut self) {
        if self.asked {
            return;
        }
        self.asked = true;
        self.kick_off();
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn kick_off(&mut self) {
        let (tx, rx) = mpsc::channel();
        self.receiver = Some(Mutex::new(rx));
        // `renzora::net::fetch` blocks its own thread while the frame carries
        // on, and must never be called from a system — see `renzora::net`.
        std::thread::spawn(move || {
            if let Some(list) = fetch() {
                let _ = tx.send(list);
            }
        });
    }

    /// No `ureq` on the web, and the fetch backend there is the page's own
    /// `fetch` — not something this section is worth wiring up for.
    #[cfg(target_arch = "wasm32")]
    fn kick_off(&mut self) {}
}

/// Move a finished fetch into the resource. Cheap enough to run every frame:
/// it does nothing at all until someone opens About, and nothing again once the
/// answer is in.
pub(crate) fn poll_contributors(mut contributors: ResMut<Contributors>) {
    let received = contributors
        .receiver
        .as_ref()
        .and_then(|rx| rx.lock().ok())
        .and_then(|rx| rx.try_recv().ok());
    if let Some(list) = received {
        contributors.list = list;
        contributors.receiver = None;
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn fetch() -> Option<Vec<Contributor>> {
    let response = renzora::net::Request::get(CONTRIBUTORS_API)
        // GitHub rejects an API request with no User-Agent outright.
        .header("User-Agent", "renzora-editor")
        .header("Accept", "application/vnd.github+json")
        .send()
        .ok()?;
    if !response.is_ok() {
        return None;
    }
    let parsed: Vec<ApiContributor> = response.json().ok()?;
    Some(
        parsed
            .into_iter()
            .filter(|c| c.kind != "Bot")
            .map(|c| Contributor {
                // `s=68` is the 34px avatar at 2x. Without it GitHub serves the
                // full upload — a 460px PNG per person, decoded on the CPU.
                // The URL normally arrives with a `?v=4` already on it, but the
                // separator is picked rather than assumed.
                avatar_url: {
                    let sep = if c.avatar_url.contains('?') { '&' } else { '?' };
                    format!("{}{}s=68", c.avatar_url, sep)
                },
                login: c.login,
                html_url: c.html_url,
            })
            .collect(),
    )
}
