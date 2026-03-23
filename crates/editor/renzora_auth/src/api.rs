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

// ── API calls (blocking, run on background thread) ──

#[cfg(not(target_arch = "wasm32"))]
fn post_json<T: serde::de::DeserializeOwned>(url: &str, body: &impl Serialize) -> Result<T, String> {
    let json = serde_json::to_string(body).map_err(|e| e.to_string())?;

    let response = ureq::post(url)
        .header("Content-Type", "application/json")
        .send(json.as_bytes())
        .map_err(|e| format!("Request failed: {e}"))?;

    let body_str = response
        .into_body()
        .read_to_string()
        .map_err(|e| format!("Failed to read response: {e}"))?;

    serde_json::from_str(&body_str).map_err(|e| format!("Failed to parse response: {e}"))
}

#[cfg(not(target_arch = "wasm32"))]
pub fn login(email: &str, password: &str) -> Result<AuthResponse, String> {
    post_json(
        &format!("{API_BASE}/api/auth/login"),
        &LoginRequest {
            email: email.to_string(),
            password: password.to_string(),
        },
    )
}

#[cfg(not(target_arch = "wasm32"))]
pub fn register(username: &str, email: &str, password: &str) -> Result<AuthResponse, String> {
    post_json(
        &format!("{API_BASE}/api/auth/register"),
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
        &format!("{API_BASE}/api/auth/refresh"),
        &RefreshRequest {
            refresh_token: refresh_token.to_string(),
        },
    )
}

#[cfg(not(target_arch = "wasm32"))]
pub fn forgot_password(email: &str) -> Result<MessageResponse, String> {
    post_json(
        &format!("{API_BASE}/api/auth/forgot"),
        &ForgotPasswordRequest {
            email: email.to_string(),
        },
    )
}
