//! Social feed API client — posts, likes, comments.
//!
//! Blocking HTTP calls designed to run on background threads.

use serde::Deserialize;

use crate::client::api_base;
#[cfg(not(target_arch = "wasm32"))]
use crate::client::{delete_json, get_json, post_json, require_token};
use crate::session::AuthSession;

// ── Types ──

#[derive(Debug, Deserialize, Clone, Default)]
pub struct Reaction {
    #[serde(default)]
    pub icon: String,
    #[serde(default)]
    pub count: i64,
    #[serde(default)]
    pub reacted: bool,
}

#[derive(Debug, Deserialize, Clone)]
pub struct FeedPost {
    pub id: String,
    pub user_id: String,
    #[serde(default)]
    pub username: String,
    #[serde(default)]
    pub avatar_url: Option<String>,
    #[serde(default)]
    pub role: String,
    #[serde(default)]
    pub body: String,
    #[serde(default)]
    pub media_urls: Vec<String>,
    /// `public`, `followers`, or `friends`.
    #[serde(default)]
    pub visibility: String,
    #[serde(default)]
    pub like_count: i64,
    #[serde(default)]
    pub comment_count: i64,
    #[serde(default)]
    pub is_liked: bool,
    #[serde(default)]
    pub reactions: Vec<Reaction>,
    /// The channel the post is in, if any.
    #[serde(default)]
    pub channel_slug: Option<String>,
    #[serde(default)]
    pub channel_name: Option<String>,
    #[serde(default)]
    pub channel_icon: Option<String>,
    /// Moderation: a hidden post is only returned to its author, who can request
    /// a staff review.
    #[serde(default)]
    pub hidden: bool,
    #[serde(default)]
    pub review_requested: bool,
    #[serde(default)]
    pub created_at: String,
}

/// A feed channel (topic room).
#[derive(Debug, Deserialize, Clone)]
pub struct Channel {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub slug: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub icon: String,
    #[serde(default)]
    pub post_count: i64,
}

