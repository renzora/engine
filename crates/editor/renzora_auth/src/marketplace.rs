//! Marketplace API client for the Renzora engine.
//!
//! Provides blocking HTTP calls to browse, search, and download marketplace assets.
//! All functions are designed to run on background threads.

use serde::{Deserialize, Serialize};

#[cfg(not(target_arch = "wasm32"))]
use std::io::Read;

use crate::session::AuthSession;

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
}

// ── API calls (blocking) ──

/// Search/browse marketplace assets.
#[cfg(not(target_arch = "wasm32"))]
pub fn list_assets(
    query: Option<&str>,
    category: Option<&str>,
    sort: Option<&str>,
    page: u32,
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

    let response = ureq::get(&url)
        .call()
        .map_err(|e| format!("Request failed: {e}"))?;

    let body = response
        .into_body()
        .read_to_string()
        .map_err(|e| format!("Failed to read response: {e}"))?;

    serde_json::from_str(&body).map_err(|e| format!("Failed to parse response: {e}"))
}

/// Get asset detail by slug.
#[cfg(not(target_arch = "wasm32"))]
pub fn get_asset(slug: &str) -> Result<AssetDetail, String> {
    let url = format!("{API_BASE}/api/marketplace/detail/{slug}");

    let response = ureq::get(&url)
        .call()
        .map_err(|e| format!("Request failed: {e}"))?;

    let body = response
        .into_body()
        .read_to_string()
        .map_err(|e| format!("Failed to read response: {e}"))?;

    serde_json::from_str(&body).map_err(|e| format!("Failed to parse response: {e}"))
}

/// Download an asset (requires authentication).
/// Returns the download URL for the asset file.
#[cfg(not(target_arch = "wasm32"))]
pub fn download_asset(session: &AuthSession, asset_id: &str) -> Result<DownloadResponse, String> {
    let token = session
        .access_token
        .as_deref()
        .ok_or("Not signed in")?;

    let url = format!("{API_BASE}/api/marketplace/{asset_id}/download");

    let response = ureq::get(&url)
        .header("Authorization", &format!("Bearer {token}"))
        .call()
        .map_err(|e| format!("Request failed: {e}"))?;

    let body = response
        .into_body()
        .read_to_string()
        .map_err(|e| format!("Failed to read response: {e}"))?;

    serde_json::from_str(&body).map_err(|e| format!("Failed to parse response: {e}"))
}

/// Download the actual file bytes from a URL.
#[cfg(not(target_arch = "wasm32"))]
pub fn download_file(url: &str) -> Result<Vec<u8>, String> {
    let response = ureq::get(url)
        .call()
        .map_err(|e| format!("Download failed: {e}"))?;

    let mut bytes = Vec::new();
    response
        .into_body()
        .into_reader()
        .take(256 * 1024 * 1024) // 256 MB limit
        .read_to_end(&mut bytes)
        .map_err(|e| format!("Failed to read file: {e}"))?;

    Ok(bytes)
}

/// List marketplace categories.
#[cfg(not(target_arch = "wasm32"))]
pub fn list_categories() -> Result<Vec<Category>, String> {
    let url = format!("{API_BASE}/api/marketplace/categories");

    let response = ureq::get(&url)
        .call()
        .map_err(|e| format!("Request failed: {e}"))?;

    let body = response
        .into_body()
        .read_to_string()
        .map_err(|e| format!("Failed to read response: {e}"))?;

    serde_json::from_str(&body).map_err(|e| format!("Failed to parse response: {e}"))
}

/// Purchase an asset with credits (requires authentication).
#[cfg(not(target_arch = "wasm32"))]
pub fn purchase_asset(session: &AuthSession, asset_id: &str) -> Result<PurchaseResponse, String> {
    let token = session
        .access_token
        .as_deref()
        .ok_or("Not signed in")?;

    let url = format!("{API_BASE}/api/credits/purchase");
    let body = serde_json::json!({ "asset_id": asset_id });
    let json = serde_json::to_string(&body).map_err(|e| e.to_string())?;

    let response = ureq::post(&url)
        .header("Authorization", &format!("Bearer {token}"))
        .header("Content-Type", "application/json")
        .send(json.as_bytes())
        .map_err(|e| format!("Request failed: {e}"))?;

    let body_str = response
        .into_body()
        .read_to_string()
        .map_err(|e| format!("Failed to read response: {e}"))?;

    serde_json::from_str(&body_str).map_err(|e| format!("Failed to parse response: {e}"))
}

