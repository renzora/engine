//! HTTP client for the Renzora authentication API.

use serde::{Deserialize, Serialize};

use super::client::api_base;

// ── Request types ──

#[derive(Serialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

#[derive(Serialize)]
pub struct RegisterRequest {
    pub username: String,
    pub email: String,
    pub password: String,
}

#[derive(Serialize)]
pub struct RefreshRequest {
    pub refresh_token: String,
}

#[derive(Serialize)]
pub struct ForgotPasswordRequest {
    pub email: String,
}

// ── Response types ──

#[derive(Debug, Deserialize, Clone)]
pub struct AuthResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub user: UserProfile,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct UserProfile {
    pub id: String,
    pub username: String,
    pub email: String,
    pub role: String,
    pub credit_balance: i64,
}

#[derive(Debug, Deserialize)]
pub struct MessageResponse {
    pub message: String,
}

// ── API calls (blocking, run on background thread) ──

#[cfg(not(target_arch = "wasm32"))]
fn post_json<T: serde::de::DeserializeOwned>(
    url: &str,
    body: &impl Serialize,
) -> Result<T, String> {
    renzora::net::Request::post(url)
        .json(body)
        .send()
        .map_err(|e| format!("Request failed: {e}"))?
        .json()
        .map_err(|e| e.to_string())
}

#[cfg(not(target_arch = "wasm32"))]
pub fn login(email: &str, password: &str) -> Result<AuthResponse, String> {
    post_json(
        &format!("{}/api/auth/login", api_base()),
        &LoginRequest {
            email: email.to_string(),
            password: password.to_string(),
        },
    )
}

#[cfg(not(target_arch = "wasm32"))]
pub fn register(username: &str, email: &str, password: &str) -> Result<AuthResponse, String> {
    post_json(
        &format!("{}/api/auth/register", api_base()),
        &RegisterRequest {
            username: username.to_string(),
            email: email.to_string(),
            password: password.to_string(),
        },
    )
}

#[cfg(not(target_arch = "wasm32"))]
pub fn refresh_token(refresh_token: &str) -> Result<AuthResponse, String> {
    post_json(
        &format!("{}/api/auth/refresh", api_base()),
        &RefreshRequest {
            refresh_token: refresh_token.to_string(),
        },
    )
}

/// Why a token refresh did not produce a new session.
///
/// The distinction is the whole point of this type, and it decides whether the
/// user stays signed in: only [`Rejected`](Self::Rejected) means the saved
/// session is actually dead. Everything else — offline, no HTTP plugin, the API
/// returning a 502 — leaves it alone.
///
/// Collapsing the two was a real bug: `try_restore_session` treated any `Err` as
/// "token expired", so a failed refresh **deleted the session file**, and the
/// editor asked for a sign-in on every launch.
#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug)]
pub enum RefreshFailure {
    /// The server answered and refused the token. It is not coming back.
    Rejected(String),
    /// No answer, or an answer that says nothing about the token: a transport
    /// failure, a missing network backend, or a 5xx. Try again later; the
    /// session stays as it is.
    Unavailable(String),
}

#[cfg(not(target_arch = "wasm32"))]
impl std::fmt::Display for RefreshFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Rejected(e) | Self::Unavailable(e) => f.write_str(e),
        }
    }
}

/// Refresh a token, saying whether a failure invalidates the session.
///
/// Goes through `renzora_net` directly rather than [`post_json`] because the
/// status code is the entire signal here and that helper folds it into a string.
#[cfg(not(target_arch = "wasm32"))]
pub fn refresh_token_checked(refresh_token: &str) -> Result<AuthResponse, RefreshFailure> {
    let response = renzora::net::Request::post(&format!("{}/api/auth/refresh", api_base()))
        .json(&RefreshRequest {
            refresh_token: refresh_token.to_string(),
        })
        .send()
        .map_err(|e| RefreshFailure::Unavailable(e.to_string()))?;

    // 401/403 are the only answers that mean the token itself is bad. A 500 or a
    // 502 is the API having a bad day, and signing every user out over it would
    // turn a brief outage into a support queue.
    if response.status == 401 || response.status == 403 {
        return Err(RefreshFailure::Rejected(
            response
                .json::<serde_json::Value>()
                .err()
                .map(|e| e.to_string())
                .unwrap_or_else(|| "session expired".to_string()),
        ));
    }
    if !response.is_ok() {
        return Err(RefreshFailure::Unavailable(format!(
            "HTTP {}",
            response.status
        )));
    }
    response
        .json()
        .map_err(|e| RefreshFailure::Unavailable(e.to_string()))
}

#[cfg(not(target_arch = "wasm32"))]
pub fn forgot_password(email: &str) -> Result<MessageResponse, String> {
    post_json(
        &format!("{}/api/auth/forgot", api_base()),
        &ForgotPasswordRequest {
            email: email.to_string(),
        },
    )
}

// WASM stubs — auth not yet supported in browser
#[cfg(target_arch = "wasm32")]
pub fn login(_email: &str, _password: &str) -> Result<AuthResponse, String> {
    Err("Authentication is not supported in the browser yet".into())
}

#[cfg(target_arch = "wasm32")]
pub fn register(_username: &str, _email: &str, _password: &str) -> Result<AuthResponse, String> {
    Err("Authentication is not supported in the browser yet".into())
}

#[cfg(target_arch = "wasm32")]
pub fn refresh_token(_refresh_token: &str) -> Result<AuthResponse, String> {
    Err("Authentication is not supported in the browser yet".into())
}

#[cfg(target_arch = "wasm32")]
pub fn forgot_password(_email: &str) -> Result<MessageResponse, String> {
    Err("Authentication is not supported in the browser yet".into())
}
