//! Marketplace API client for the Renzora engine.
//!
//! Provides blocking HTTP calls to browse, search, and download marketplace assets.
//! All functions are designed to run on background threads.

use serde::{Deserialize, Serialize};

#[cfg(not(target_arch = "wasm32"))]
use super::session::AuthSession;

/// Base URL for the Renzora API.
const API_BASE: &str = "https://renzora.com";

// ── Types ──

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct AssetSummary {
    pub id: String,
    pub name: String,
    pub slug: String,
    pub description: String,
    pub category: String,
    pub price_credits: i64,
    pub thumbnail_url: Option<String>,
    pub version: String,
    pub downloads: i64,
    pub creator_name: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct MarketplaceListResponse {
    pub assets: Vec<AssetSummary>,
    pub total: i64,
    pub page: i64,
    pub per_page: i64,
}

#[derive(Debug, Deserialize, Clone)]
pub struct AssetDetail {
    pub id: String,
    pub name: String,
    pub slug: String,
    pub description: String,
    pub category: String,
    pub price_credits: i64,
    pub file_url: Option<String>,
    pub thumbnail_url: Option<String>,
    pub version: String,
    pub downloads: i64,
    pub published: bool,
    pub owned: Option<bool>,
    #[serde(default)]
    pub creator_name: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct DownloadResponse {
    pub download_url: String,
    #[serde(default)]
    pub download_filename: String,
    /// Per-file details. Only `file_size` is read here, to give the install
    /// progress bar a real total — the transport reports no Content-Length, so
    /// without this the bar can only sweep.
    #[serde(default)]
    pub files: Vec<DownloadFileInfo>,
}

#[derive(Debug, Deserialize, Clone, Default)]
pub struct DownloadFileInfo {
    #[serde(default)]
    pub file_size: i64,
}

impl DownloadResponse {
    /// Total bytes to expect, when the server said. `None` for a multi-file
    /// asset or a response without sizes, where one number would be a guess.
    pub fn total_bytes(&self) -> Option<u64> {
        match self.files.as_slice() {
            [only] if only.file_size > 0 => Some(only.file_size as u64),
            _ => None,
        }
    }
}

/// What the marketplace currently publishes for an installed plugin.
#[derive(Debug, Deserialize, Clone)]
pub struct PluginUpdate {
    pub id: String,
    pub slug: String,
    pub name: String,
    pub version: String,
    /// Minimum engine release. Empty means the creator did not set one, which
    /// is treated as "any" rather than as a floor nobody meets.
    #[serde(default)]
    pub min_engine_version: String,
    #[serde(default)]
    pub published: bool,
}

/// Latest published versions for a set of installed plugins, in one request.
///
/// Public — an update check should not need a sign-in, and the version is on
/// the listing anyway.
#[cfg(not(target_arch = "wasm32"))]
pub fn plugin_updates(ids: &[String]) -> Result<Vec<PluginUpdate>, String> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    super::client::post_json(
        &format!("{}/api/marketplace/plugin-updates", super::client::api_base()),
        &serde_json::json!({ "ids": ids }),
        None,
    )
}

// ── API calls (blocking) ──

/// Search/browse marketplace assets.
#[cfg(not(target_arch = "wasm32"))]
pub fn list_assets(
    query: Option<&str>,
    category: Option<&str>,
    sort: Option<&str>,
    page: u32,
    min_rating: Option<i32>,
    max_price: Option<i64>,
) -> Result<MarketplaceListResponse, String> {
    let mut url = format!("{API_BASE}/api/marketplace?page={page}");
    if let Some(q) = query {
        url.push_str(&format!("&q={}", urlencoded(q)));
    }
    if let Some(cat) = category {
        url.push_str(&format!("&category={cat}"));
    }
    if let Some(s) = sort {
        url.push_str(&format!("&sort={s}"));
    }
    // Filters: the backend reads `min_rating` (1–5) and `max_price` (credits;
    // 0 = free-only). Omitted params mean "no filter".
    if let Some(r) = min_rating {
        url.push_str(&format!("&min_rating={r}"));
    }
    if let Some(p) = max_price {
        url.push_str(&format!("&max_price={p}"));
    }

    let response = super::client::get_json_raw(&url, None)?;
    response.json().map_err(|e| e.to_string())
}

/// Get asset detail by slug.
#[cfg(not(target_arch = "wasm32"))]
pub fn get_asset(slug: &str) -> Result<AssetDetail, String> {
    let url = format!("{API_BASE}/api/marketplace/detail/{slug}");

    let response = super::client::get_json_raw(&url, None)?;
    response.json().map_err(|e| e.to_string())
}

/// Download an asset (requires authentication).
/// Returns the download URL for the asset file.
#[cfg(not(target_arch = "wasm32"))]
pub fn download_asset(session: &AuthSession, asset_id: &str) -> Result<DownloadResponse, String> {
    let token = session.access_token.as_deref().ok_or("Not signed in")?;

    let url = format!("{API_BASE}/api/marketplace/{asset_id}/download");

    let response = super::client::get_json_raw(&url, Some(token))?;
    response.json().map_err(|e| e.to_string())
}

/// Public URL of an asset's file via the marketplace preview proxy.
///
/// The proxy serves the real file for **free** assets (and the watermarked
/// preview for paid ones) without authentication, so it's the path used to
/// preview a theme live or to grab a free asset when the user isn't signed in.
pub fn preview_file_url(asset_id: &str) -> String {
    format!("{API_BASE}/api/marketplace/{asset_id}/preview-file")
}

/// 256 MB. Enforced by the backend as the bytes arrive rather than after — this
/// is an asset file whose size the marketplace chose, not us.
const MAX_DOWNLOAD: u32 = 256 * 1024 * 1024;

/// Download the actual file bytes from a URL.
#[cfg(not(target_arch = "wasm32"))]
pub fn download_file(url: &str) -> Result<Vec<u8>, String> {
    let response = renzora::net::Request::get(url)
        .max_bytes(MAX_DOWNLOAD)
        .send()
        .map_err(|e| format!("Download failed: {e}"))?;
    if !response.is_ok() {
        return Err(format!("Download failed: HTTP {}", response.status));
    }
    Ok(response.body)
}

/// [`download_file`], reporting the running byte count as it arrives.
///
/// Streamed for the progress alone — the body is still accumulated whole,
/// because the installer writes it through an archive reader that wants all of
/// it. The count is what the status bar shows while a background install runs,
/// and it is the only real number available: nothing in the transport or the
/// catalogue reports a total, so there is no percentage to be had. See
/// `ShellStatusBar::Busy`.
#[cfg(not(target_arch = "wasm32"))]
pub fn download_file_progress(
    url: &str,
    on_bytes: &mut dyn FnMut(u64),
) -> Result<Vec<u8>, String> {
    let mut stream = renzora::net::Request::get(url)
        .max_bytes(MAX_DOWNLOAD)
        .send_stream()
        .map_err(|e| format!("Download failed: {e}"))?;

    let mut body: Vec<u8> = Vec::new();
    for chunk in &mut stream {
        body.extend_from_slice(&chunk.data);
        on_bytes(body.len() as u64);
    }
    // A stream that fails halfway delivers what it got and then stops, which is
    // indistinguishable from success until asked — so ask, before treating a
    // truncated body as a file.
    if let Some(e) = stream.error() {
        return Err(format!("Download failed: {e}"));
    }
    if stream.status() != 0 && !(200..300).contains(&stream.status()) {
        return Err(format!("Download failed: HTTP {}", stream.status()));
    }
    Ok(body)
}

/// One entry in an asset's preview-media gallery. `media_type` is one of
/// `"image"`, `"video"`, or `"audio"`; the two optional fields default so an
/// item that omits them (e.g. an image with no separate thumbnail) still parses.
#[derive(Deserialize, Clone, Debug)]
pub struct MediaItem {
    pub id: String,
    pub media_type: String,
    pub url: String,
    #[serde(default)]
    pub thumbnail_url: Option<String>,
    #[serde(default)]
    pub sort_order: i64,
}

/// Fetch an asset's preview-media gallery (images / video / audio). Public
/// endpoint — no auth. The server returns items in no guaranteed order, so we
/// sort by `sort_order` here once, keeping every consumer's rendering stable.
#[cfg(not(target_arch = "wasm32"))]
pub fn get_media(asset_id: &str) -> Result<Vec<MediaItem>, String> {
    let url = format!("{API_BASE}/api/marketplace/{asset_id}/media");

    let mut items: Vec<MediaItem> = super::client::get_json_raw(&url, None)?
        .json()
        .map_err(|e| e.to_string())?;
    items.sort_by_key(|m| m.sort_order);
    Ok(items)
}

/// One downloadable file of an asset (a multi-format model ships FBX + GLB + OBJ,
/// a pack ships many). `download_url` is a presigned URL (present for free assets
/// / owned paid assets).
#[derive(Debug, Deserialize, Clone)]
pub struct AssetFileInfo {
    pub id: String,
    #[serde(default)]
    pub original_filename: String,
    #[serde(default)]
    pub mime_type: String,
    #[serde(default)]
    pub sort_order: i64,
    #[serde(default)]
    pub preview_url: Option<String>,
    #[serde(default)]
    pub download_url: Option<String>,
}

/// List an asset's files. Public endpoint — no auth. Used to pick the right
/// format for a native preview (e.g. the `.glb`, since the `preview-file` proxy
/// serves only the *first* file — often an FBX Bevy can't load).
#[cfg(not(target_arch = "wasm32"))]
pub fn get_asset_files(asset_id: &str) -> Result<Vec<AssetFileInfo>, String> {
    let url = format!("{API_BASE}/api/marketplace/{asset_id}/asset-files");
    let response = super::client::get_json_raw(&url, None)?;
    response.json().map_err(|e| e.to_string())
}

/// List marketplace categories.
#[cfg(not(target_arch = "wasm32"))]
pub fn list_categories() -> Result<Vec<Category>, String> {
    let url = format!("{API_BASE}/api/marketplace/categories");

    let response = super::client::get_json_raw(&url, None)?;
    response.json().map_err(|e| e.to_string())
}

/// Purchase an asset with credits (requires authentication).
#[cfg(not(target_arch = "wasm32"))]
pub fn purchase_asset(session: &AuthSession, asset_id: &str) -> Result<PurchaseResponse, String> {
    let token = session.access_token.as_deref().ok_or("Not signed in")?;

    let url = format!("{API_BASE}/api/credits/purchase");
    let body = serde_json::json!({ "asset_id": asset_id });
        let response = super::client::post_json_raw(&url, &body, Some(token))?;
    response.json().map_err(|e| e.to_string())
}

/// Get the user's purchased/owned assets.
#[cfg(not(target_arch = "wasm32"))]
pub fn get_my_assets(session: &AuthSession) -> Result<MarketplaceListResponse, String> {
    let token = session.access_token.as_deref().ok_or("Not signed in")?;

    let url = format!("{API_BASE}/api/marketplace/purchased");

    let response = super::client::get_json_raw(&url, Some(token))?;
    response.json().map_err(|e| e.to_string())
}

/// Get the user's credit balance.
#[cfg(not(target_arch = "wasm32"))]
pub fn get_credit_balance(session: &AuthSession) -> Result<CreditBalanceResponse, String> {
    let token = session.access_token.as_deref().ok_or("Not signed in")?;

    let url = format!("{API_BASE}/api/credits/balance");

    let response = super::client::get_json_raw(&url, Some(token))?;
    response.json().map_err(|e| e.to_string())
}

#[derive(Debug, Deserialize, Clone)]
pub struct Category {
    pub id: String,
    pub name: String,
    pub slug: String,
    // Games categories may omit these; default so both endpoints parse.
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub icon: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct PurchaseResponse {
    pub message: String,
    pub new_balance: i64,
}

#[derive(Debug, Deserialize, Clone)]
pub struct CreditBalanceResponse {
    pub balance: i64,
}

// ── Comments & Ratings ──

#[derive(Debug, Deserialize, Clone)]
pub struct AssetComment {
    pub id: String,
    pub user_name: String,
    pub content: String,
    pub created_at: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct CommentsResponse {
    pub comments: Vec<AssetComment>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct AssetRating {
    pub average: f32,
    pub count: i64,
    pub user_rating: Option<i32>,
}

/// Get comments for an asset (keyed by asset **id**, not slug).
#[cfg(not(target_arch = "wasm32"))]
pub fn get_comments(asset_id: &str) -> Result<CommentsResponse, String> {
    let url = format!("{API_BASE}/api/marketplace/{asset_id}/comments");

    let response = super::client::get_json_raw(&url, None)?;
    response.json().map_err(|e| e.to_string())
}

/// Post a comment on an asset (requires authentication).
#[cfg(not(target_arch = "wasm32"))]
pub fn post_comment(
    session: &AuthSession,
    asset_id: &str,
    content: &str,
) -> Result<AssetComment, String> {
    let token = session.access_token.as_deref().ok_or("Not signed in")?;

    let url = format!("{API_BASE}/api/marketplace/{asset_id}/comments");
    let body = serde_json::json!({ "content": content });
        let response = super::client::post_json_raw(&url, &body, Some(token))?;
    response.json().map_err(|e| e.to_string())
}

/// Get an asset's rating aggregate. Ratings are "reviews", keyed by asset **id**
/// at `/{id}/reviews`, which returns `{ rating_avg, rating_count, reviews }`.
/// Best-effort and NON-FATAL: any failure yields an empty rating rather than an
/// error (so an unrated asset never surfaces a "404" toast in the overlay).
#[cfg(not(target_arch = "wasm32"))]
pub fn get_rating(asset_id: &str, _session: Option<&AuthSession>) -> Result<AssetRating, String> {
    let empty = AssetRating { average: 0.0, count: 0, user_rating: None };
    let url = format!("{API_BASE}/api/marketplace/{asset_id}/reviews");
    let Ok(response) = renzora::net::Request::get(&url).send() else { return Ok(empty) };
    let Ok(v) = response.json::<serde_json::Value>() else { return Ok(empty) };
    let average = v.get("rating_avg").and_then(|x| x.as_f64()).unwrap_or(0.0) as f32;
    let count = v.get("rating_count").and_then(|x| x.as_i64()).unwrap_or(0);
    Ok(AssetRating { average, count, user_rating: None })
}

/// Submit or update your rating for an asset (requires authentication). Ratings
/// are reviews: `POST /{id}/reviews` with `{ rating }`. The endpoint returns only
/// `{ id, message }`, so we re-fetch the aggregate to reflect the new average and
/// echo back the rating the user just set as their own.
#[cfg(not(target_arch = "wasm32"))]
pub fn post_rating(session: &AuthSession, asset_id: &str, rating: i32) -> Result<AssetRating, String> {
    let token = session.access_token.as_deref().ok_or("Not signed in")?;

    let url = format!("{API_BASE}/api/marketplace/{asset_id}/reviews");
    let body = serde_json::json!({ "rating": rating });

    // Non-2xx (e.g. "you must own this asset") must surface as Err, which the
    // overlay toasts. Checked explicitly: `renzora_net` treats an HTTP error
    // status as a successful request — that is what lets every other call site
    // here read the server's own `{"error": …}` — so the status is ours to
    // interpret.
    let response = super::client::post_json_raw(&url, &body, Some(token))?;
    if !response.is_ok() {
        return Err(response
            .json::<serde_json::Value>()
            .err()
            .map(|e| e.to_string())
            .unwrap_or_else(|| format!("HTTP {}", response.status)));
    }

    // Re-read the aggregate; stamp our own rating so the stars stay filled.
    let mut agg = get_rating(asset_id, Some(session))?;
    agg.user_rating = Some(rating);
    Ok(agg)
}

/// Simple percent-encoding for query parameters.
fn urlencoded(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char);
            }
            b' ' => out.push('+'),
            _ => {
                out.push('%');
                out.push_str(&format!("{b:02X}"));
            }
        }
    }
    out
}
