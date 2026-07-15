//! Social API client — friends, presence, profiles, notifications, privacy.
//!
//! Blocking HTTP calls designed to run on background threads.

use serde::{Deserialize, Serialize};

use crate::client::{api_base, urlencoded};
#[cfg(not(target_arch = "wasm32"))]
use crate::client::{delete_json, get_json, post_json, put_json, require_token};
use crate::session::AuthSession;

// ── Types ──

#[derive(Debug, Deserialize, Clone)]
pub struct FriendEntry {
    pub user_id: String,
    pub username: String,
    #[serde(default)]
    pub avatar_url: Option<String>,
    #[serde(default)]
    pub since: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct FriendRequestEntry {
    pub from_user_id: String,
    pub username: String,
    #[serde(default)]
    pub avatar_url: Option<String>,
    #[serde(default)]
    pub sent_at: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct PresenceEntry {
    pub user_id: String,
    pub online: bool,
}

#[derive(Debug, Deserialize, Clone)]
pub struct UserSearchResult {
    pub username: String,
    #[serde(default)]
    pub role: String,
    #[serde(default)]
    pub avatar_url: Option<String>,
}

#[derive(Debug, Deserialize, Clone, Default)]
pub struct ProfileBadge {
    #[serde(default)]
    pub slug: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub icon: String,
    #[serde(default)]
    pub color: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct PublicProfile {
    pub id: String,
    pub username: String,
    #[serde(default)]
    pub role: String,
    #[serde(default)]
    pub bio: Option<String>,
    #[serde(default)]
    pub website: Option<String>,
    #[serde(default)]
    pub location: Option<String>,
    #[serde(default)]
    pub profile_color: Option<String>,
    #[serde(default)]
    pub banner_color: Option<String>,
    /// Cover/banner photo URL (newer backend; falls back to `banner_color`).
    #[serde(default)]
    pub banner_url: Option<String>,
    #[serde(default)]
    pub avatar_url: Option<String>,
    /// Social links shown on the profile.
    #[serde(default)]
    pub connections: Vec<crate::account::SocialConnection>,
    #[serde(default)]
    pub follower_count: i64,
    #[serde(default)]
    pub following_count: i64,
    #[serde(default)]
    pub post_count: i64,
    #[serde(default)]
    pub total_xp: i64,
    #[serde(default)]
    pub level: i64,
    #[serde(default)]
    pub seller_level: i64,
    #[serde(default)]
    pub is_following: bool,
    #[serde(default)]
    pub badges: Vec<ProfileBadge>,
    #[serde(default)]
    pub asset_count: i64,
    #[serde(default)]
    pub created_at: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ProfileAsset {
    pub id: String,
    pub name: String,
    pub slug: String,
    #[serde(default)]
    pub category: String,
    #[serde(default)]
    pub price_credits: i64,
    #[serde(default)]
    pub thumbnail_url: Option<String>,
    #[serde(default)]
    pub downloads: i64,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ProfileAssetsResponse {
    pub assets: Vec<ProfileAsset>,
    #[serde(default)]
    pub total: i64,
}

/// A notification row — also the exact payload of the WS `notification` event.
#[derive(Debug, Deserialize, Clone)]
pub struct NotificationRow {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub title: String,
    #[serde(default)]
    pub body: String,
    #[serde(default)]
    pub link: Option<String>,
    #[serde(default)]
    pub read: bool,
    #[serde(default)]
    pub actor_avatar_url: Option<String>,
    #[serde(default)]
    pub created_at: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct NotificationsResponse {
    pub notifications: Vec<NotificationRow>,
    #[serde(default)]
    pub unread: i64,
}

#[derive(Debug, Deserialize, Clone)]
pub struct CountResponse {
    pub count: i64,
}

#[derive(Debug, Deserialize, Clone)]
pub struct StatusResponse {
    pub status: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct FollowResponse {
    pub following: bool,
}

#[derive(Debug, Deserialize, Clone)]
pub struct MeResponse {
    pub id: String,
    pub username: String,
    #[serde(default)]
    pub email: String,
    #[serde(default)]
    pub credit_balance: i64,
    #[serde(default)]
    pub role: String,
    #[serde(default)]
    pub avatar_url: Option<String>,
    /// `everyone` | `friends` | `nobody`
    #[serde(default)]
    pub message_privacy: String,
    #[serde(default = "default_true")]
    pub online_status_visible: bool,
    /// `public` | `friends_only`
    #[serde(default)]
    pub profile_visibility: String,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Deserialize, Clone)]
pub struct BlockedUser {
    pub user_id: String,
    pub username: String,
    #[serde(default)]
    pub avatar_url: Option<String>,
}

/// Privacy settings sent to `PUT /api/user/privacy`. All fields optional —
/// omitted fields keep their current server-side value.
#[derive(Debug, Serialize, Clone, Default)]
pub struct PrivacySettings {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message_privacy: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub online_status_visible: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub profile_visibility: Option<String>,
}

// ── Friends ──

/// List accepted friends.
#[cfg(not(target_arch = "wasm32"))]
pub fn get_friends(session: &AuthSession) -> Result<Vec<FriendEntry>, String> {
    let token = require_token(session)?;
    get_json(&format!("{}/api/gameservices/friends", api_base()), Some(token))
}

/// List pending incoming friend requests.
#[cfg(not(target_arch = "wasm32"))]
pub fn get_friend_requests(session: &AuthSession) -> Result<Vec<FriendRequestEntry>, String> {
    let token = require_token(session)?;
    get_json(&format!("{}/api/gameservices/friends/requests", api_base()), Some(token))
}

/// Online status for each friend (respects their privacy settings).
#[cfg(not(target_arch = "wasm32"))]
pub fn get_friends_presence(session: &AuthSession) -> Result<Vec<PresenceEntry>, String> {
    let token = require_token(session)?;
    get_json(&format!("{}/api/gameservices/friends/presence", api_base()), Some(token))
}

/// Send a friend request by user id.
#[cfg(not(target_arch = "wasm32"))]
pub fn friend_add(session: &AuthSession, user_id: &str) -> Result<StatusResponse, String> {
    let token = require_token(session)?;
    post_json(
        &format!("{}/api/gameservices/friends/add", api_base()),
        &serde_json::json!({ "user_id": user_id }),
        Some(token),
    )
}

/// Accept an incoming friend request.
#[cfg(not(target_arch = "wasm32"))]
pub fn friend_accept(session: &AuthSession, user_id: &str) -> Result<StatusResponse, String> {
    let token = require_token(session)?;
    post_json(
        &format!("{}/api/gameservices/friends/accept", api_base()),
        &serde_json::json!({ "user_id": user_id }),
        Some(token),
    )
}

/// Remove a friend (also declines a pending request).
#[cfg(not(target_arch = "wasm32"))]
pub fn friend_remove(session: &AuthSession, user_id: &str) -> Result<serde_json::Value, String> {
    let token = require_token(session)?;
    post_json(
        &format!("{}/api/gameservices/friends/remove", api_base()),
        &serde_json::json!({ "user_id": user_id }),
        Some(token),
    )
}

/// Block a user by id.
#[cfg(not(target_arch = "wasm32"))]
pub fn friend_block(session: &AuthSession, user_id: &str) -> Result<serde_json::Value, String> {
    let token = require_token(session)?;
    post_json(
        &format!("{}/api/gameservices/friends/block", api_base()),
        &serde_json::json!({ "user_id": user_id }),
        Some(token),
    )
}

// ── Profiles ──

/// Search users by username prefix (public, min 2 chars).
#[cfg(not(target_arch = "wasm32"))]
pub fn search_users(query: &str) -> Result<Vec<UserSearchResult>, String> {
    get_json(
        &format!("{}/api/profiles/search?q={}", api_base(), urlencoded(query)),
        None,
    )
}

/// View a user's public profile. Pass the session to get `is_following`.
#[cfg(not(target_arch = "wasm32"))]
pub fn view_profile(username: &str, session: Option<&AuthSession>) -> Result<PublicProfile, String> {
    let token = session.and_then(|s| s.access_token.as_deref());
    get_json(&format!("{}/api/profiles/view/{username}", api_base()), token)
}

/// A user's published marketplace assets (public, 12 per page).
#[cfg(not(target_arch = "wasm32"))]
pub fn get_user_assets(username: &str, page: u32) -> Result<ProfileAssetsResponse, String> {
    get_json(
        &format!("{}/api/profiles/{username}/assets?page={page}", api_base()),
        None,
    )
}

/// Toggle following a user.
#[cfg(not(target_arch = "wasm32"))]
pub fn toggle_follow(session: &AuthSession, username: &str) -> Result<FollowResponse, String> {
    let token = require_token(session)?;
    post_json(
        &format!("{}/api/profiles/follow/{username}", api_base()),
        &serde_json::json!({}),
        Some(token),
    )
}

/// Block a user by username (profiles surface).
#[cfg(not(target_arch = "wasm32"))]
pub fn block_user(session: &AuthSession, username: &str) -> Result<serde_json::Value, String> {
    let token = require_token(session)?;
    post_json(
        &format!("{}/api/profiles/block/{username}", api_base()),
        &serde_json::json!({}),
        Some(token),
    )
}

// ── Notifications ──

/// List the newest 50 notifications plus the unread count.
#[cfg(not(target_arch = "wasm32"))]
pub fn get_notifications(session: &AuthSession) -> Result<NotificationsResponse, String> {
    let token = require_token(session)?;
    get_json(&format!("{}/api/notifications", api_base()), Some(token))
}

/// Unread notification count.
#[cfg(not(target_arch = "wasm32"))]
pub fn get_notification_count(session: &AuthSession) -> Result<CountResponse, String> {
    let token = require_token(session)?;
    get_json(&format!("{}/api/notifications/count", api_base()), Some(token))
}

/// Mark one notification as read.
#[cfg(not(target_arch = "wasm32"))]
pub fn mark_notification_read(session: &AuthSession, id: &str) -> Result<serde_json::Value, String> {
    let token = require_token(session)?;
    put_json(
        &format!("{}/api/notifications/{id}/read", api_base()),
        &serde_json::json!({}),
        Some(token),
    )
}

/// Mark all notifications as read.
#[cfg(not(target_arch = "wasm32"))]
pub fn mark_all_notifications_read(session: &AuthSession) -> Result<serde_json::Value, String> {
    let token = require_token(session)?;
    put_json(
        &format!("{}/api/notifications/read-all", api_base()),
        &serde_json::json!({}),
        Some(token),
    )
}

// ── Account / privacy ──

/// Current user info.
#[cfg(not(target_arch = "wasm32"))]
pub fn get_me(session: &AuthSession) -> Result<MeResponse, String> {
    let token = require_token(session)?;
    get_json(&format!("{}/api/user/me", api_base()), Some(token))
}

/// Update privacy settings (applies to the renzora.com account).
#[cfg(not(target_arch = "wasm32"))]
pub fn update_privacy(session: &AuthSession, settings: &PrivacySettings) -> Result<serde_json::Value, String> {
    let token = require_token(session)?;
    put_json(&format!("{}/api/user/privacy", api_base()), settings, Some(token))
}

/// List blocked users.
#[cfg(not(target_arch = "wasm32"))]
pub fn get_blocked(session: &AuthSession) -> Result<Vec<BlockedUser>, String> {
    let token = require_token(session)?;
    get_json(&format!("{}/api/user/blocked", api_base()), Some(token))
}

/// Unblock a user.
#[cfg(not(target_arch = "wasm32"))]
pub fn unblock(session: &AuthSession, user_id: &str) -> Result<serde_json::Value, String> {
    let token = require_token(session)?;
    delete_json(&format!("{}/api/user/blocked/{user_id}", api_base()), Some(token))
}


// ── Community extras ─────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, Clone)]
pub struct PopularUser {
    pub username: String,
    #[serde(default)]
    pub avatar_url: Option<String>,
    #[serde(default)]
    pub role: String,
    #[serde(default)]
    pub follower_count: i64,
}

/// Top users by follower count (public).
#[cfg(not(target_arch = "wasm32"))]
pub fn get_popular_users() -> Result<Vec<PopularUser>, String> {
    get_json(&format!("{}/api/profiles/popular", api_base()), None)
}

#[derive(Debug, Deserialize, Clone)]
pub struct UserRef {
    pub username: String,
    #[serde(default)]
    pub avatar_url: Option<String>,
    #[serde(default)]
    pub role: String,
}

/// Followers of a user (403 when their privacy hides it).
#[cfg(not(target_arch = "wasm32"))]
pub fn get_followers(session: Option<&AuthSession>, username: &str) -> Result<Vec<UserRef>, String> {
    let token = session.and_then(|s| s.access_token.as_deref());
    get_json(&format!("{}/api/profiles/{username}/followers", api_base()), token)
}

/// Who a user follows (403 when their privacy hides it).
#[cfg(not(target_arch = "wasm32"))]
pub fn get_following(session: Option<&AuthSession>, username: &str) -> Result<Vec<UserRef>, String> {
    let token = session.and_then(|s| s.access_token.as_deref());
    get_json(&format!("{}/api/profiles/{username}/following", api_base()), token)
}

/// A user's friends (403 when their privacy hides it).
#[cfg(not(target_arch = "wasm32"))]
pub fn get_friends_of(session: Option<&AuthSession>, username: &str) -> Result<Vec<UserRef>, String> {
    let token = session.and_then(|s| s.access_token.as_deref());
    get_json(&format!("{}/api/profiles/{username}/friends-list", api_base()), token)
}

#[derive(Debug, Deserialize, Clone)]
pub struct UserForumPost {
    #[serde(default)]
    pub thread_title: String,
    #[serde(default)]
    pub thread_slug: String,
    #[serde(default)]
    pub content: String,
    #[serde(default)]
    pub created_at: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct UserForumPostsResponse {
    pub posts: Vec<UserForumPost>,
    #[serde(default)]
    pub total: i64,
}

/// A user's recent forum posts (public).
#[cfg(not(target_arch = "wasm32"))]
pub fn get_user_forum_posts(username: &str, page: u32) -> Result<UserForumPostsResponse, String> {
    get_json(
        &format!("{}/api/profiles/{username}/forum-posts?page={page}", api_base()),
        None,
    )
}

/// Update the caller's forum signature (max 300 chars).
#[cfg(not(target_arch = "wasm32"))]
pub fn update_signature(session: &AuthSession, signature: &str) -> Result<serde_json::Value, String> {
    let token = require_token(session)?;
    put_json(
        &format!("{}/api/user/signature", api_base()),
        &serde_json::json!({ "signature": signature }),
        Some(token),
    )
}
