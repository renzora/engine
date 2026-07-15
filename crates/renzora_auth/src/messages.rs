//! Messaging API client — conversations (DMs, groups, team chats) and messages.
//!
//! Blocking HTTP calls designed to run on background threads.

use serde::Deserialize;

use crate::client::api_base;
#[cfg(not(target_arch = "wasm32"))]
use crate::client::urlencoded;
#[cfg(not(target_arch = "wasm32"))]
use crate::client::{delete_json, get_json, post_json, put_json, require_token};
use crate::session::AuthSession;

// ── Types ──

#[derive(Debug, Deserialize, Clone)]
pub struct ConversationPreview {
    pub id: String,
    /// `dm`, `group`, `team`, or `admin_staff`.
    pub kind: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub avatar_url: Option<String>,
    #[serde(default)]
    pub updated_at: String,
    #[serde(default)]
    pub last_message_body: Option<String>,
    #[serde(default)]
    pub last_message_sender: Option<String>,
    #[serde(default)]
    pub last_message_at: Option<String>,
    #[serde(default)]
    pub unread_count: i64,
}

#[derive(Debug, Deserialize, Clone)]
pub struct MessageRow {
    pub id: String,
    pub conversation_id: String,
    pub sender_id: String,
    #[serde(default)]
    pub sender_username: String,
    #[serde(default)]
    pub sender_avatar_url: Option<String>,
    #[serde(default)]
    pub body: String,
    #[serde(default)]
    pub reply_to_id: Option<String>,
    #[serde(default)]
    pub edited_at: Option<String>,
    #[serde(default)]
    pub deleted: bool,
    #[serde(default)]
    pub created_at: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Participant {
    pub user_id: String,
    pub username: String,
    #[serde(default)]
    pub avatar_url: Option<String>,
    #[serde(default)]
    pub role: String,
    #[serde(default)]
    pub joined_at: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct SentMessage {
    pub id: String,
    pub conversation_id: String,
    #[serde(default)]
    pub body: String,
    #[serde(default)]
    pub created_at: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ConversationIdResponse {
    pub conversation_id: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct UnreadCountResponse {
    pub count: i64,
}

// ── API calls ──

/// List the user's conversations with preview info.
#[cfg(not(target_arch = "wasm32"))]
pub fn list_conversations(session: &AuthSession) -> Result<Vec<ConversationPreview>, String> {
    let token = require_token(session)?;
    get_json(&format!("{}/api/messages/conversations", api_base()), Some(token))
}

/// Find or create a DM with a user.
#[cfg(not(target_arch = "wasm32"))]
pub fn open_dm(session: &AuthSession, user_id: &str) -> Result<ConversationIdResponse, String> {
    let token = require_token(session)?;
    post_json(
        &format!("{}/api/messages/conversations/dm/{user_id}", api_base()),
        &serde_json::json!({}),
        Some(token),
    )
}

/// Create a group conversation.
#[cfg(not(target_arch = "wasm32"))]
pub fn create_group(
    session: &AuthSession,
    name: &str,
    member_ids: &[String],
) -> Result<ConversationIdResponse, String> {
    let token = require_token(session)?;
    post_json(
        &format!("{}/api/messages/conversations/group", api_base()),
        &serde_json::json!({ "name": name, "member_ids": member_ids }),
        Some(token),
    )
}

/// List messages, newest first. `before` is a message id cursor for history.
#[cfg(not(target_arch = "wasm32"))]
pub fn list_messages(
    session: &AuthSession,
    conversation_id: &str,
    before: Option<&str>,
    limit: u32,
) -> Result<Vec<MessageRow>, String> {
    let token = require_token(session)?;
    let mut url = format!(
        "{}/api/messages/conversations/{conversation_id}/messages?limit={limit}",
        api_base()
    );
    if let Some(b) = before {
        url.push_str(&format!("&before={b}"));
    }
    get_json(&url, Some(token))
}

/// Send a message.
#[cfg(not(target_arch = "wasm32"))]
pub fn send_message(
    session: &AuthSession,
    conversation_id: &str,
    body: &str,
    reply_to_id: Option<&str>,
) -> Result<SentMessage, String> {
    let token = require_token(session)?;
    post_json(
        &format!("{}/api/messages/conversations/{conversation_id}/messages", api_base()),
        &serde_json::json!({ "body": body, "reply_to_id": reply_to_id }),
        Some(token),
    )
}

/// Edit one of your own messages.
#[cfg(not(target_arch = "wasm32"))]
pub fn edit_message(
    session: &AuthSession,
    conversation_id: &str,
    message_id: &str,
    body: &str,
) -> Result<serde_json::Value, String> {
    let token = require_token(session)?;
    put_json(
        &format!(
            "{}/api/messages/conversations/{conversation_id}/messages/{message_id}",
            api_base()
        ),
        &serde_json::json!({ "body": body }),
        Some(token),
    )
}

/// Delete one of your own messages (soft delete).
#[cfg(not(target_arch = "wasm32"))]
pub fn delete_message(
    session: &AuthSession,
    conversation_id: &str,
    message_id: &str,
) -> Result<serde_json::Value, String> {
    let token = require_token(session)?;
    delete_json(
        &format!(
            "{}/api/messages/conversations/{conversation_id}/messages/{message_id}",
            api_base()
        ),
        Some(token),
    )
}

/// Mark a conversation as read.
#[cfg(not(target_arch = "wasm32"))]
pub fn mark_read(session: &AuthSession, conversation_id: &str) -> Result<serde_json::Value, String> {
    let token = require_token(session)?;
    post_json(
        &format!("{}/api/messages/conversations/{conversation_id}/read", api_base()),
        &serde_json::json!({}),
        Some(token),
    )
}

/// List conversation participants.
#[cfg(not(target_arch = "wasm32"))]
pub fn list_participants(
    session: &AuthSession,
    conversation_id: &str,
) -> Result<Vec<Participant>, String> {
    let token = require_token(session)?;
    get_json(
        &format!("{}/api/messages/conversations/{conversation_id}/participants", api_base()),
        Some(token),
    )
}

/// Total unread message count across all conversations.
#[cfg(not(target_arch = "wasm32"))]
pub fn unread_count(session: &AuthSession) -> Result<UnreadCountResponse, String> {
    let token = require_token(session)?;
    get_json(&format!("{}/api/messages/unread-count", api_base()), Some(token))
}


#[derive(Debug, Deserialize, Clone)]
pub struct MessageSearchHit {
    pub conversation_id: String,
    #[serde(default)]
    pub conversation_name: Option<String>,
    pub message_id: String,
    #[serde(default)]
    pub body: String,
    #[serde(default)]
    pub sender_username: String,
    #[serde(default)]
    pub created_at: String,
}

/// Search messages across the caller's conversations.
#[cfg(not(target_arch = "wasm32"))]
pub fn search_messages(session: &AuthSession, query: &str) -> Result<Vec<MessageSearchHit>, String> {
    let token = require_token(session)?;
    get_json(
        &format!("{}/api/messages/search?q={}&limit=30", api_base(), urlencoded(query)),
        Some(token),
    )
}
