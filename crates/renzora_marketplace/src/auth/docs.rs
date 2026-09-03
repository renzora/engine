//! Docs portal API client — the four public endpoints behind the Docs panel.
//!
//! Doc pages are requested with `?format=md` so the editor renders the raw
//! markdown natively with `markdown_view`. Blocking calls, designed to run on a
//! background thread.
//!
//! Restored from `renzora_auth::docs` (deleted in 2ce2d58d with the social
//! crates) minus its courses half, which the website no longer serves.

use serde::Deserialize;

use super::client::api_base;
#[cfg(not(target_arch = "wasm32"))]
use super::client::get_json;

/// Percent-encode a query value. `renzora_auth::client` had this; the
/// marketplace client never needed it until search came back.
#[cfg(not(target_arch = "wasm32"))]
fn urlencoded(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(b as char),
            b' ' => out.push('+'),
            _ => {
                out.push('%');
                out.push_str(&format!("{b:02X}"));
            }
        }
    }
    out
}

// ── Types ──

#[derive(Debug, Deserialize, Clone)]
pub struct DocVersion {
    pub id: String,
    #[serde(default)]
    pub label: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct DocVersions {
    #[serde(default)]
    pub default: String,
    #[serde(default)]
    pub versions: Vec<DocVersion>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Sidebar {
    pub version: String,
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub groups: Vec<SidebarGroup>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct SidebarGroup {
    pub group: String,
    /// `basic` or `advanced` (empty = basic).
    #[serde(default)]
    pub level: String,
    #[serde(default)]
    pub categories: Vec<SidebarCategory>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct SidebarCategory {
    pub category: String,
    #[serde(default)]
    pub pages: Vec<SidebarPage>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct SidebarPage {
    pub slug: String,
    pub title: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct DocPage {
    pub version: String,
    pub slug: String,
    pub title: String,
    #[serde(default)]
    pub group: String,
    #[serde(default)]
    pub category: String,
    /// Raw markdown when fetched with `?format=md`.
    pub content: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct DocSearchResult {
    pub slug: String,
    pub title: String,
    #[serde(default)]
    pub group: String,
    #[serde(default)]
    pub category: String,
    #[serde(default)]
    pub version: String,
}

// ── API calls ──

/// Available doc versions.
#[cfg(not(target_arch = "wasm32"))]
pub fn get_versions() -> Result<DocVersions, String> {
    get_json(&format!("{}/api/docs/versions", api_base()), None)
}

/// Sidebar tree for a version.
#[cfg(not(target_arch = "wasm32"))]
pub fn get_sidebar(version: &str) -> Result<Sidebar, String> {
    get_json(&format!("{}/api/docs/sidebar/{version}", api_base()), None)
}

/// A doc page as raw markdown.
#[cfg(not(target_arch = "wasm32"))]
pub fn get_page(version: &str, slug: &str) -> Result<DocPage, String> {
    get_json(
        &format!("{}/api/docs/page/{version}/{slug}?format=md", api_base()),
        None,
    )
}

/// Search docs within a version.
#[cfg(not(target_arch = "wasm32"))]
pub fn search(version: &str, query: &str) -> Result<Vec<DocSearchResult>, String> {
    get_json(
        &format!("{}/api/docs/search/{version}?q={}", api_base(), urlencoded(query)),
        None,
    )
}
