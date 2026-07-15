//! Forum API client — categories, threads, posts.
//!
//! Browsing is public (works signed out); posting requires authentication.
//! Blocking HTTP calls designed to run on background threads.

use serde::Deserialize;

use crate::client::api_base;
#[cfg(not(target_arch = "wasm32"))]
use crate::client::urlencoded;
#[cfg(not(target_arch = "wasm32"))]
use crate::client::{get_json, post_json, require_token};
use crate::session::AuthSession;

// ── Types ──

#[derive(Debug, Deserialize, Clone, Default)]
pub struct CategoryLastPost {
    #[serde(default)]
    pub thread_title: String,
    #[serde(default)]
    pub thread_slug: String,
    #[serde(default)]
    pub username: String,
    #[serde(default)]
    pub created_at: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ForumCategory {
    pub id: String,
    pub name: String,
    pub slug: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub icon: String,
    #[serde(default)]
    pub thread_count: i64,
    #[serde(default)]
    pub post_count: i64,
    #[serde(default)]
    pub last_post: Option<CategoryLastPost>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ThreadSummary {
    pub id: String,
    pub title: String,
    pub slug: String,
    #[serde(default)]
    pub pinned: bool,
    #[serde(default)]
    pub locked: bool,
    #[serde(default)]
    pub post_count: i64,
    #[serde(default)]
    pub views: i64,
    #[serde(default)]
    pub author_name: String,
    #[serde(default)]
    pub author_avatar_url: Option<String>,
    #[serde(default)]
    pub last_post_at: Option<String>,
    #[serde(default)]
    pub created_at: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ForumThread {
    pub id: String,
    pub title: String,
    pub slug: String,
    #[serde(default)]
    pub pinned: bool,
    #[serde(default)]
    pub locked: bool,
    #[serde(default)]
    pub post_count: i64,
    #[serde(default)]
    pub views: i64,
    #[serde(default)]
    pub created_at: String,
}

pub use crate::feed::Reaction;

#[derive(Debug, Deserialize, Clone)]
pub struct ForumPostRow {
    pub id: String,
    #[serde(default)]
    pub content: String,
    #[serde(default)]
    pub is_first_post: bool,
    #[serde(default)]
    pub edited: bool,
    #[serde(default)]
    pub author_id: String,
    #[serde(default)]
    pub author_name: String,
    #[serde(default)]
    pub author_role: String,
    #[serde(default)]
    pub author_post_count: i64,
    #[serde(default)]
    pub author_avatar_url: Option<String>,
    #[serde(default)]
    pub author_signature: String,
    #[serde(default)]
    pub reactions: Vec<Reaction>,
    #[serde(default)]
    pub created_at: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct CategoryThreadsResponse {
    pub category: ForumCategory,
    pub threads: Vec<ThreadSummary>,
    #[serde(default)]
    pub total: i64,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ThreadResponse {
    pub thread: ForumThread,
    pub posts: Vec<ForumPostRow>,
    #[serde(default)]
    pub total_posts: i64,
}

// ── API calls ──

/// List forum categories (public).
#[cfg(not(target_arch = "wasm32"))]
pub fn get_categories() -> Result<Vec<ForumCategory>, String> {
    get_json(&format!("{}/api/forum/categories", api_base()), None)
}

/// Threads in a category, 25 per page (public).
#[cfg(not(target_arch = "wasm32"))]
pub fn get_category(slug: &str, page: u32) -> Result<CategoryThreadsResponse, String> {
    get_json(
        &format!("{}/api/forum/categories/{slug}?page={page}", api_base()),
        None,
    )
}

/// A thread with its posts, 20 per page (public).
#[cfg(not(target_arch = "wasm32"))]
pub fn get_thread(slug: &str, page: u32) -> Result<ThreadResponse, String> {
    get_json(&format!("{}/api/forum/threads/{slug}?page={page}", api_base()), None)
}

/// Create a thread (authenticated).
#[cfg(not(target_arch = "wasm32"))]
pub fn create_thread(
    session: &AuthSession,
    category_slug: &str,
    title: &str,
    content: &str,
) -> Result<ForumThread, String> {
    let token = require_token(session)?;
    post_json(
        &format!("{}/api/forum/threads", api_base()),
        &serde_json::json!({ "category_slug": category_slug, "title": title, "content": content }),
        Some(token),
    )
}

/// Reply to a thread (authenticated).
#[cfg(not(target_arch = "wasm32"))]
pub fn reply(
    session: &AuthSession,
    thread_slug: &str,
    content: &str,
) -> Result<serde_json::Value, String> {
    let token = require_token(session)?;
    post_json(
        &format!("{}/api/forum/threads/{thread_slug}/reply", api_base()),
        &serde_json::json!({ "content": content }),
        Some(token),
    )
}


#[derive(Debug, Deserialize, Clone)]
pub struct ForumSearchHit {
    pub id: String,
    pub title: String,
    pub slug: String,
    #[serde(default)]
    pub category_slug: String,
    #[serde(default)]
    pub author_name: String,
    #[serde(default)]
    pub post_count: i64,
    #[serde(default)]
    pub last_post_at: Option<String>,
    #[serde(default)]
    pub created_at: String,
}

/// Search threads by title/content (public).
#[cfg(not(target_arch = "wasm32"))]
pub fn search_forum(query: &str) -> Result<Vec<ForumSearchHit>, String> {
    get_json(
        &format!("{}/api/forum/search?q={}", api_base(), urlencoded(query)),
        None,
    )
}

/// Toggle a phosphor-icon reaction on a forum post.
#[cfg(not(target_arch = "wasm32"))]
pub fn react_to_post(session: &AuthSession, post_id: &str, icon: &str) -> Result<serde_json::Value, String> {
    let token = require_token(session)?;
    post_json(
        &format!("{}/api/forum/posts/{post_id}/reactions", api_base()),
        &serde_json::json!({ "icon": icon }),
        Some(token),
    )
}
