//! renzora.com account & profile management API client.
//!
//! Profile edits, avatar/banner uploads, email/password changes, social
//! connections, connected-app grants, communication preferences, and account
//! deletion. Native-only blocking HTTP on worker threads (the crate convention).
//!
//! Some of these hit endpoints that are new to the backend (change-password,
//! delete-account, communication prefs, banner upload); they degrade to the
//! server's `{"error":...}` message if a deployment predates them.

use serde::{Deserialize, Serialize};

#[cfg(not(target_arch = "wasm32"))]
use crate::client::{
    api_base, delete_json, get_json, post_json, put_json, put_multipart, require_token,
};
use crate::session::AuthSession;

// ── Profile edit ─────────────────────────────────────────────────────────────

/// Partial profile update — omit a field to leave it unchanged (the server
/// COALESCEs). Matches `PUT /api/auth/me`'s `UpdateProfileRequest`.
#[derive(Serialize, Default, Clone)]
pub struct ProfileUpdate {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bio: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub website: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub profile_color: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub banner_color: Option<String>,
}

/// Save profile fields (bio/location/website/colors, and — inline — email/
/// username). `PUT /api/auth/me`.
#[cfg(not(target_arch = "wasm32"))]
pub fn update_profile(session: &AuthSession, update: &ProfileUpdate) -> Result<serde_json::Value, String> {
    let token = require_token(session)?;
    put_json(&format!("{}/api/auth/me", api_base()), update, Some(token))
}

// ── Avatar / banner (cover) photo ────────────────────────────────────────────

#[derive(Deserialize)]
pub struct AvatarUrlResponse {
    pub avatar_url: String,
}

#[derive(Deserialize)]
pub struct BannerUrlResponse {
    pub banner_url: String,
}

/// Upload an avatar photo (multipart field `avatar`, ≤2 MB). `PUT /api/profiles/avatar`.
#[cfg(not(target_arch = "wasm32"))]
pub fn upload_avatar(
    session: &AuthSession,
    filename: &str,
    content_type: &str,
    bytes: &[u8],
) -> Result<AvatarUrlResponse, String> {
    let token = require_token(session)?;
    put_multipart(
        &format!("{}/api/profiles/avatar", api_base()),
        "avatar",
        filename,
        content_type,
        bytes,
        token,
    )
}

/// Upload a cover/banner photo (multipart field `banner`, ≤4 MB). `PUT /api/profiles/banner`.
#[cfg(not(target_arch = "wasm32"))]
pub fn upload_banner(
    session: &AuthSession,
    filename: &str,
    content_type: &str,
    bytes: &[u8],
) -> Result<BannerUrlResponse, String> {
    let token = require_token(session)?;
    put_multipart(
        &format!("{}/api/profiles/banner", api_base()),
        "banner",
        filename,
        content_type,
        bytes,
        token,
    )
}

/// Clear the cover/banner photo. `DELETE /api/profiles/banner`.
#[cfg(not(target_arch = "wasm32"))]
pub fn remove_banner(session: &AuthSession) -> Result<serde_json::Value, String> {
    let token = require_token(session)?;
    delete_json(&format!("{}/api/profiles/banner", api_base()), Some(token))
}

// ── Email / password / delete ────────────────────────────────────────────────

/// Change the account password. `POST /api/auth/change-password`.
#[cfg(not(target_arch = "wasm32"))]
pub fn change_password(session: &AuthSession, current: &str, new: &str) -> Result<serde_json::Value, String> {
    let token = require_token(session)?;
    post_json(
        &format!("{}/api/auth/change-password", api_base()),
        &serde_json::json!({ "current_password": current, "new_password": new }),
        Some(token),
    )
}

/// Permanently delete the account (password-confirmed). `POST /api/auth/delete-account`.
#[cfg(not(target_arch = "wasm32"))]
pub fn delete_account(session: &AuthSession, password: &str) -> Result<serde_json::Value, String> {
    let token = require_token(session)?;
    post_json(
        &format!("{}/api/auth/delete-account", api_base()),
        &serde_json::json!({ "password": password }),
        Some(token),
    )
}

// ── Communication preferences ────────────────────────────────────────────────

/// Email/notification category toggles. `GET`/`PUT /api/user/communication`.
#[derive(Serialize, Deserialize, Clone, Copy, Debug)]
pub struct CommunicationPrefs {
    pub product_updates: bool,
    pub marketplace: bool,
    pub comments: bool,
    pub security: bool,
}