/// Get the user's purchased/owned assets.
#[cfg(not(target_arch = "wasm32"))]
pub fn get_my_assets(session: &AuthSession) -> Result<MarketplaceListResponse, String> {
    let token = session
        .access_token
        .as_deref()
        .ok_or("Not signed in")?;

    let url = format!("{API_BASE}/api/marketplace/purchased");

    let response = ureq::get(&url)
        .header("Authorization", &format!("Bearer {token}"))
        .call()
        .map_err(|e| format!("Request failed: {e}"))?;

    let body = response
        .into_body()
        .read_to_string()
        .map_err(|e| format!("Failed to read response: {e}"))?;

    serde_json::from_str(&body).map_err(|e| format!("Failed to parse response: {e}"))
}

/// Get the user's credit balance.
#[cfg(not(target_arch = "wasm32"))]
pub fn get_credit_balance(session: &AuthSession) -> Result<CreditBalanceResponse, String> {
    let token = session
        .access_token
        .as_deref()
        .ok_or("Not signed in")?;

    let url = format!("{API_BASE}/api/credits/balance");

    let response = ureq::get(&url)
        .header("Authorization", &format!("Bearer {token}"))
        .call()
        .map_err(|e| format!("Request failed: {e}"))?;

    let body = response
        .into_body()
        .read_to_string()
        .map_err(|e| format!("Failed to read response: {e}"))?;

    serde_json::from_str(&body).map_err(|e| format!("Failed to parse response: {e}"))
}

#[derive(Debug, Deserialize, Clone)]
pub struct Category {
    pub id: String,
    pub name: String,
    pub slug: String,
    pub description: String,
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

/// Get comments for an asset.
#[cfg(not(target_arch = "wasm32"))]
pub fn get_comments(slug: &str) -> Result<CommentsResponse, String> {
    let url = format!("{API_BASE}/api/marketplace/detail/{slug}/comments");

    let response = ureq::get(&url)
        .call()
        .map_err(|e| format!("Request failed: {e}"))?;

    let body = response
        .into_body()
        .read_to_string()
        .map_err(|e| format!("Failed to read response: {e}"))?;

    serde_json::from_str(&body).map_err(|e| format!("Failed to parse response: {e}"))
}

/// Post a comment on an asset (requires authentication).
#[cfg(not(target_arch = "wasm32"))]
pub fn post_comment(
    session: &AuthSession,
    slug: &str,
    content: &str,
) -> Result<AssetComment, String> {
    let token = session
        .access_token
        .as_deref()
        .ok_or("Not signed in")?;

    let url = format!("{API_BASE}/api/marketplace/detail/{slug}/comments");
    let body = serde_json::json!({ "content": content });
    let json = serde_json::to_string(&body).map_err(|e| e.to_string())?;

    let response = ureq::post(&url)
        .header("Authorization", &format!("Bearer {token}"))
        .header("Content-Type", "application/json")
        .send(json.as_bytes())
        .map_err(|e| format!("Request failed: {e}"))?;

    let body_str = response
        .into_body()
        .read_to_string()
        .map_err(|e| format!("Failed to read response: {e}"))?;

    serde_json::from_str(&body_str).map_err(|e| format!("Failed to parse response: {e}"))
}

/// Get the average rating and user's own rating for an asset.
#[cfg(not(target_arch = "wasm32"))]
pub fn get_rating(slug: &str, session: Option<&AuthSession>) -> Result<AssetRating, String> {
    let url = format!("{API_BASE}/api/marketplace/detail/{slug}/rating");

    let mut req = ureq::get(&url);
    if let Some(s) = session {
        if let Some(token) = s.access_token.as_deref() {
            req = req.header("Authorization", &format!("Bearer {token}"));
        }
    }

    let response = req
        .call()
        .map_err(|e| format!("Request failed: {e}"))?;

    let body = response
        .into_body()
        .read_to_string()
        .map_err(|e| format!("Failed to read response: {e}"))?;

    serde_json::from_str(&body).map_err(|e| format!("Failed to parse response: {e}"))
}

/// Submit or update a rating for an asset (requires authentication).
#[cfg(not(target_arch = "wasm32"))]
pub fn post_rating(
    session: &AuthSession,
    slug: &str,
    rating: i32,
) -> Result<AssetRating, String> {
    let token = session
        .access_token
        .as_deref()
        .ok_or("Not signed in")?;

    let url = format!("{API_BASE}/api/marketplace/detail/{slug}/rating");
    let body = serde_json::json!({ "rating": rating });
    let json = serde_json::to_string(&body).map_err(|e| e.to_string())?;

    let response = ureq::post(&url)
        .header("Authorization", &format!("Bearer {token}"))
        .header("Content-Type", "application/json")
        .send(json.as_bytes())
        .map_err(|e| format!("Request failed: {e}"))?;

    let body_str = response
        .into_body()
        .read_to_string()
        .map_err(|e| format!("Failed to read response: {e}"))?;

    serde_json::from_str(&body_str).map_err(|e| format!("Failed to parse response: {e}"))
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
