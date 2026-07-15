//! Teams API client — teams, members, invites, and the team chat conversation.
//!
//! Blocking HTTP calls designed to run on background threads.

use serde::Deserialize;

use crate::client::api_base;
#[cfg(not(target_arch = "wasm32"))]
use crate::client::{delete_json, get_json, post_json, put_json, require_token};
use crate::session::AuthSession;

// ── Types ──

#[derive(Debug, Deserialize, Clone)]
pub struct Team {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub slug: String,
    pub owner_id: String,
    #[serde(default)]
    pub avatar_url: Option<String>,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub created_at: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct TeamMemberEntry {
    pub user_id: String,
    #[serde(default)]
    pub role: String,
    #[serde(default)]
    pub username: String,
    #[serde(default)]
    pub avatar_url: Option<String>,
    #[serde(default)]
    pub joined_at: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct TeamInvite {
    pub id: String,
    pub team_id: String,
    #[serde(default)]
    pub invited_by: String,
    #[serde(default)]
    pub invited_user_id: Option<String>,
    #[serde(default)]
    pub invited_email: Option<String>,
    #[serde(default)]
    pub role: String,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub created_at: String,
    #[serde(default)]
    pub expires_at: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct TeamDetail {
    pub team: Team,
    #[serde(default)]
    pub members: Vec<TeamMemberEntry>,
    #[serde(default)]
    pub invites: Vec<TeamInvite>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct TeamConversationResponse {
    pub conversation_id: String,
}

// ── API calls ──

/// List teams the user belongs to.
#[cfg(not(target_arch = "wasm32"))]
pub fn list_teams(session: &AuthSession) -> Result<Vec<Team>, String> {
    let token = require_token(session)?;
    get_json(&format!("{}/api/teams", api_base()), Some(token))
}

/// Create a team.
#[cfg(not(target_arch = "wasm32"))]
pub fn create_team(session: &AuthSession, name: &str, description: &str) -> Result<Team, String> {
    let token = require_token(session)?;
    post_json(
        &format!("{}/api/teams", api_base()),
        &serde_json::json!({ "name": name, "description": description }),
        Some(token),
    )
}

/// Team detail with members and pending invites (member-only).
#[cfg(not(target_arch = "wasm32"))]
pub fn get_team(session: &AuthSession, team_id: &str) -> Result<TeamDetail, String> {
    let token = require_token(session)?;
    get_json(&format!("{}/api/teams/{team_id}", api_base()), Some(token))
}

/// Invite a user by username or email (owner/admin only).
#[cfg(not(target_arch = "wasm32"))]
pub fn invite_member(
    session: &AuthSession,
    team_id: &str,
    identifier: &str,
    role: Option<&str>,
) -> Result<serde_json::Value, String> {
    let token = require_token(session)?;
    post_json(
        &format!("{}/api/teams/{team_id}/invite", api_base()),
        &serde_json::json!({ "identifier": identifier, "role": role }),
        Some(token),
    )
}

/// List my pending team invites.
#[cfg(not(target_arch = "wasm32"))]
pub fn list_invites(session: &AuthSession) -> Result<Vec<TeamInvite>, String> {
    let token = require_token(session)?;
    get_json(&format!("{}/api/teams/invites", api_base()), Some(token))
}

/// Accept a team invite.
#[cfg(not(target_arch = "wasm32"))]
pub fn accept_invite(session: &AuthSession, invite_id: &str) -> Result<serde_json::Value, String> {
    let token = require_token(session)?;
    post_json(
        &format!("{}/api/teams/invites/{invite_id}/accept", api_base()),
        &serde_json::json!({}),
        Some(token),
    )
}

/// Decline a team invite.
#[cfg(not(target_arch = "wasm32"))]
pub fn decline_invite(session: &AuthSession, invite_id: &str) -> Result<serde_json::Value, String> {
    let token = require_token(session)?;
    post_json(
        &format!("{}/api/teams/invites/{invite_id}/decline", api_base()),
        &serde_json::json!({}),
        Some(token),
    )
}

/// Change a member's role (owner only). Role must be `member` or `admin`.
#[cfg(not(target_arch = "wasm32"))]
pub fn update_member_role(
    session: &AuthSession,
    team_id: &str,
    user_id: &str,
    role: &str,
) -> Result<serde_json::Value, String> {
    let token = require_token(session)?;
    put_json(
        &format!("{}/api/teams/{team_id}/members/{user_id}/role", api_base()),
        &serde_json::json!({ "role": role }),
        Some(token),
    )
}

/// Remove a member (or leave the team by removing yourself).
#[cfg(not(target_arch = "wasm32"))]
pub fn remove_member(
    session: &AuthSession,
    team_id: &str,
    user_id: &str,
) -> Result<serde_json::Value, String> {
    let token = require_token(session)?;
    delete_json(
        &format!("{}/api/teams/{team_id}/members/{user_id}", api_base()),
        Some(token),
    )
}

/// Get or create the team chat conversation (member-only).
#[cfg(not(target_arch = "wasm32"))]
pub fn get_team_conversation(
    session: &AuthSession,
    team_id: &str,
) -> Result<TeamConversationResponse, String> {
    let token = require_token(session)?;
    post_json(
        &format!("{}/api/teams/{team_id}/conversation", api_base()),
        &serde_json::json!({}),
        Some(token),
    )
}