#[derive(Debug, Deserialize, Clone)]
pub struct FeedComment {
    pub id: String,
    pub post_id: String,
    pub user_id: String,
    #[serde(default)]
    pub username: String,
    #[serde(default)]
    pub avatar_url: Option<String>,
    #[serde(default)]
    pub body: String,
    #[serde(default)]
    pub parent_id: Option<String>,
    #[serde(default)]
    pub like_count: i64,
    #[serde(default)]
    pub is_liked: bool,
    #[serde(default)]
    pub created_at: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct CreatedResponse {
    pub id: String,
    #[serde(default)]
    pub created_at: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct LikeResponse {
    pub liked: bool,
}

// ── API calls ──

/// Get the home feed (own + followed users' posts). `before` is a post-id cursor.
#[cfg(not(target_arch = "wasm32"))]
pub fn get_feed(
    session: &AuthSession,
    before: Option<&str>,
    limit: u32,
    channel: Option<&str>,
) -> Result<Vec<FeedPost>, String> {
    let token = require_token(session)?;
    let mut url = format!("{}/api/feed/feed?limit={limit}", api_base());
    if let Some(b) = before {
        url.push_str(&format!("&before={b}"));
    }
    if let Some(c) = channel.filter(|c| !c.is_empty()) {
        url.push_str(&format!("&channel={c}"));
    }
    get_json(&url, Some(token))
}

/// Create a post. `visibility` is `public`, `followers`, or `friends`; `channel`
/// is an optional channel slug to post into.
#[cfg(not(target_arch = "wasm32"))]
pub fn create_post(
    session: &AuthSession,
    body: &str,
    visibility: &str,
    media_urls: &[String],
    channel: Option<&str>,
) -> Result<CreatedResponse, String> {
    let token = require_token(session)?;
    post_json(
        &format!("{}/api/feed/posts", api_base()),
        &serde_json::json!({ "body": body, "visibility": visibility, "media_urls": media_urls, "channel": channel }),
        Some(token),
    )
}

/// List the live (approved) feed channels.
#[cfg(not(target_arch = "wasm32"))]
pub fn list_channels(session: &AuthSession) -> Result<Vec<Channel>, String> {
    let token = require_token(session)?;
    get_json(&format!("{}/api/feed/channels", api_base()), Some(token))
}

/// Suggest a new channel (created unapproved; an admin approves it before it
/// goes live).
#[cfg(not(target_arch = "wasm32"))]
pub fn suggest_channel(
    session: &AuthSession,
    name: &str,
    description: &str,
    icon: &str,
) -> Result<serde_json::Value, String> {
    let token = require_token(session)?;
    post_json(
        &format!("{}/api/feed/channels/suggest", api_base()),
        &serde_json::json!({ "name": name, "description": description, "icon": icon }),
        Some(token),
    )
}

/// Report a post. Enough distinct reports auto-hide it pending review.
#[cfg(not(target_arch = "wasm32"))]
pub fn report_post(session: &AuthSession, post_id: &str, reason: &str) -> Result<serde_json::Value, String> {
    let token = require_token(session)?;
    post_json(
        &format!("{}/api/feed/posts/{post_id}/report", api_base()),
        &serde_json::json!({ "reason": reason }),
        Some(token),
    )
}

/// Ask staff to review one of your own hidden posts.
#[cfg(not(target_arch = "wasm32"))]
pub fn request_review(session: &AuthSession, post_id: &str) -> Result<serde_json::Value, String> {
    let token = require_token(session)?;
    post_json(
        &format!("{}/api/feed/posts/{post_id}/request-review", api_base()),
        &serde_json::json!({}),
        Some(token),
    )
}

/// Delete one of your own posts.
#[cfg(not(target_arch = "wasm32"))]
pub fn delete_post(session: &AuthSession, post_id: &str) -> Result<serde_json::Value, String> {
    let token = require_token(session)?;
    delete_json(&format!("{}/api/feed/posts/{post_id}", api_base()), Some(token))
}

/// Toggle a like on a post.
#[cfg(not(target_arch = "wasm32"))]
pub fn like_post(session: &AuthSession, post_id: &str) -> Result<LikeResponse, String> {
    let token = require_token(session)?;
    post_json(
        &format!("{}/api/feed/posts/{post_id}/like", api_base()),
        &serde_json::json!({}),
        Some(token),
    )
}

/// List comments on a post.
#[cfg(not(target_arch = "wasm32"))]
pub fn get_comments(
    session: &AuthSession,
    post_id: &str,
    limit: u32,
    offset: u32,
) -> Result<Vec<FeedComment>, String> {
    let token = require_token(session)?;
    get_json(
        &format!("{}/api/feed/posts/{post_id}/comments?limit={limit}&offset={offset}", api_base()),
        Some(token),
    )
}

/// Comment on a post (optionally threaded under `parent_id`).
#[cfg(not(target_arch = "wasm32"))]
pub fn post_comment(
    session: &AuthSession,
    post_id: &str,
    body: &str,
    parent_id: Option<&str>,
) -> Result<CreatedResponse, String> {
    let token = require_token(session)?;
    post_json(
        &format!("{}/api/feed/posts/{post_id}/comments", api_base()),
        &serde_json::json!({ "body": body, "parent_id": parent_id }),
        Some(token),
    )
}

/// Toggle a like on a comment.
#[cfg(not(target_arch = "wasm32"))]
pub fn like_comment(session: &AuthSession, comment_id: &str) -> Result<LikeResponse, String> {
    let token = require_token(session)?;
    post_json(
        &format!("{}/api/feed/comments/{comment_id}/like", api_base()),
        &serde_json::json!({}),
        Some(token),
    )
}

/// A user's posts (public only unless it's you).
#[cfg(not(target_arch = "wasm32"))]
pub fn get_user_posts(
    session: &AuthSession,
    username: &str,
    before: Option<&str>,
    limit: u32,
) -> Result<Vec<FeedPost>, String> {
    let token = require_token(session)?;
    let mut url = format!("{}/api/feed/users/{username}/posts?limit={limit}", api_base());
    if let Some(b) = before {
        url.push_str(&format!("&before={b}"));
    }
    get_json(&url, Some(token))
}


#[derive(Debug, Deserialize, Clone)]
pub struct UploadResponse {
    pub url: String,
}

/// Upload an image for a feed post; returns its URL.
#[cfg(not(target_arch = "wasm32"))]
pub fn upload_image(
    session: &AuthSession,
    filename: &str,
    content_type: &str,
    bytes: &[u8],
) -> Result<UploadResponse, String> {
    let token = require_token(session)?;
    crate::client::post_multipart(
        &format!("{}/api/feed/upload", api_base()),
        "image",
        filename,
        content_type,
        bytes,
        token,
    )
}

/// Toggle a phosphor-icon reaction on a feed post.
#[cfg(not(target_arch = "wasm32"))]
pub fn react_to_post(session: &AuthSession, post_id: &str, icon: &str) -> Result<serde_json::Value, String> {
    let token = require_token(session)?;
    post_json(
        &format!("{}/api/feed/posts/{post_id}/reactions", api_base()),
        &serde_json::json!({ "icon": icon }),
        Some(token),
    )
}
