//! HTTP client for the Renzora authentication API.

use serde::{Deserialize, Serialize};

/// Base URL for the Renzora API.
const API_BASE: &str = "https://renzora.com";

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

#[derive(Debug, Deserialize)]
pub struct ErrorResponse {
    pub error: String,
}

// ── API calls (blocking, run on background thread) ──

#[cfg(not(target_arch = "wasm32"))]
pub fn login(email: &str, password: &str) -> Result<AuthResponse, String> {
    let body = serde_json::to_string(&LoginRequest {
        email: email.to_string(),
        password: password.to_string(),
    })
    .map_err(|e| e.to_string())?;

    let response = ureq::post(&format!("{API_BASE}/api/auth/login"))
        .header("Content-Type", "application/json")
        .send(body.as_bytes())
        .map_err(|e| parse_error(e))?;

    let auth: AuthResponse = response.body_mut().read_json().map_err(|e| e.to_string())?;
    Ok(auth)
}

#[cfg(not(target_arch = "wasm32"))]
pub fn register(username: &str, email: &str, password: &str) -> Result<AuthResponse, String> {
    let body = serde_json::to_string(&RegisterRequest {
        username: username.to_string(),
        email: email.to_string(),
        password: password.to_string(),
    })
    .map_err(|e| e.to_string())?;

    let response = ureq::post(&format!("{API_BASE}/api/auth/register"))
        .header("Content-Type", "application/json")
        .send(body.as_bytes())
        .map_err(|e| parse_error(e))?;

    let auth: AuthResponse = response.body_mut().read_json().map_err(|e| e.to_string())?;
    Ok(auth)
}

#[cfg(not(target_arch = "wasm32"))]
pub fn refresh_token(refresh_token: &str) -> Result<AuthResponse, String> {
    let body = serde_json::to_string(&RefreshRequest {
        refresh_token: refresh_token.to_string(),
    })
    .map_err(|e| e.to_string())?;

    let response = ureq::post(&format!("{API_BASE}/api/auth/refresh"))
        .header("Content-Type", "application/json")
        .send(body.as_bytes())
        .map_err(|e| parse_error(e))?;

    let auth: AuthResponse = response.body_mut().read_json().map_err(|e| e.to_string())?;
    Ok(auth)
}

#[cfg(not(target_arch = "wasm32"))]
pub fn forgot_password(email: &str) -> Result<MessageResponse, String> {
    let body = serde_json::to_string(&ForgotPasswordRequest {
        email: email.to_string(),
    })
    .map_err(|e| e.to_string())?;

    let response = ureq::post(&format!("{API_BASE}/api/auth/forgot"))
        .header("Content-Type", "application/json")
        .send(body.as_bytes())
        .map_err(|e| parse_error(e))?;

    let msg: MessageResponse = response.body_mut().read_json().map_err(|e| e.to_string())?;
    Ok(msg)
}

#[cfg(not(target_arch = "wasm32"))]
fn parse_error(err: ureq::Error) -> String {
    match err {
        ureq::Error::StatusCode(code) => {
            format!("Request failed (HTTP {code})")
        }
        other => other.to_string(),
    }
}