impl Default for CommunicationPrefs {
    fn default() -> Self {
        // Mirrors the server column defaults (all opted-in).
        Self { product_updates: true, marketplace: true, comments: true, security: true }
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub fn get_communication(session: &AuthSession) -> Result<CommunicationPrefs, String> {
    let token = require_token(session)?;
    get_json(&format!("{}/api/user/communication", api_base()), Some(token))
}

#[cfg(not(target_arch = "wasm32"))]
pub fn update_communication(
    session: &AuthSession,
    prefs: &CommunicationPrefs,
) -> Result<CommunicationPrefs, String> {
    let token = require_token(session)?;
    put_json(&format!("{}/api/user/communication", api_base()), prefs, Some(token))
}

// ── Social connections (profile social links) ────────────────────────────────

/// The recognized social platforms (value sent as `platform`, plus a label and
/// a Phosphor icon name for the UI).
pub const SOCIAL_PLATFORMS: &[(&str, &str, &str)] = &[
    ("discord", "Discord", "discord-logo"),
    ("twitch", "Twitch", "twitch-logo"),
    ("youtube", "YouTube", "youtube-logo"),
    ("twitter", "Twitter / X", "x-logo"),
    ("github", "GitHub", "github-logo"),
    ("steam", "Steam", "steam-logo"),
    ("kick", "Kick", "monitor-play"),
    ("xbox", "Xbox", "xbox-logo"),
    ("playstation", "PlayStation", "playstation-logo"),
    ("epic", "Epic Games", "game-controller"),
];

#[derive(Deserialize, Clone, Debug)]
pub struct SocialConnection {
    pub platform: String,
    #[serde(default)]
    pub platform_username: String,
    #[serde(default)]
    pub platform_url: Option<String>,
    #[serde(default)]
    pub verified: bool,
}

/// List the signed-in user's social connections. `GET /api/profiles/connections`.
#[cfg(not(target_arch = "wasm32"))]
pub fn list_connections(session: &AuthSession) -> Result<Vec<SocialConnection>, String> {
    let token = require_token(session)?;
    get_json(&format!("{}/api/profiles/connections", api_base()), Some(token))
}

#[derive(Serialize)]
struct AddConnectionBody<'a> {
    platform: &'a str,
    username: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    url: Option<&'a str>,
}

/// Add/update a social connection (manual entry → unverified).
/// `POST /api/profiles/connections`.
#[cfg(not(target_arch = "wasm32"))]
pub fn add_connection(
    session: &AuthSession,
    platform: &str,
    username: &str,
    url: Option<&str>,
) -> Result<serde_json::Value, String> {
    let token = require_token(session)?;
    post_json(
        &format!("{}/api/profiles/connections", api_base()),
        &AddConnectionBody { platform, username, url },
        Some(token),
    )
}

/// Remove a social connection by platform. `DELETE /api/profiles/connections/:platform`.
#[cfg(not(target_arch = "wasm32"))]
pub fn remove_connection(session: &AuthSession, platform: &str) -> Result<serde_json::Value, String> {
    let token = require_token(session)?;
    delete_json(
        &format!("{}/api/profiles/connections/{}", api_base(), platform),
        Some(token),
    )
}

// ── Connected apps (authorized game-service grants) ──────────────────────────

#[derive(Deserialize, Clone, Debug)]
pub struct AppGrant {
    pub app_id: String,
    #[serde(default)]
    pub app_name: String,
    #[serde(default)]
    pub app_icon_url: Option<String>,
    #[serde(default)]
    pub website_url: Option<String>,
    #[serde(default)]
    pub scopes_granted: Vec<String>,
    #[serde(default)]
    pub granted_at: String,
}

/// Apps you've authorized to access your account. `GET /api/gameservices/grants`.
#[cfg(not(target_arch = "wasm32"))]
pub fn list_app_grants(session: &AuthSession) -> Result<Vec<AppGrant>, String> {
    let token = require_token(session)?;
    get_json(&format!("{}/api/gameservices/grants", api_base()), Some(token))
}

/// Revoke an app's access. `DELETE /api/gameservices/grants/:app_id`.
#[cfg(not(target_arch = "wasm32"))]
pub fn revoke_app_grant(session: &AuthSession, app_id: &str) -> Result<serde_json::Value, String> {
    let token = require_token(session)?;
    delete_json(
        &format!("{}/api/gameservices/grants/{}", api_base(), app_id),
        Some(token),
    )
}
