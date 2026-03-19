//! Persistent auth session — stores tokens on disk and in a Bevy resource.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::api::{AuthResponse, UserProfile};

/// Bevy resource holding the current authentication session.
#[derive(Resource, Default)]
pub struct AuthSession {
    /// The signed-in user, if any.
    pub user: Option<UserProfile>,
    /// Short-lived access token for API calls.
    pub access_token: Option<String>,
    /// Long-lived refresh token for renewing the session.
    pub refresh_token: Option<String>,
}

impl AuthSession {
    /// Returns true if the user is currently signed in.
    pub fn is_signed_in(&self) -> bool {
        self.user.is_some()
    }

    /// Update session from a successful auth response.
    pub fn set_from_response(&mut self, response: &AuthResponse) {
        self.user = Some(response.user.clone());
        self.access_token = Some(response.access_token.clone());
        self.refresh_token = Some(response.refresh_token.clone());
    }

    /// Clear the session (sign out).
    pub fn clear(&mut self) {
        self.user = None;
        self.access_token = None;
        self.refresh_token = None;
    }
}

// ── Disk persistence ──

#[derive(Serialize, Deserialize)]
struct StoredSession {
    access_token: String,
    refresh_token: String,
    user: UserProfile,
}

/// Directory for storing auth data.
#[cfg(not(target_arch = "wasm32"))]
fn auth_dir() -> Option<std::path::PathBuf> {
    dirs::data_local_dir().map(|p| p.join("renzora"))
}

/// Path to the auth session file.
#[cfg(not(target_arch = "wasm32"))]
fn auth_file() -> Option<std::path::PathBuf> {
    auth_dir().map(|p| p.join("auth.json"))
}

/// Save the current session to disk.
#[cfg(not(target_arch = "wasm32"))]
pub fn save_session(session: &AuthSession) {
    let Some(path) = auth_file() else { return };
    let Some(dir) = auth_dir() else { return };

    if let (Some(access), Some(refresh), Some(user)) = (
        &session.access_token,
        &session.refresh_token,
        &session.user,
    ) {
        let stored = StoredSession {
            access_token: access.clone(),
            refresh_token: refresh.clone(),
            user: user.clone(),
        };

        if let Ok(json) = serde_json::to_string_pretty(&stored) {
            let _ = std::fs::create_dir_all(&dir);
            let _ = std::fs::write(&path, json);
        }
    } else {
        // No session — remove the file
        let _ = std::fs::remove_file(&path);
    }
}

/// Load a saved session from disk.
#[cfg(not(target_arch = "wasm32"))]
pub fn load_session() -> Option<AuthSession> {
    let path = auth_file()?;
    let data = std::fs::read_to_string(&path).ok()?;
    let stored: StoredSession = serde_json::from_str(&data).ok()?;

    Some(AuthSession {
        user: Some(stored.user),
        access_token: Some(stored.access_token),
        refresh_token: Some(stored.refresh_token),
    })
}

/// Delete the saved session from disk.
#[cfg(not(target_arch = "wasm32"))]
pub fn delete_session() {
    if let Some(path) = auth_file() {
        let _ = std::fs::remove_file(&path);
    }
}
